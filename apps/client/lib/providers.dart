/// The provider graph.
///
/// Hand-written, no codegen, and **modern providers only** — `Notifier`/`AsyncNotifier` and the
/// read-only `Provider`/`FutureProvider` family. Never `StateProvider`,
/// `StateNotifierProvider` or `ChangeNotifierProvider`; those live behind a separate
/// `legacy.dart` import that this file must never reach for. See `docs/CLIENT.md` §2.
///
/// Notifiers read their dependencies through `ref` rather than through constructors, because a
/// `Notifier` is constructed before its `ref` exists. Tests substitute fakes by overriding the
/// providers below, which is also how the whole client is tested without a socket.
library;

import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import 'data/hosts/host_record.dart';
import 'data/hosts/host_store.dart';
import 'data/identity/device_identity.dart';
import 'domain/discovery/discovery_controller.dart';
import 'domain/pairing/pairing_controller.dart';
import 'domain/session/session_controller.dart';
import 'domain/session/session_state.dart';

/// Overridden in `main()` once `SharedPreferences` has loaded, and in tests with a fake.
///
/// Throwing by default is deliberate: a provider silently returning an empty store would make
/// "my hosts disappeared" a plausible bug report instead of a startup crash in development.
final hostStoreProvider = Provider<HostStore>(
  (ref) => throw UnimplementedError('hostStoreProvider must be overridden in main()'),
);

final deviceIdentityStoreProvider = Provider<DeviceIdentityStore>(
  (ref) => DeviceIdentityStore(),
);

/// Builds the transport for a paired host. Overridden in tests with a fake.
final transportFactoryProvider = Provider<TransportFactory>(
  (ref) => (host) => LanTransport(
    uri: host.websocketUri,
    expectedFingerprint: host.fingerprint,
    hostName: host.hostName,
  ),
);

/// Builds the transport used during pairing, before a host record exists.
final pairingTransportFactoryProvider = Provider<PairingTransportFactory>(
  (ref) => ({required String address, required String fingerprint}) => LanTransport(
    uri: Uri.parse('wss://$address/ws'),
    expectedFingerprint: fingerprint,
  ),
);

/// The paired hosts on disk.
///
/// `ref.invalidate(pairedHostsProvider)` after pairing or unpairing; the store is the source of
/// truth and this is only a view of it.
final pairedHostsProvider = Provider<List<HostRecord>>(
  (ref) => ref.watch(hostStoreProvider).all(),
);

final sessionProvider = NotifierProvider<SessionController, SessionState>(
  SessionController.new,
);

final discoveryProvider = NotifierProvider<DiscoveryController, DiscoveryState>(
  DiscoveryController.new,
);

final pairingProvider = NotifierProvider<PairingController, PairingFlowState>(
  PairingController.new,
);
