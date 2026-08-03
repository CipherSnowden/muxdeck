/// The real Dart client against a real running Rust engine.
///
/// Everything else in this suite is Dart talking to Dart. This is the only test where the
/// client's Ed25519 signature is verified by `ed25519-dalek` and its certificate pin is checked
/// against a certificate `rcgen` actually issued — the two places where the two languages could
/// disagree and produce no symptom worth reading.
///
/// M1 proved the *signing buffers* match byte-for-byte via shared fixtures. That is necessary
/// and not sufficient: it says both sides build the same bytes, not that a signature over those
/// bytes verifies. This closes that gap.
///
/// **Skipped unless pointed at a live engine.** To run it:
///
/// ```powershell
/// # terminal 1
/// cd engine ; cargo run -p muxdeckd -- --foreground --log-level info
///
/// # terminal 2
/// cd engine
/// $env:MUXDECK_LIVE_FP   = (cargo run -q -p muxdeckd -- --print-fingerprint)
/// $env:MUXDECK_LIVE_CODE = ((cargo run -q -p muxdeckd -- pair begin) `
///                            | Select-String 'Pairing code: (\d{6})').Matches.Groups[1].Value
/// $env:MUXDECK_LIVE_ADDR = '127.0.0.1:47654'
///
/// cd ..\apps\client ; fvm flutter test test/live_engine_test.dart
/// ```
///
/// Deliberately not part of CI: a GitHub runner has no engine, and standing one up there would
/// test the runner's network stack rather than the protocol.
library;

import 'dart:convert';
// Aliased: muxdeck_protocol exports its own `Platform` enum, which otherwise wins here.
import 'dart:io' as io;

import 'package:flutter_secure_storage/flutter_secure_storage.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_client/core/errors.dart';
import 'package:muxdeck_client/data/identity/device_identity.dart';
import 'package:muxdeck_client/data/transport/lan_transport.dart';
import 'package:muxdeck_client/data/transport/protocol_client.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();

  final env = io.Platform.environment;
  final addr = env['MUXDECK_LIVE_ADDR'] ?? '';
  final fingerprint = (env['MUXDECK_LIVE_FP'] ?? '').trim().toLowerCase();
  final code = (env['MUXDECK_LIVE_CODE'] ?? '').trim();

  final skipReason = addr.isEmpty || fingerprint.isEmpty
      ? 'set MUXDECK_LIVE_ADDR and MUXDECK_LIVE_FP to run against a live engine'
      : null;

  Uri uri() => Uri.parse('wss://$addr/ws');

  group('against a live engine', () {
    late DeviceIdentity identity;

    setUp(() async {
      // flutter_test installs a mock HttpClient that answers every request with
      // "Unsupported operation: Mocked response", so a real socket is impossible until it is
      // cleared. Every other test in this suite wants that mock; this file is the exception,
      // because talking to a real engine is the entire point of it.
      io.HttpOverrides.global = null;

      FlutterSecureStorage.setMockInitialValues({});
      identity = await DeviceIdentityStore().load();
    });

    test("the pin accepts the engine's real certificate", () async {
      final transport = LanTransport(uri: uri(), expectedFingerprint: fingerprint);

      await transport.connect();
      expect(
        transport.presentedFingerprint,
        fingerprint,
        reason: 'the client must hash the same leaf DER the engine reports',
      );
      await transport.close();
    }, skip: skipReason);

    test('a wrong pin is rejected against that same certificate', () async {
      // Proves the pin is load-bearing rather than incidentally passing because the connection
      // would have been accepted anyway.
      final transport = LanTransport(uri: uri(), expectedFingerprint: '0' * 64);

      await expectLater(transport.connect(), throwsA(isA<FingerprintMismatch>()));
      await transport.close();
    }, skip: skipReason);

    test('Rust verifies a signature this Dart client produced', () async {
      // The assertion this file exists for. Everything above could pass with two
      // implementations that agree with each other rather than with the specification.
      if (code.isEmpty) {
        fail('set MUXDECK_LIVE_CODE from `muxdeckd pair begin` to run this leg');
      }

      // --- pair: ed25519-dalek verifies pairProofMessage ---
      final pairing = LanTransport(uri: uri(), expectedFingerprint: fingerprint);
      await pairing.connect();
      final pairClient = ProtocolClient(pairing);

      final proof = await identity.sign(
        pairProofMessage(code: code, devicePubkey: identity.publicKey),
      );

      final paired = await pairClient.request(KnownOp.pairRequest, {
        'code': code,
        'device_pubkey': base64Encode(identity.publicKey),
        'device_name': 'live_engine_test',
        'platform': 'android',
        'proof': base64Encode(proof),
      });

      expect(
        paired['device_id'],
        identity.deviceId,
        reason: 'both languages must derive the same device id from the same public key',
      );
      final hostId = paired['host_id'] as String;

      await pairClient.close();
      await pairing.close();

      // --- authenticate: verify_strict accepts, on a fresh socket as a real deck would ---
      final session = LanTransport(uri: uri(), expectedFingerprint: fingerprint);
      await session.connect();
      final client = ProtocolClient(session);

      final hello = await client.request(KnownOp.sessionHello, {
        'device_id': identity.deviceId,
        'client_version': '0.1.0',
        'platform': 'android',
      });
      expect(hello['mode'], 'challenge');

      final signature = await identity.sign(
        sessionAuthMessage(
          nonce: base64Decode(hello['nonce'] as String),
          deviceId: identity.deviceId,
          hostId: hostId,
        ),
      );

      final ready = await client.request(KnownOp.sessionAuth, {
        'signature': base64Encode(signature),
      });

      expect(
        ready['role'],
        'deck',
        reason: 'verify_strict accepted a signature made by package:cryptography',
      );

      final pong = await client.request(KnownOp.systemPing, {
        't_client': DateTime.now().millisecondsSinceEpoch,
      });
      expect(pong['t_engine'], isA<int>());

      // F13 rather than a real shortcut: it is a key almost nothing binds, so a stray press
      // during a test run cannot close a window or overwrite a file.
      await client.request(KnownOp.inputKeyCombo, {
        'keys': ['CONTROL', 'SHIFT', 'F13'],
      });

      await client.close();
      await session.close();
    }, skip: skipReason);
  });
}
