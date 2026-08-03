/// Reconnect behaviour. `docs/CLIENT.md` §7.
library;

import 'dart:math';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_client/data/hosts/host_record.dart';
import 'package:muxdeck_client/data/hosts/host_store.dart';
import 'package:muxdeck_client/domain/session/session_controller.dart';
import 'package:muxdeck_client/domain/session/session_state.dart';
import 'package:muxdeck_client/providers.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'package:shared_preferences/shared_preferences.dart';

import 'support/fake_engine.dart';

/// A random that always returns the middle of the range, so the base delay comes out unjittered.
class _NoJitter implements Random {
  @override
  bool nextBool() => false;

  @override
  double nextDouble() => 0.5;

  @override
  int nextInt(int max) => 0;
}

const _host = HostRecord(
  hostId: 'h_0000000000000000',
  hostName: 'ENIGMA-ENTROPY',
  address: '192.168.1.99:47654',
  fingerprint: fakeFingerprint,
  deviceId: 'd_0000000000000000',
);

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  group('backoff schedule', () {
    test('doubles from 500ms and caps at 8s', () {
      final random = _NoJitter();
      final delays = [
        for (var attempt = 0; attempt < 5; attempt++)
          reconnectDelay(attempt, random: random).inMilliseconds,
      ];
      expect(delays, [500, 1000, 2000, 4000, 8000]);
    });

    test('stays at the cap rather than growing without bound', () {
      // A phone left on a table through an overnight outage must still be trying every eight
      // seconds in the morning, not every four hours.
      final random = _NoJitter();
      expect(reconnectDelay(20, random: random).inMilliseconds, 8000);
      expect(reconnectDelay(1000, random: random).inMilliseconds, 8000);
    });

    test('jitter stays inside a quarter either side', () {
      // The point is that a room of decks that lost the same access point does not come back in
      // lockstep — but the spread must not be so wide that the first retry takes seconds.
      for (var i = 0; i < 200; i++) {
        final delay = reconnectDelay(0).inMilliseconds;
        expect(delay, inInclusiveRange(375, 625));
      }
    });

    test('the schedule is the one the spec names', () {
      expect(reconnectBackoffMs, [500, 1000, 2000, 4000, 8000]);
      expect(pingInterval, const Duration(seconds: 5));
      expect(missedPingsBeforeReconnect, 3);
    });
  });

  group('retrying', () {
    late ProviderContainer container;

    Future<ProviderContainer> build(
      Transport Function(HostRecord) factory,
    ) async {
      SharedPreferences.setMockInitialValues({});
      FlutterSecureStorage.setMockInitialValues({});
      final hostStore = await HostStore.open();

      return ProviderContainer(
        overrides: [
          hostStoreProvider.overrideWithValue(hostStore),
          transportFactoryProvider.overrideWithValue(factory),
        ],
      );
    }

    tearDown(() => container.dispose());

    test('a network failure schedules a retry', () async {
      container = await build(
        (_) => const FailingTransport(HostUnreachable('ENIGMA-ENTROPY')),
      );

      await container.read(sessionProvider.notifier).connect(_host);

      final state = container.read(sessionProvider) as SessionFailed;
      expect(state.error, isA<HostUnreachable>());
      expect(
        state.willRetry,
        isTrue,
        reason:
            'a host that is merely down will come back; the deck should be waiting',
      );
    });

    test('an unpaired device does not retry for ever', () async {
      // Retrying cannot help: the host will refuse the same key just as fast every time, and
      // spinning would bury the one message that says what to do.
      container = await build((_) => const FailingTransport(NotPaired()));

      await container.read(sessionProvider.notifier).connect(_host);

      final state = container.read(sessionProvider) as SessionFailed;
      expect(state.error, isA<NotPaired>());
      expect(state.willRetry, isFalse);
    });

    test('disconnecting stops the retries', () async {
      container = await build(
        (_) => const FailingTransport(HostUnreachable('ENIGMA-ENTROPY')),
      );

      await container.read(sessionProvider.notifier).connect(_host);
      expect(
        (container.read(sessionProvider) as SessionFailed).willRetry,
        isTrue,
      );

      await container.read(sessionProvider.notifier).disconnect();
      expect(container.read(sessionProvider), isA<SessionDisconnected>());
    });

    test('resuming with nothing to reconnect to does nothing', () async {
      // The lifecycle hook fires on every resume, including before the user has ever connected.
      container = await build(
        (_) => const FailingTransport(HostUnreachable('ENIGMA-ENTROPY')),
      );

      await container.read(sessionProvider.notifier).reconnectNow();
      expect(container.read(sessionProvider), isA<SessionDisconnected>());
    });

    test('a resume after a drop reconnects at once', () async {
      final engine = await FakeEngine.create();

      // The first attempt finds the host down; the second finds it back. A real backoff of
      // seconds sits between them, and resume is what skips it.
      var attempts = 0;
      container = await build((_) {
        attempts++;
        return attempts == 1
            ? const FailingTransport(HostUnreachable('ENIGMA-ENTROPY'))
            : engine;
      });

      final identity = await container.read(deviceIdentityStoreProvider).load();
      final deviceId = engine.registerDevice(identity.publicKey);

      final host = HostRecord(
        hostId: engine.hostId,
        hostName: 'ENIGMA-ENTROPY',
        address: '192.168.1.99:47654',
        fingerprint: fakeFingerprint,
        deviceId: deviceId,
      );

      await container.read(sessionProvider.notifier).connect(host);
      expect(container.read(sessionProvider), isA<SessionFailed>());

      await container.read(sessionProvider.notifier).reconnectNow();

      expect(
        container.read(sessionProvider),
        isA<SessionReady>(),
        reason: 'resume must skip the backoff, not join the queue behind it',
      );
    });
  });
}
