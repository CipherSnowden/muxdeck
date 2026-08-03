/// Engine status, and anything wrong with it.
library;

import 'dart:async';

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../domain/admin_session.dart';
import '../providers.dart';

class DashboardPage extends ConsumerWidget {
  const DashboardPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(adminSessionProvider);

    return switch (state) {
      AdminConnecting() => const _Centred(
        icon: Icons.hourglass_empty,
        title: 'Looking for the engine…',
        busy: true,
      ),
      AdminNotInstalled(:final canInstall) => _NotInstalled(
        canInstall: canInstall,
      ),
      AdminEngineStopped(:final detail) => _Stopped(detail: detail),
      AdminFailed(:final message) => _Centred(
        icon: Icons.gpp_bad,
        title: 'Something is wrong',
        detail: message,
      ),
      AdminReady() => _Running(state: state),
    };
  }
}

class _Running extends ConsumerStatefulWidget {
  const _Running({required this.state});

  final AdminReady state;

  @override
  ConsumerState<_Running> createState() => _RunningState();
}

class _RunningState extends ConsumerState<_Running> {
  @override
  void initState() {
    super.initState();
    // Deferred past the first frame: both touch providers, and Riverpod refuses a write while
    // the widget tree is still building.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (!mounted) return;
      unawaited(ref.read(telemetryProvider.notifier).subscribe());
      ref.read(logProvider.notifier).start();
    });
  }

  @override
  void dispose() {
    ref.read(logProvider.notifier).stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final state = widget.state;
    final ready = state.ready;
    final connected = state.devices.where((d) => d.connected).length;
    final telemetry = ref.watch(telemetryProvider).latest;

    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        // The preflight result is the loudest thing on the screen when it fails, because the
        // alternative is buttons that silently do nothing. `docs/SERVER.md` §6.
        if (state.inputUnavailable)
          _InputUnavailableBanner(platform: ready.hostPlatform),

        _StatusCard(
          title: 'Engine running',
          subtitle:
              'Version ${ready.engineVersion} · ${ready.hostPlatform.wire}',
          icon: Icons.check_circle,
          colour: const Color(0xFF1F8A70),
        ),
        const SizedBox(height: 16),

        Row(
          children: [
            Expanded(
              child: _Metric(
                label: 'Paired devices',
                value: '${state.devices.length}',
                icon: Icons.devices,
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: _Metric(
                label: 'Connected now',
                value: '$connected',
                icon: Icons.wifi_tethering,
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: _Metric(
                label: 'CPU',
                // Em dash rather than 0% until the first sample arrives: a dashboard that says
                // the machine is idle before it has measured anything is lying.
                value: telemetry == null
                    ? '—'
                    : '${telemetry.cpuPct.toStringAsFixed(1)}%',
                icon: Icons.memory,
              ),
            ),
            const SizedBox(width: 16),
            Expanded(
              child: _Metric(
                label: 'Memory',
                value: telemetry == null
                    ? '—'
                    : '${telemetry.ramPct.toStringAsFixed(1)}%',
                icon: Icons.storage,
              ),
            ),
          ],
        ),
        const SizedBox(height: 24),

        Text(
          'What this host can do',
          style: Theme.of(context).textTheme.titleMedium,
        ),
        const SizedBox(height: 8),
        _CapabilityRow(
          label: 'Keyboard and text',
          available: ready.capabilities.textUnicode,
        ),
        _CapabilityRow(
          label: 'Media keys',
          available: ready.capabilities.mediaKeys,
        ),
        _CapabilityRow(label: 'Mouse', available: ready.capabilities.mouse),
        _CapabilityRow(
          label: 'Shell actions',
          available: ready.capabilities.shellActions,
          offIsNormal: true,
        ),

        const SizedBox(height: 24),
        const _LogTail(),
      ],
    );
  }
}

