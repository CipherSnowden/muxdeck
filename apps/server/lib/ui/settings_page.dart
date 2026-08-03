/// Engine settings. `docs/SERVER.md` §6, `docs/PROTOCOL.md` §4.6.
library;

import 'dart:async';
// Prefixed: muxdeck_protocol exports its own `Platform` enum, which otherwise collides.
import 'dart:io' as io;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../data/engine_locator.dart';
import '../providers.dart';

class SettingsPage extends ConsumerStatefulWidget {
  const SettingsPage({super.key});

  @override
  ConsumerState<SettingsPage> createState() => _SettingsPageState();
}

class _SettingsPageState extends ConsumerState<SettingsPage> {
  final _hostName = TextEditingController();
  final _port = TextEditingController();

  /// The values last loaded, so a save can send only what the user actually changed.
  Settings? _loaded;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addPostFrameCallback((_) {
      if (mounted) unawaited(ref.read(settingsProvider.notifier).load());
    });
  }

  @override
  void dispose() {
    _hostName.dispose();
    _port.dispose();
    super.dispose();
  }

  /// Fills the text fields the first time settings arrive, and after a save.
  ///
  /// Guarded on the value having changed rather than done unconditionally: overwriting a field
  /// the user is mid-way through typing into is the classic way a settings form eats input.
  void _syncFields(Settings settings) {
    if (_loaded == settings) return;
    _loaded = settings;
    _hostName.text = settings.hostName;
    _port.text = '${settings.port}';
  }

  Future<void> _save(SettingsPatch patch) =>
      ref.read(settingsProvider.notifier).save(patch);

  @override
  Widget build(BuildContext context) {
    final state = ref.watch(settingsProvider);
    final settings = state.settings;

    if (settings == null) {
      return Center(
        child: state.error != null
            ? Text(state.error!)
            : const CircularProgressIndicator(),
      );
    }
    _syncFields(settings);

    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        if (state.restartRequired) const _RestartBanner(),
        if (state.error != null) _ErrorBanner(message: state.error!),

        const _SectionHeader('Network'),
        _TextSetting(
          controller: _hostName,
          label: 'Name shown to devices',
          helper: 'Re-advertises over mDNS at once. No restart needed.',
          onSubmitted: (value) => _save(SettingsPatch(hostName: value)),
        ),
        _TextSetting(
          controller: _port,
          label: 'Port',
          helper: 'Takes effect when the engine restarts.',
          keyboardType: TextInputType.number,
          onSubmitted: (value) {
            final port = int.tryParse(value);
            if (port != null) _save(SettingsPatch(port: port));
          },
        ),

        const _SectionHeader('Telemetry'),
        SwitchListTile(
          title: const Text('Report CPU and memory'),
          subtitle: const Text(
            'Shown on the dashboard and to any deck that asks for it.',
          ),
          value: settings.telemetryEnabled,
          onChanged: state.saving
              ? null
              : (value) => _save(SettingsPatch(telemetryEnabled: value)),
        ),
        ListTile(
          title: const Text('Sample every'),
          subtitle: Text('${settings.telemetryIntervalMs} ms'),
          trailing: SizedBox(
            width: 260,
            child: Slider(
              min: 200,
              max: 5000,
              divisions: 24,
              value: settings.telemetryIntervalMs.clamp(200, 5000).toDouble(),
              label: '${settings.telemetryIntervalMs} ms',
              onChanged: state.saving
                  ? null
                  : (value) => _save(
                      SettingsPatch(telemetryIntervalMs: value.round()),
                    ),
            ),
          ),
        ),

        const _SectionHeader('Startup'),
        SwitchListTile(
          title: const Text('Start MuxDeck when I log in'),
          subtitle: const Text(
            'Registers a startup entry for this user. The engine runs in your desktop '
            'session, which is what lets it type.',
          ),
          value: settings.autostart,
          onChanged: state.saving
              ? null
              : (value) => _save(SettingsPatch(autostart: value)),
        ),

        const _SectionHeader('Shell actions'),
        _ShellActionsSwitch(enabled: settings.shellActionsEnabled),

        const _SectionHeader('Files'),
        const _ConfigFolderTile(),
      ],
    );
  }
}

/// The one setting that needs a warning rather than a subtitle.
class _ShellActionsSwitch extends ConsumerWidget {
  const _ShellActionsSwitch({required this.enabled});

