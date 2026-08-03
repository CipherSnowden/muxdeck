/// mDNS discovery of MuxDeck hosts. `docs/ARCHITECTURE.md` §6.
library;

import 'dart:async';

import 'package:bonsoir/bonsoir.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../data/hosts/host_record.dart';
import '../../providers.dart';

/// The service type to browse for.
///
/// **No `.local.` suffix.** The engine advertises `_muxdeck._tcp.local.` on the wire and that is
/// correct; the platform appends the domain itself. bonsoir's normalizer silently rewrites a
/// fully-qualified type to a default service type, after which discovery finds nothing and
/// reports no error. See `docs/CLIENT.md` §4.1.
const muxdeckServiceType = '_muxdeck._tcp';

/// TXT record keys the engine publishes. `docs/ARCHITECTURE.md` §6.
const _txtHostId = 'id';
const _txtHostName = 'name';
const _txtFingerprint = 'fp';

/// A host found on the network, whether or not this device has paired with it.
class DiscoveredHost {
  const DiscoveredHost({
    required this.hostId,
    required this.hostName,
    required this.address,
    required this.fingerprint,
    this.paired,
  });

  final String hostId;
  final String hostName;

  /// `<ip>:<port>`.
  final String address;

  /// From the TXT record. Lets a paired client confirm it is talking to the same host even
  /// after the IP changed — which is why an address change needs no re-pairing.
  final String fingerprint;

  /// The stored record, when this host has been paired with before.
  final HostRecord? paired;

  bool get isPaired => paired != null;

  /// True when this host's certificate no longer matches what was stored at pairing time.
  ///
  /// Surfaced before connecting so the user gets "this host's identity changed" rather than a
  /// TLS error after a spinner.
  bool get fingerprintChanged =>
      paired != null &&
      paired!.fingerprint.toLowerCase() != fingerprint.toLowerCase();
}

/// What the connect screen renders.
class DiscoveryState {
  const DiscoveryState({
    this.hosts = const [],
    this.isScanning = false,
    this.unsupported = false,
  });

  final List<DiscoveredHost> hosts;
  final bool isScanning;

  /// True when mDNS could not be started at all — a desktop test host, or a platform without it.
  final bool unsupported;

  /// True when a scan has run and found nothing.
  ///
  /// Distinct from "still scanning": `docs/CLIENT.md` §6 requires "no hosts found" to be a
  /// stated outcome with a suggested fix, never an indefinite spinner.
  bool get isEmptyAfterScan => !isScanning && hosts.isEmpty;
}

/// Browses for hosts and merges the results with previously paired ones.
class DiscoveryController extends Notifier<DiscoveryState> {
  List<HostRecord> _pairedHosts() => ref.read(pairedHostsProvider);

  BonsoirDiscovery? _discovery;
  StreamSubscription<BonsoirDiscoveryEvent>? _subscription;

  /// Keyed by host ID so a host re-announcing itself updates in place.
  final _found = <String, DiscoveredHost>{};

  @override
  DiscoveryState build() {
    ref.onDispose(_stop);
    return DiscoveryState(hosts: _mergedWithPaired());
  }

  /// Starts a scan, replacing any in progress.
  ///
  /// A fresh [BonsoirDiscovery] every time is required, not defensive: `stop()` is terminal in
  /// bonsoir 7.x and `start()` asserts on a stopped instance.
  Future<void> scan() async {
    await _stop();
    _found.clear();
    state = DiscoveryState(hosts: _mergedWithPaired(), isScanning: true);

    final discovery = BonsoirDiscovery(type: muxdeckServiceType);
    _discovery = discovery;

    try {
      await discovery.initialize();
    } catch (_) {
      // No mDNS here — a desktop test host, or a platform that cannot browse. Manual address
      // entry remains available, so this is a degraded mode rather than a failure.
      state = DiscoveryState(hosts: _mergedWithPaired(), unsupported: true);
      return;
    }

    // Listen before starting, or the first announcements are missed.
    _subscription = discovery.eventStream?.listen(_onEvent);
    await discovery.start();
  }

  void _onEvent(BonsoirDiscoveryEvent event) {
    switch (event) {
      case BonsoirDiscoveryServiceFoundEvent():
        // A `found` event carries no addresses; resolving is what produces them.
        event.service.resolve(_discovery!.serviceResolver);

      case BonsoirDiscoveryServiceResolvedEvent():
        final host = _toHost(event.service);
        if (host != null) {
          _found[host.hostId] = host;
          _publish();
        }

      case BonsoirDiscoveryServiceLostEvent():
        final hostId = event.service.attributes[_txtHostId];
        if (hostId != null && _found.remove(hostId) != null) _publish();

      default:
        break;
    }
  }

  /// Builds a [DiscoveredHost] from a resolved service, or null if the TXT records are
  /// incomplete.
  ///
  /// Incomplete records mean something else is squatting the service type; ignoring it is
  /// better than showing a host that cannot be connected to.
  DiscoveredHost? _toHost(BonsoirService service) {
    final hostId = service.attributes[_txtHostId];
    final fingerprint = service.attributes[_txtFingerprint];
    final address = service.hostAddresses.isNotEmpty
        ? service.hostAddresses.first
        : null;

    if (hostId == null || fingerprint == null || address == null) return null;

    final paired = _pairedHosts().where((h) => h.hostId == hostId).firstOrNull;

    return DiscoveredHost(
      hostId: hostId,
      hostName: service.attributes[_txtHostName] ?? service.name,
      address: '$address:${service.port}',
      fingerprint: fingerprint,
      paired: paired,
    );
  }

  void _publish() =>
      state = DiscoveryState(hosts: _mergedWithPaired(), isScanning: true);

  /// Discovered hosts first, then paired hosts not currently visible.
  ///
  /// De-duplicated by host ID, which is exact string equality: the mDNS TXT `id` record carries
  /// the same `h_…` string the protocol uses, so there is no second representation to normalise
  /// (`docs/ARCHITECTURE.md` §6).
  List<DiscoveredHost> _mergedWithPaired() {
    final merged = <String, DiscoveredHost>{..._found};

    for (final record in _pairedHosts()) {
      merged.putIfAbsent(
        record.hostId,
        () => DiscoveredHost(
          hostId: record.hostId,
          hostName: record.hostName,
          address: record.address,
          fingerprint: record.fingerprint,
          paired: record,
        ),
      );
    }

    final hosts = merged.values.toList()
      ..sort((a, b) {
        // Paired hosts first — they are what the user is most likely reaching for.
        if (a.isPaired != b.isPaired) return a.isPaired ? -1 : 1;
        return a.hostName.toLowerCase().compareTo(b.hostName.toLowerCase());
      });
    return hosts;
  }

  Future<void> stop() async {
    await _stop();
    state = DiscoveryState(hosts: _mergedWithPaired());
  }

  Future<void> _stop() async {
    await _subscription?.cancel();
    _subscription = null;
    final discovery = _discovery;
    _discovery = null;
    if (discovery != null && !discovery.isStopped) {
      try {
        await discovery.stop();
      } catch (_) {
        // Already gone; nothing to release.
      }
    }
  }
}