/// The engine's recent output, read from its log file.
///
/// Read from disk rather than fetched over the socket: the engine has no `log.tail` op, and
/// adding one would put a file read on the protocol for something only a panel on the same
/// machine could ever use.
class _LogTail extends ConsumerWidget {
  const _LogTail();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final log = ref.watch(logProvider);
    final theme = Theme.of(context);

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text('Recent log', style: theme.textTheme.titleMedium),
            const Spacer(),
            if (log.path != null)
              Text(
                log.path!,
                style: theme.textTheme.bodySmall,
                overflow: TextOverflow.ellipsis,
              ),
          ],
        ),
        const SizedBox(height: 8),
        Container(
          height: 220,
          width: double.infinity,
          padding: const EdgeInsets.all(12),
          decoration: BoxDecoration(
            color: const Color(0xFF0C0E13),
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: Colors.white12),
          ),
          child: log.error != null
              ? Center(
                  child: Text(log.error!, style: theme.textTheme.bodySmall),
                )
              // Reversed so the newest line is at the bottom **and** the view starts scrolled
              // there. A log that opens at the top of a 200-line tail shows the oldest thing
              // that happened, which is never what anybody came for.
              : ListView(
                  reverse: true,
                  children: [
                    for (final line in log.lines.reversed)
                      SelectableText(
                        line,
                        style: theme.textTheme.bodySmall?.copyWith(
                          fontFamily: 'monospace',
                          color: _lineColour(line),
                        ),
                      ),
                  ],
                ),
        ),
      ],
    );
  }

  /// `tracing`'s level word is the only structure worth reading out of a log line here.
  Color? _lineColour(String line) {
    if (line.contains(' ERROR ')) return const Color(0xFFE06C5A);
    if (line.contains(' WARN ')) return const Color(0xFFD8A657);
    return Colors.white70;
  }
}

/// Shown when the engine reports it cannot inject anything.
///
/// On Windows this should never appear — `SendInput` needs no permission. It exists for macOS
/// (Accessibility not granted) and Linux (`/dev/uinput` not writable).
///
/// The remediation is chosen from the host platform rather than from the engine's own
/// `preflight()` message, which is more specific but does not travel: `Ready` carries the
/// capability flags and no error string (`docs/PROTOCOL.md` §4.1). Naming a wire field for it
/// would be a protocol change, so the panel says the platform-typical fix instead.
class _InputUnavailableBanner extends StatelessWidget {
  const _InputUnavailableBanner({required this.platform});

  final HostPlatform platform;

  String get _remediation => switch (platform) {
    HostPlatform.macos =>
      'Open System Settings > Privacy & Security > Accessibility, add MuxDeck, and turn it '
          'on. The permission is remembered per application binary, so moving or replacing '
          'the app means granting it again.',
    HostPlatform.linux =>
      'MuxDeck cannot open /dev/uinput. Add your user to the input group and then log out '
          'and back in — a new login is required, restarting the app is not enough:\n\n'
          '    sudo usermod -aG input \$USER',
    _ =>
      'The engine could not reach this desktop session. If it was started by a scheduled '
          'task, check that the task runs as you and not as SYSTEM.',
  };

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 16),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: const Color(0xFFB3422F).withValues(alpha: 0.15),
        border: Border.all(color: const Color(0xFFB3422F)),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Icon(Icons.warning_amber, color: Color(0xFFB3422F)),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  'This computer cannot receive button presses',
                  style: Theme.of(context).textTheme.titleSmall,
                ),
                const SizedBox(height: 4),
                const Text(
                  'The engine is running, but it cannot send keystrokes to this desktop. '
                  'Your deck will connect and its buttons will do nothing.',
                ),
                const SizedBox(height: 8),
                SelectableText(_remediation),
              ],
            ),
          ),
        ],
      ),
    );
  }
}

class _NotInstalled extends ConsumerWidget {
  const _NotInstalled({required this.canInstall});

