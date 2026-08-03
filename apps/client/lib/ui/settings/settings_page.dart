/// Device-local settings. `docs/CLIENT.md` §6.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/identity/device_identity.dart';
import '../../domain/session/session_state.dart';
import '../../providers.dart';

class SettingsPage extends ConsumerWidget {
  const SettingsPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final settings = ref.watch(settingsProvider);
    final session = ref.watch(sessionProvider);

    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: ListView(
        children: [
          const _SectionHeader('This device'),
          const _DeviceIdTile(),

          const _SectionHeader('Host'),
          ListTile(
            title: const Text('Connected to'),
            subtitle: Text(switch (session) {
              SessionReady(:final hostName) => hostName,
              SessionConnecting(:final hostName) => '$hostName — connecting',
              SessionAuthenticating(:final hostName) =>
                '$hostName — authenticating',
              SessionFailed(:final error, :final willRetry) =>
                willRetry ? '${error.message} Retrying.' : error.message,
              SessionDisconnected() => 'Not connected',
            }),
          ),
          ListTile(
            title: const Text('Disconnect'),
            subtitle: const Text('Return to the host list. Stays paired.'),
            trailing: const Icon(Icons.logout),
            onTap: () async {
              await ref.read(sessionProvider.notifier).disconnect();
              if (context.mounted) Navigator.of(context).pop();
            },
          ),

          const _SectionHeader('Display'),
          SwitchListTile(
            title: const Text('Keep the screen awake'),
            subtitle: const Text('A deck that sleeps is not a deck.'),
            value: settings.keepScreenAwake,
            onChanged: (enabled) => ref
                .read(settingsProvider.notifier)
                .setKeepScreenAwake(enabled: enabled),
          ),
          SwitchListTile(
            title: const Text('Show round-trip time'),
            subtitle: const Text(
              'Latency in milliseconds, in the status chip.',
            ),
            value: settings.showRoundTrip,
            onChanged: (enabled) => ref
                .read(settingsProvider.notifier)
                .setShowRoundTrip(enabled: enabled),
          ),
        ],
      ),
    );
  }
}

/// The device ID, which is what a host shows in its device list.
///
/// Worth surfacing: when a user has three tablets paired and wants to revoke one, this string is
/// the only thing that tells them which row is which.
class _DeviceIdTile extends ConsumerWidget {
  const _DeviceIdTile();

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    return FutureBuilder<DeviceIdentity>(
      future: ref.read(deviceIdentityStoreProvider).load(),
      builder: (context, snapshot) => ListTile(
        title: const Text('Device ID'),
        subtitle: Text(snapshot.hasData ? snapshot.data!.deviceId : 'Loading…'),
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
      padding: const EdgeInsets.fromLTRB(16, 20, 16, 4),
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