  final bool enabled;

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SwitchListTile(
          title: const Text('Let devices run shell actions'),
          subtitle: const Text(
            'Off by default. Turning this on lets any paired device run every action you '
            'have defined.',
          ),
          value: enabled,
          onChanged: (value) async {
            if (value && !await _confirm(context)) return;
            await ref
                .read(settingsProvider.notifier)
                .save(SettingsPatch(shellActionsEnabled: value));
          },
        ),
        if (enabled)
          const Padding(
            padding: EdgeInsets.fromLTRB(16, 0, 16, 8),
            child: Text(
              'Actions are defined here on this computer, and a device sends only their name — '
              'never a command. Nothing is passed to a shell interpreter.',
            ),
          ),
      ],
    );
  }

  /// An unambiguous warning before enabling, as `docs/SERVER.md` §6 requires.
  Future<bool> _confirm(BuildContext context) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text('Let devices run programs on this computer?'),
        content: const Text(
          'Every paired device will be able to run any action you define here, at any time, '
          'without asking. Revoke a device from the Devices screen if you stop trusting it.\n\n'
          'Actions run as you, with your files and your permissions.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Enable'),
          ),
        ],
      ),
    );
    return confirmed ?? false;
  }
}

class _ConfigFolderTile extends StatelessWidget {
  const _ConfigFolderTile();

  @override
  Widget build(BuildContext context) {
    final directory = engineConfigDirectory();

    return ListTile(
      title: const Text('Configuration folder'),
      subtitle: Text(directory?.path ?? 'Not found'),
      trailing: const Icon(Icons.folder_open),
      onTap: directory == null ? null : () => _open(directory.path),
    );
  }

  /// Opens the folder in the platform's file manager.
  ///
  /// Three different commands rather than a package: this is the entire extent of the need, and
  /// a dependency for one `Process.run` is not worth its own supply chain.
  void _open(String path) {
    final (executable, args) = switch (io.Platform.operatingSystem) {
      'windows' => ('explorer', [path]),
      'macos' => ('open', [path]),
      _ => ('xdg-open', [path]),
    };
    // Deliberately not awaited and errors ignored: explorer.exe returns a non-zero exit code
    // even on success, and there is nothing useful to tell the user if it fails.
    unawaited(
      io.Process.run(executable, args).catchError((Object _) {
        return io.ProcessResult(0, 0, '', '');
      }),
    );
  }
}

class _RestartBanner extends StatelessWidget {
  const _RestartBanner();

  @override
  Widget build(BuildContext context) => _Banner(
    colour: const Color(0xFFD8A657),
    icon: Icons.restart_alt,
    message:
        'The port change takes effect when the engine restarts. Devices will need to find '
        'it again on the new port.',
  );
}

class _ErrorBanner extends StatelessWidget {
  const _ErrorBanner({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) => _Banner(
    colour: const Color(0xFFB3422F),
    icon: Icons.error_outline,
    message: message,
  );
}

class _Banner extends StatelessWidget {
  const _Banner({
    required this.colour,
    required this.icon,
    required this.message,
  });

  final Color colour;
  final IconData icon;
  final String message;

  @override
  Widget build(BuildContext context) {
    return Container(
      margin: const EdgeInsets.only(bottom: 16),
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: colour.withValues(alpha: 0.15),
        border: Border.all(color: colour),
        borderRadius: BorderRadius.circular(12),
      ),
      child: Row(
        children: [
          Icon(icon, color: colour),
          const SizedBox(width: 12),
          Expanded(child: Text(message)),
        ],
      ),
    );
  }
}

class _TextSetting extends StatelessWidget {
  const _TextSetting({
    required this.controller,
    required this.label,
    required this.helper,
    required this.onSubmitted,
    this.keyboardType,
  });

  final TextEditingController controller;
  final String label;
  final String helper;
  final ValueChanged<String> onSubmitted;
  final TextInputType? keyboardType;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 16, vertical: 8),
      child: TextField(
        controller: controller,
        keyboardType: keyboardType,
        decoration: InputDecoration(
          labelText: label,
          helperText: helper,
          border: const OutlineInputBorder(),
        ),
        // Saved on commit rather than on every keystroke: writing settings.set per character
        // would re-advertise mDNS a dozen times while somebody renames their computer.
        onSubmitted: onSubmitted,
        onEditingComplete: () => onSubmitted(controller.text),
      ),
    );
  }
}

class _SectionHeader extends StatelessWidget {
  const _SectionHeader(this.title);

  final String title;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    return Padding(
      padding: const EdgeInsets.fromLTRB(16, 24, 16, 4),
      child: Text(
        title.toUpperCase(),
        style: theme.textTheme.labelSmall?.copyWith(
          color: theme.colorScheme.primary,
          letterSpacing: 1.2,
        ),
      ),
    );
  }
}