  final bool canInstall;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return _Centred(
      icon: Icons.download_for_offline,
      title: 'MuxDeck is not set up yet',
      detail: canInstall
          ? 'Start the engine to create this computer\'s identity. It will then run in the '
                'background, and your deck will work whether or not this window is open.'
          : 'The muxdeckd program could not be found. It should sit next to this app.',
      action: canInstall
          ? FilledButton.icon(
              onPressed: () =>
                  ref.read(adminSessionProvider.notifier).connect(),
              icon: const Icon(Icons.play_arrow),
              label: const Text('Start the engine'),
            )
          : null,
    );
  }
}

class _Stopped extends ConsumerWidget {
  const _Stopped({required this.detail});

  final String detail;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return _Centred(
      icon: Icons.power_settings_new,
      title: 'The engine is not running',
      detail: detail,
      action: FilledButton.icon(
        onPressed: () => ref.read(adminSessionProvider.notifier).connect(),
        icon: const Icon(Icons.play_arrow),
        label: const Text('Start the engine'),
      ),
    );
  }
}

class _StatusCard extends StatelessWidget {
  const _StatusCard({
    required this.title,
    required this.subtitle,
    required this.icon,
    required this.colour,
  });

  final String title;
  final String subtitle;
  final IconData icon;
  final Color colour;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: Icon(icon, color: colour, size: 32),
        title: Text(title, style: Theme.of(context).textTheme.titleMedium),
        subtitle: Text(subtitle),
      ),
    );
  }
}

class _Metric extends StatelessWidget {
  const _Metric({required this.label, required this.value, required this.icon});

  final String label;
  final String value;
  final IconData icon;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: Padding(
        padding: const EdgeInsets.all(16),
        child: Row(
          children: [
            Icon(icon, size: 28),
            const SizedBox(width: 12),
            Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(value, style: Theme.of(context).textTheme.headlineSmall),
                Text(label, style: Theme.of(context).textTheme.bodySmall),
              ],
            ),
          ],
        ),
      ),
    );
  }
}

class _CapabilityRow extends StatelessWidget {
  const _CapabilityRow({
    required this.label,
    required this.available,
    this.offIsNormal = false,
  });

  final String label;
  final bool available;

  /// True for capabilities that are off by default on purpose, so "off" is not drawn as a
  /// fault. Shell actions are the only one — see `docs/ARCHITECTURE.md` §5.5.
  final bool offIsNormal;

  @override
  Widget build(BuildContext context) {
    final good = available || offIsNormal;
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 4),
      child: Row(
        children: [
          Icon(
            available
                ? Icons.check_circle_outline
                : Icons.remove_circle_outline,
            size: 18,
            color: good ? const Color(0xFF1F8A70) : const Color(0xFFB3422F),
          ),
          const SizedBox(width: 10),
          Text(label),
          if (!available && offIsNormal) ...[
            const SizedBox(width: 8),
            Text(
              '(off by default)',
              style: Theme.of(context).textTheme.bodySmall,
            ),
          ],
        ],
      ),
    );
  }
}

class _Centred extends StatelessWidget {
  const _Centred({
    required this.icon,
    required this.title,
    this.detail,
    this.action,
    this.busy = false,
  });

  final IconData icon;
  final String title;
  final String? detail;
  final Widget? action;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: ConstrainedBox(
        constraints: const BoxConstraints(maxWidth: 460),
        child: Padding(
          padding: const EdgeInsets.all(32),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (busy)
                const CircularProgressIndicator()
              else
                Icon(icon, size: 48),
              const SizedBox(height: 20),
              Text(
                title,
                textAlign: TextAlign.center,
                style: Theme.of(context).textTheme.titleLarge,
              ),
              if (detail != null) ...[
                const SizedBox(height: 10),
                Text(detail!, textAlign: TextAlign.center),
              ],
              if (action != null) ...[const SizedBox(height: 24), action!],
            ],
          ),
        ),
      ),
    );
  }
}
