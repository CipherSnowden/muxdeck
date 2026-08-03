/// Engine status, and anything wrong with it.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

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
      AdminNotInstalled(:final canInstall) => _NotInstalled(canInstall: canInstall),
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

class _Running extends ConsumerWidget {
  const _Running({required this.state});

  final AdminReady state;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final ready = state.ready;
    final connected = state.devices.where((d) => d.connected).length;

    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        // The preflight result is the loudest thing on the screen when it fails, because the
        // alternative is buttons that silently do nothing. `docs/SERVER.md` §6.
        if (state.inputUnavailable) const _InputUnavailableBanner(),

        _StatusCard(
          title: 'Engine running',
          subtitle: 'Version ${ready.engineVersion} · ${ready.hostPlatform.wire}',
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
          ],
        ),
        const SizedBox(height: 24),

        Text('What this host can do', style: Theme.of(context).textTheme.titleMedium),
        const SizedBox(height: 8),
        _CapabilityRow(
          label: 'Keyboard and text',
          available: ready.capabilities.textUnicode,
        ),
        _CapabilityRow(label: 'Media keys', available: ready.capabilities.mediaKeys),
        _CapabilityRow(label: 'Mouse', available: ready.capabilities.mouse),
        _CapabilityRow(
          label: 'Shell actions',
          available: ready.capabilities.shellActions,
          offIsNormal: true,
        ),
      ],
    );
  }
}

/// Shown when the engine reports it cannot inject anything.
///
/// On Windows this should never appear — `SendInput` needs no permission. It exists for macOS
/// (Accessibility not granted) and Linux (`/dev/uinput` not writable), whose backends arrive in
/// M7 along with the specific remediation each one needs.
class _InputUnavailableBanner extends StatelessWidget {
  const _InputUnavailableBanner();

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
              onPressed: () => ref.read(adminSessionProvider.notifier).connect(),
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
            available ? Icons.check_circle_outline : Icons.remove_circle_outline,
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
              if (busy) const CircularProgressIndicator() else Icon(icon, size: 48),
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
