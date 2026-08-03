/// Host list, discovery and manual entry.
library;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../domain/discovery/discovery_controller.dart';
import '../../providers.dart';
import '../deck/deck_page.dart';
import '../pairing/pairing_page.dart';

class ConnectPage extends ConsumerStatefulWidget {
  const ConnectPage({super.key});

  @override
  ConsumerState<ConnectPage> createState() => _ConnectPageState();
}

class _ConnectPageState extends ConsumerState<ConnectPage> {
  @override
  void initState() {
    super.initState();
    // Scan on open. Deferred past the first frame because a Notifier cannot be modified while
    // the widget tree is still building.
    WidgetsBinding.instance.addPostFrameCallback((_) {
      ref.read(discoveryProvider.notifier).scan();
    });
  }

  @override
  Widget build(BuildContext context) {
    final discovery = ref.watch(discoveryProvider);

    ref.listen(sessionProvider, (previous, next) {
      if (next.isReady && mounted) {
        Navigator.of(
          context,
        ).push(MaterialPageRoute<void>(builder: (_) => const DeckPage()));
      }
    });

    return Scaffold(
      backgroundColor: const Color(0xFF12141A),
      appBar: AppBar(
        backgroundColor: const Color(0xFF1A1D26),
        foregroundColor: Colors.white,
        title: const Text('MuxDeck'),
        actions: [
          IconButton(
            icon: const Icon(Icons.refresh),
            tooltip: 'Scan again',
            onPressed: () => ref.read(discoveryProvider.notifier).scan(),
          ),
        ],
      ),
      floatingActionButton: FloatingActionButton.extended(
        onPressed: _openPairing,
        icon: const Icon(Icons.qr_code_scanner),
        label: const Text('Pair a device'),
      ),
      body: _body(discovery),
    );
  }

  Widget _body(DiscoveryState discovery) {
    if (discovery.hosts.isEmpty) {
      // Never an indefinite spinner. `docs/CLIENT.md` §6 requires each failure to say what to
      // try, because "searching…" forever and "nothing is there" need different reactions from
      // the user.
      if (discovery.isScanning) {
        return const _Message(
          icon: Icons.wifi_tethering,
          title: 'Looking for hosts…',
          detail: 'Searching the local network for computers running MuxDeck.',
          busy: true,
        );
      }
      if (discovery.unsupported) {
        return const _Message(
          icon: Icons.error_outline,
          title: 'Automatic discovery is unavailable',
          detail:
              'Enter the address shown on your computer manually to connect.',
        );
      }
      return const _Message(
        icon: Icons.search_off,
        title: 'No MuxDeck hosts found',
        detail:
            'Check that the desktop app is running, and that this device is on the '
            'same Wi-Fi network — not a guest network.',
      );
    }

    return ListView.separated(
      padding: const EdgeInsets.fromLTRB(12, 12, 12, 96),
      itemCount: discovery.hosts.length,
      separatorBuilder: (_, _) => const SizedBox(height: 8),
      itemBuilder: (context, index) => _HostTile(
        host: discovery.hosts[index],
        onTap: () => _connect(discovery.hosts[index]),
      ),
    );
  }

  Future<void> _connect(DiscoveredHost host) async {
    final paired = host.paired;

    if (paired == null) {
      // Unpaired: there is no stored fingerprint, so the QR is the only way to learn one out of
      // band. Sending them to the scanner is the honest answer.
      await _openPairing();
      return;
    }

    if (host.fingerprintChanged) {
      // Caught before connecting, so the user is told their host's identity changed rather than
      // shown a TLS error after a spinner.
      _showIdentityChanged(host);
      return;
    }

    // Reconnect at the address just discovered: an IP change must not require re-pairing, which
    // is exactly what the engine's immutable certificate guarantees.
    await ref
        .read(sessionProvider.notifier)
        .connect(
          paired.copyWith(address: host.address, hostName: host.hostName),
        );
  }

  void _showIdentityChanged(DiscoveredHost host) {
    showDialog<void>(
      context: context,
      builder: (context) => AlertDialog(
        title: const Text("This host's identity has changed"),
        content: Text(
          '${host.hostName} is presenting a different certificate than when you paired with it.\n\n'
          'This happens if the host was reset — but it can also mean something else is '
          'impersonating it. Pair again only if you reset it yourself.',
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          FilledButton(
            onPressed: () {
              Navigator.of(context).pop();
              _openPairing();
            },
            child: const Text('Pair again'),
          ),
        ],
      ),
    );
  }

  Future<void> _openPairing() async {
    await Navigator.of(
      context,
    ).push(MaterialPageRoute<void>(builder: (_) => const PairingPage()));
    if (mounted) {
      ref.invalidate(pairedHostsProvider);
      await ref.read(discoveryProvider.notifier).scan();
    }
  }
}

class _HostTile extends StatelessWidget {
  const _HostTile({required this.host, required this.onTap});

  final DiscoveredHost host;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final (icon, colour, subtitle) = switch (host) {
      _ when host.fingerprintChanged => (
        Icons.gpp_maybe,
        const Color(0xFFB3422F),
        'Identity changed — re-pair to continue',
      ),
      _ when host.isPaired => (
        Icons.computer,
        const Color(0xFF1F8A70),
        host.address,
      ),
      _ => (
        Icons.devices_other,
        const Color(0xFF4A5568),
        '${host.address} · not paired',
      ),
    };

    return Material(
      color: const Color(0xFF1A1D26),
      borderRadius: BorderRadius.circular(12),
      child: ListTile(
        shape: RoundedRectangleBorder(borderRadius: BorderRadius.circular(12)),
        leading: CircleAvatar(
          backgroundColor: colour.withValues(alpha: 0.2),
          child: Icon(icon, color: colour),
        ),
        title: Text(host.hostName, style: const TextStyle(color: Colors.white)),
        subtitle: Text(subtitle, style: const TextStyle(color: Colors.white54)),
        trailing: const Icon(Icons.chevron_right, color: Colors.white38),
        onTap: onTap,
      ),
    );
  }
}

class _Message extends StatelessWidget {
  const _Message({
    required this.icon,
    required this.title,
    required this.detail,
    this.busy = false,
  });

  final IconData icon;
  final String title;
  final String detail;
  final bool busy;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.all(32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (busy)
              const CircularProgressIndicator()
            else
              Icon(icon, size: 48, color: Colors.white38),
            const SizedBox(height: 20),
            Text(
              title,
              textAlign: TextAlign.center,
              style: Theme.of(
                context,
              ).textTheme.titleMedium?.copyWith(color: Colors.white),
            ),
            const SizedBox(height: 8),
            Text(
              detail,
              textAlign: TextAlign.center,
              style: Theme.of(
                context,
              ).textTheme.bodyMedium?.copyWith(color: Colors.white54),
            ),
          ],
        ),
      ),
    );
  }
}
