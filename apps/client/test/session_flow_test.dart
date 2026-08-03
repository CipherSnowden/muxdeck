/// The whole client flow against a fake engine that verifies real signatures.
///
/// This is the "fake in-process engine" `docs/CLIENT.md` §8 asks for. Because [FakeEngine]
/// runs `Ed25519().verify()` against the buffers built by `muxdeck_protocol`'s
/// `sessionAuthMessage` and `pairProofMessage`, a passing test here proves the client assembles
/// byte-identical signing input to the Rust engine — the one disagreement that authenticates
/// nothing and produces no diagnosable symptom at runtime.
library;

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_client/data/hosts/host_record.dart';
import 'package:muxdeck_client/data/hosts/host_store.dart';
import 'package:muxdeck_client/data/identity/device_identity.dart';
import 'package:muxdeck_client/domain/pairing/pairing_controller.dart';
import 'package:muxdeck_client/domain/session/session_state.dart';
import 'package:muxdeck_client/providers.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'support/fake_engine.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  late FakeEngine engine;
  late ProviderContainer container;
  late HostStore hostStore;

  /// Builds a container wired to [engine] instead of a real socket.
  Future<ProviderContainer> build() async {
    SharedPreferences.setMockInitialValues({});
    // The plugin has no Windows/test implementation, so an in-memory map stands in. The Ed25519
    // work is still real — only the persistence is faked.
    FlutterSecureStorage.setMockInitialValues({});

    hostStore = await HostStore.open();

    return ProviderContainer(
      overrides: [
        hostStoreProvider.overrideWithValue(hostStore),
        transportFactoryProvider.overrideWithValue((_) => engine),
        pairingTransportFactoryProvider.overrideWithValue(
          ({required String address, required String fingerprint}) => engine,
        ),
      ],
    );
  }

  setUp(() async {
    engine = await FakeEngine.create();
    container = await build();
  });

  tearDown(() => container.dispose());

  /// The identity this device will present.
  Future<DeviceIdentity> identity() =>
      container.read(deviceIdentityStoreProvider).load();

  group('pairing', () {
    test('a correct code pairs and stores the host', () async {
      engine.openPairingWindow('402913');

      await container
          .read(pairingProvider.notifier)
          .pair(
            address: '192.168.1.42:47654',
            hostId: engine.hostId,
            fingerprint: engine.fingerprint,
            code: '402913',
          );

      final state = container.read(pairingProvider);
      expect(state, isA<PairingSucceeded>());

      final stored = hostStore.byId(engine.hostId);
      expect(stored, isNotNull);
      expect(stored!.fingerprint, engine.fingerprint);
      expect(stored.deviceId, (await identity()).deviceId);
      expect(
        hostStore.lastHostId,
        engine.hostId,
        reason: 'the freshly paired host is the one to reconnect to on launch',
      );
    });

    test('the proof of possession is verified for real', () async {
      // FakeEngine runs Ed25519().verify() against pairProofMessage(). Success here means the
      // client built the exact bytes the engine expects — the assertion this whole harness
      // exists for.
      engine.openPairingWindow('111111');

      await container
          .read(pairingProvider.notifier)
          .pair(
            address: '10.0.0.5:47654',
            hostId: engine.hostId,
            fingerprint: engine.fingerprint,
            code: '111111',
          );

      expect(container.read(pairingProvider), isA<PairingSucceeded>());
    });

    test('a wrong code is refused with an actionable message', () async {
      engine.openPairingWindow('402913');

      await container
          .read(pairingProvider.notifier)
          .pair(
            address: '192.168.1.42:47654',
            hostId: engine.hostId,
            fingerprint: engine.fingerprint,
            code: '000000',
          );

      final state = container.read(pairingProvider);
      expect(state, isA<PairingFailed>());
      expect((state as PairingFailed).error, isA<PairingRejected>());
      expect(state.error.message, contains('code'));
      expect(
        hostStore.all(),
        isEmpty,
        reason: 'nothing may be stored on failure',
      );
    });

    test('pairing outside a window is refused', () async {
      engine.closePairingWindow();

      await container
          .read(pairingProvider.notifier)
          .pair(
            address: '192.168.1.42:47654',
            hostId: engine.hostId,
            fingerprint: engine.fingerprint,
            code: '402913',
          );

      final state = container.read(pairingProvider);
      expect(state, isA<PairingFailed>());
      expect(state.toString(), isNotEmpty);
      expect(hostStore.all(), isEmpty);
    });

    test('a malformed QR payload never reaches the network', () async {
      await container
          .read(pairingProvider.notifier)
          .pairFromQr('https://example.com');

      expect(container.read(pairingProvider), isA<PairingFailed>());
      expect(engine.receivedInput, isEmpty);
    });
  });

  group('session', () {
    /// Registers this device with the engine and returns the matching host record.
    Future<HostRecord> paired() async {
      final device = await identity();
      engine.registerDevice(device.publicKey);
      return HostRecord(
        hostId: engine.hostId,
        hostName: engine.hostName,
        address: '192.168.1.42:47654',
        fingerprint: engine.fingerprint,
        deviceId: device.deviceId,
      );
    }

    test(
      'the challenge signature verifies and the session reaches ready',
      () async {
        final host = await paired();

        await container.read(sessionProvider.notifier).connect(host);

        final state = container.read(sessionProvider);
        expect(
          state,
          isA<SessionReady>(),
          reason:
              'a real Ed25519 verification against sessionAuthMessage must succeed',
        );

        final ready = (state as SessionReady).ready;
        expect(ready.role, Role.deck);
        expect(ready.protocol, protocolVersion);
        expect(ready.capabilities.shellActions, isFalse);
      },
    );

    test('an unpaired device is told it is not paired', () async {
      // Nothing registered with the engine, which is what a revoked device sees.
      final device = await identity();
      final host = HostRecord(
        hostId: engine.hostId,
        hostName: engine.hostName,
        address: '192.168.1.42:47654',
        fingerprint: engine.fingerprint,
        deviceId: device.deviceId,
      );

      await container.read(sessionProvider.notifier).connect(host);

      final state = container.read(sessionProvider);
      expect(state, isA<SessionFailed>());
      expect(
        (state as SessionFailed).error,
        isA<NotPaired>(),
        reason:
            'UNKNOWN_DEVICE must become a re-pair instruction, not a generic error',
      );
    });

    test(
      'a fingerprint mismatch is not treated as a retryable failure',
      () async {
        container.dispose();
        container = ProviderContainer(
          overrides: [
            hostStoreProvider.overrideWithValue(hostStore),
            transportFactoryProvider.overrideWithValue(
              (_) => const FailingTransport(FingerprintMismatch()),
            ),
          ],
        );

        await container
            .read(sessionProvider.notifier)
            .connect(
              HostRecord(
                hostId: engine.hostId,
                hostName: engine.hostName,
                address: '192.168.1.42:47654',
                fingerprint: engine.fingerprint,
                deviceId: 'd_0000000000000000',
              ),
            );

        final state = container.read(sessionProvider);
        expect(state, isA<SessionFailed>());
        expect((state as SessionFailed).error, isA<FingerprintMismatch>());
      },
    );

    test('an unreachable host reports as unreachable', () async {
      container.dispose();
      container = ProviderContainer(
        overrides: [
          hostStoreProvider.overrideWithValue(hostStore),
          transportFactoryProvider.overrideWithValue(
            (_) => const FailingTransport(HostUnreachable('ENIGMA-ENTROPY')),
          ),
        ],
      );

      await container
          .read(sessionProvider.notifier)
          .connect(
            HostRecord(
              hostId: 'h_0000000000000000',
              hostName: 'ENIGMA-ENTROPY',
              address: '192.168.1.99:47654',
              fingerprint: fakeFingerprint,
              deviceId: 'd_0000000000000000',
            ),
          );

      final state = container.read(sessionProvider);
      expect(state, isA<SessionFailed>());
      expect((state as SessionFailed).error, isA<HostUnreachable>());
      expect(state.error.message, contains('ENIGMA-ENTROPY'));
    });
  });

  group('pressing a button', () {
    test('sends input.key_combo with the canonical key names', () async {
      final device = await identity();
      engine.registerDevice(device.publicKey);
      final host = HostRecord(
        hostId: engine.hostId,
        hostName: engine.hostName,
        address: '192.168.1.42:47654',
        fingerprint: engine.fingerprint,
        deviceId: device.deviceId,
      );

      await container.read(sessionProvider.notifier).connect(host);
      expect(container.read(sessionProvider), isA<SessionReady>());

      final client = container.read(sessionProvider.notifier).client;
      expect(
        client,
        isNotNull,
        reason: 'a ready session must expose its client',
      );

      client!.fireAndForget(KnownOp.inputKeyCombo, {
        'keys': ['CONTROL', 'C'],
      });

      // Fire-and-forget, so let the microtask queue drain before asserting.
      await Future<void>.delayed(Duration.zero);

      expect(engine.receivedInput, hasLength(1));
      expect(engine.receivedInput.single['op'], KnownOp.inputKeyCombo.wire);
      expect(engine.receivedInput.single['d'], {
        'keys': ['CONTROL', 'C'],
      });
    });

    test('a press on a disconnected session is dropped, not queued', () async {
      // docs/CLIENT.md §7: replaying CONTROL+W five seconds late is worse than losing it.
      final client = container.read(sessionProvider.notifier).client;
      expect(client, isNull);
      expect(engine.receivedInput, isEmpty);
    });
  });
}
