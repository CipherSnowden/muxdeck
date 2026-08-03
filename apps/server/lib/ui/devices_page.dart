/// Paired devices, and removing them.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../domain/admin_session.dart';
import '../providers.dart';
import 'pair_page.dart';

class DevicesPage extends ConsumerWidget {
  const DevicesPage({super.key});

  @override
  Widget build(BuildContext context, WidgetRef ref) {
    final state = ref.watch(adminSessionProvider);

    if (state is! AdminReady) {
      return const Center(
        child: Text('Connect to the engine to manage devices.'),
      );
    }

    if (state.devices.isEmpty) {
      return Center(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.devices_other, size: 48),
            const SizedBox(height: 16),
            Text(
              'No devices paired yet',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const SizedBox(height: 8),
            const Text(
              'Pair your phone or tablet to start using it as a deck.',
            ),
            const SizedBox(height: 24),
            FilledButton.icon(
              onPressed: () => _openPairing(context),
              icon: const Icon(Icons.add),
              label: const Text('Pair a device'),
            ),
          ],
        ),
      );
    }

    return ListView(
      padding: const EdgeInsets.all(24),
      children: [
        Row(
          children: [
            Text(
              'Paired devices',
              style: Theme.of(context).textTheme.titleLarge,
            ),
            const Spacer(),
            FilledButton.icon(
              onPressed: () => _openPairing(context),
              icon: const Icon(Icons.add),
              label: const Text('Pair a device'),
            ),
          ],
        ),
        const SizedBox(height: 16),
        for (final device in state.devices)
          _DeviceTile(
            device: device,
            onRevoke: () => _confirmRevoke(context, ref, device),
          ),
      ],
    );
  }

  void _openPairing(BuildContext context) {
    showDialog<void>(context: context, builder: (_) => const PairDialog());
  }

  /// Revoking is destructive and immediate — the engine drops the device's live socket the
  /// moment it happens — so it asks first.
  Future<void> _confirmRevoke(
    BuildContext context,
    WidgetRef ref,
    DeviceInfo device,
  ) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (context) => AlertDialog(
        title: Text('Remove ${device.name}?'),
        content: const Text(
          'That device will stop working immediately and will have to be paired again '
          'to reconnect.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(false),
            child: const Text('Cancel'),
          ),
          FilledButton(
            style: FilledButton.styleFrom(
              backgroundColor: const Color(0xFFB3422F),
            ),
            onPressed: () => Navigator.of(context).pop(true),
            child: const Text('Remove'),
          ),
        ],
      ),
    );

    if (confirmed != true || !context.mounted) return;

    try {
      await ref
          .read(adminSessionProvider.notifier)
          .revokeDevice(device.deviceId);
    } catch (e) {
      if (context.mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          SnackBar(content: Text('Could not remove the device: $e')),
        );
      }
    }
  }
}

class _DeviceTile extends StatelessWidget {
  const _DeviceTile({required this.device, required this.onRevoke});

  final DeviceInfo device;
  final VoidCallback onRevoke;

  @override
  Widget build(BuildContext context) {
    return Card(
      child: ListTile(
        leading: CircleAvatar(
          backgroundColor: device.connected
              ? const Color(0xFF1F8A70).withValues(alpha: 0.2)
              : Theme.of(context).colorScheme.surfaceContainerHighest,
          child: Icon(
            _iconFor(device.platform),
            color: device.connected ? const Color(0xFF1F8A70) : null,
          ),
        ),
        title: Text(device.name),
        subtitle: Text(
          device.connected
              ? 'Connected now'
              : 'Last seen ${_relative(device.lastSeen)}',
        ),
        trailing: IconButton(
          icon: const Icon(Icons.delete_outline),
          tooltip: 'Remove this device',
          onPressed: onRevoke,
        ),
      ),
    );
  }

  IconData _iconFor(Platform platform) => switch (platform) {
    Platform.ios => Icons.phone_iphone,
    Platform.android => Icons.phone_android,
    _ => Icons.computer,
  };

  /// Unix seconds to something a person reads without doing arithmetic.
  String _relative(int unixSeconds) {
    final then = DateTime.fromMillisecondsSinceEpoch(unixSeconds * 1000);
    final delta = DateTime.now().difference(then);

    if (delta.inMinutes < 1) return 'moments ago';
    if (delta.inHours < 1) return _plural(delta.inMinutes, 'minute');
    if (delta.inDays < 1) return _plural(delta.inHours, 'hour');
    if (delta.inDays < 30) return _plural(delta.inDays, 'day');
    return 'on ${then.year}-${then.month.toString().padLeft(2, '0')}-'
        '${then.day.toString().padLeft(2, '0')}';
  }

  String _plural(int count, String unit) =>
      '$count $unit${count == 1 ? '' : 's'} ago';
}
