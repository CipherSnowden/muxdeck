/// Round-trips every file in `protocol/fixtures/` and asserts semantic equality.
///
/// The mirror of `engine/crates/muxdeck-core/tests/fixtures.rs`. Both suites read the same
/// files, so a Rust/Dart disagreement about the wire shows up here rather than at runtime
/// against a real device.
///
/// The concrete payload type is chosen from `op` and `t` **only** — the variant suffix on a
/// filename plays no part in that decision (`docs/PROTOCOL.md` §8).
library;

import 'dart:convert';
import 'dart:io';
import 'dart:typed_data';

import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'package:test/test.dart';

final Directory _fixtureDir = Directory('../../protocol/fixtures');

List<File> _fixtureFiles() {
  final files =
      _fixtureDir
          .listSync()
          .whereType<File>()
          .where((f) => f.path.endsWith('.json'))
          .toList()
        ..sort((a, b) => a.path.compareTo(b.path));
  expect(files, isNotEmpty, reason: 'no fixtures in ${_fixtureDir.path}');
  return files;
}

Map<String, dynamic> _readJson(File file) =>
    jsonDecode(file.readAsStringSync()) as Map<String, dynamic>;

/// Parse into [T], serialise straight back out.
Map<String, dynamic> _roundTrip<T extends Payload>(
  Map<String, dynamic> json,
  T Function(Map<String, dynamic>) fromJson,
) {
  final envelope = Envelope.fromJson<T>(json, fromJson);
  envelope.validate();
  return envelope.toJson();
}

/// The single source of the `(op, t) -> payload type` mapping.
Map<String, dynamic> _reserialise(
  KnownOp op,
  MessageType t,
  Map<String, dynamic> json,
) {
  // An `err` carries the same payload whatever op it answers, so it is matched before the
  // op is consulted at all.
  if (t == MessageType.err) {
    return _roundTrip(json, ErrorPayload.fromJson);
  }

  final isReq = t == MessageType.req;

  return switch (op) {
    KnownOp.sessionHello =>
      isReq
          ? _roundTrip(json, HelloRequest.fromJson)
          : _roundTrip(json, HelloResponse.fromJson),
    KnownOp.sessionAuth =>
      isReq
          ? _roundTrip(json, AuthRequest.fromJson)
          : _roundTrip(json, Ready.fromJson),

    KnownOp.pairRequest =>
      isReq
          ? _roundTrip(json, PairRequest.fromJson)
          : _roundTrip(json, PairResponse.fromJson),
    KnownOp.pairBegin =>
      isReq
          ? _roundTrip(json, PairBeginRequest.fromJson)
          : _roundTrip(json, PairBeginResponse.fromJson),
    KnownOp.pairCancel => _roundTrip(json, Empty.fromJson),
    KnownOp.pairListDevices =>
      isReq
          ? _roundTrip(json, Empty.fromJson)
          : _roundTrip(json, PairListDevicesResponse.fromJson),
    KnownOp.pairRevoke =>
      isReq
          ? _roundTrip(json, PairRevokeRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),

    KnownOp.systemPing =>
      isReq
          ? _roundTrip(json, PingRequest.fromJson)
          : _roundTrip(json, PingResponse.fromJson),

    KnownOp.inputKeyCombo =>
      isReq
          ? _roundTrip(json, KeyCombo.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.inputKeySequence =>
      isReq
          ? _roundTrip(json, KeySequence.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.inputText =>
      isReq
          ? _roundTrip(json, TextRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.inputMedia =>
      isReq
          ? _roundTrip(json, MediaRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.inputMouse =>
      isReq
          ? _roundTrip(json, MouseRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),

    KnownOp.actionRun =>
      isReq
          ? _roundTrip(json, ActionRunRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.actionList =>
      isReq
          ? _roundTrip(json, Empty.fromJson)
          : _roundTrip(json, ActionListResponse.fromJson),
    KnownOp.actionSet =>
      isReq
          ? _roundTrip(json, ActionSetRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.actionDelete =>
      isReq
          ? _roundTrip(json, ActionDeleteRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),

    KnownOp.profileGet =>
      isReq
          ? _roundTrip(json, ProfileGetRequest.fromJson)
          : _roundTrip(json, ProfileWrapper.fromJson),
    KnownOp.profileList =>
      isReq
          ? _roundTrip(json, Empty.fromJson)
          : _roundTrip(json, ProfileListResponse.fromJson),
    KnownOp.profileSubscribe => _roundTrip(json, Empty.fromJson),
    KnownOp.profileActivate =>
      isReq
          ? _roundTrip(json, ProfileActivateRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.profileSet =>
      isReq
          ? _roundTrip(json, ProfileWrapper.fromJson)
          : _roundTrip(json, Empty.fromJson),
    KnownOp.profileDelete =>
      isReq
          ? _roundTrip(json, ProfileDeleteRequest.fromJson)
          : _roundTrip(json, Empty.fromJson),

    KnownOp.telemetrySubscribe => _roundTrip(json, Empty.fromJson),

    KnownOp.settingsGet =>
      isReq
          ? _roundTrip(json, Empty.fromJson)
          : _roundTrip(json, Settings.fromJson),
    KnownOp.settingsSet =>
      isReq
          ? _roundTrip(json, SettingsPatch.fromJson)
          : _roundTrip(json, SettingsSetResponse.fromJson),

    KnownOp.profileChanged => _roundTrip(json, ProfileWrapper.fromJson),
    KnownOp.telemetryUpdate => _roundTrip(json, TelemetryUpdate.fromJson),
    KnownOp.deviceChanged => _roundTrip(json, DeviceChangedEvent.fromJson),
    KnownOp.pairingState => _roundTrip(json, PairingState.fromJson),
    KnownOp.engineShutdown => _roundTrip(json, EngineShutdownEvent.fromJson),
  };
}

void main() {
  group('fixtures', () {
    test('every fixture round-trips', () {
      for (final file in _fixtureFiles()) {
        final what = file.uri.pathSegments.last;
        final original = _readJson(file);
        final op = Op.parse(original['op'] as String).known;
        expect(
          op,
          isNotNull,
          reason: '$what: fixture uses an op this build does not know',
        );
        final t = MessageType.fromWire(original['t'] as String);

        expect(
          _reserialise(op!, t, original),
          equals(original),
          reason: '$what: re-serialised form differs from the fixture',
        );
      }
    });

    test('every known op has at least one fixture', () {
      final seen = <KnownOp>{};
      for (final file in _fixtureFiles()) {
        final op = Op.parse(_readJson(file)['op'] as String).known;
        if (op != null) seen.add(op);
      }
      final missing = KnownOp.values.where((op) => !seen.contains(op)).toList();
      expect(
        missing,
        isEmpty,
        reason:
            'docs/PROTOCOL.md §8 requires one file per message shape; missing: $missing',
      );
    });

    test('events carry no correlation id and everything else does', () {
      for (final file in _fixtureFiles()) {
        final what = file.uri.pathSegments.last;
        final json = _readJson(file);
        final t = MessageType.fromWire(json['t'] as String);
        final op = Op.parse(json['op'] as String).known;

        if (t == MessageType.evt) {
          expect(json['id'], isNull, reason: '$what: an evt must have no id');
        } else {
          expect(
            json['id'],
            isNotNull,
            reason: '$what: a req/res/err must have an id',
          );
        }

        if (op != null) {
          expect(
            op.isEvent,
            equals(t == MessageType.evt),
            reason: '$what: op and t disagree about whether this is an event',
          );
        }
      }
    });
  });

  // -------------------------------------------------------------------------
  // Negative tests. A fixture suite that cannot fail is not testing anything.
  // -------------------------------------------------------------------------

  group('rejections', () {
    test('an unsupported version is rejected', () {
      final json =
          jsonDecode(
                '{"v":2,"t":"req","id":"x","op":"system.ping","d":{"t_client":1}}',
              )
              as Map<String, dynamic>;
      final envelope = Envelope.fromJson(json, PingRequest.fromJson);
      expect(
        () => envelope.validate(),
        throwsA(
          isA<ProtocolException>().having(
            (e) => e.code,
            'code',
            ErrorCode.unsupportedVersion,
          ),
        ),
      );
    });

    test('an unknown op parses into a rejectable value rather than throwing', () {
      // Parsing must survive so the engine can answer UNKNOWN_OP while still echoing the
      // correlation ID. A hard failure here would leave the client waiting forever.
      final op = Op.parse('input.telepathy');
      expect(op.known, isNull);
      expect(op.wire, equals('input.telepathy'));
    });

    test('a missing required field fails the parse', () {
      final json =
          jsonDecode(
                '{"v":1,"t":"req","id":"x","op":"input.key_combo","d":{"hold_ms":0}}',
              )
              as Map<String, dynamic>;
      expect(
        () => Envelope.fromJson(json, KeyCombo.fromJson),
        throwsA(isA<TypeError>()),
      );
    });

    test('an event carrying a correlation id is rejected', () {
      final json =
          jsonDecode(
                '{"v":1,"t":"evt","id":"x","op":"engine.shutdown","d":{"reason":"fatal_error"}}',
              )
              as Map<String, dynamic>;
      final envelope = Envelope.fromJson(json, EngineShutdownEvent.fromJson);
      expect(
        () => envelope.validate(),
        throwsA(
          isA<ProtocolException>().having(
            (e) => e.code,
            'code',
            ErrorCode.badRequest,
          ),
        ),
      );
    });

    test('a hello response with an unrecognised mode is rejected', () {
      // The tag is the only thing that picks a branch. An unrecognised value is a hard
      // failure, not a field to skip past.
      expect(
        () => HelloResponse.fromJson(<String, dynamic>{
          'mode': 'maybe',
          'role': 'deck',
        }),
        throwsA(isA<ProtocolException>()),
      );
    });

    test('hello requires exactly one of device_id and admin_token', () {
      const both = HelloRequest(
        deviceId: 'd_1',
        adminToken: 't',
        clientVersion: '0.1.0',
        platform: Platform.windows,
      );
      expect(both.validate, throwsA(isA<ProtocolException>()));

      const neither = HelloRequest(
        clientVersion: '0.1.0',
        platform: Platform.windows,
      );
      expect(neither.validate, throwsA(isA<ProtocolException>()));
    });

    test('Ready has no mode of its own but gains one through the union', () {
      const ready = Ready(
        role: Role.deck,
        protocol: 1,
        engineVersion: '0.1.0',
        hostPlatform: HostPlatform.linux,
        activeProfileId: 'p_default',
        capabilities: Capabilities(
          textUnicode: false,
          mediaKeys: true,
          mouse: true,
          shellActions: false,
        ),
      );

      expect(
        ready.toJson().containsKey('mode'),
        isFalse,
        reason: 'session.auth returns Ready untagged',
      );
      expect(const ReadyResponse(ready).toJson()['mode'], equals('ready'));
    });

    test('a key combo with two non-modifiers is rejected', () {
      expect(
        const KeyCombo([Key.a, Key.b]).validate,
        throwsA(isA<ProtocolException>()),
      );
      // Zero non-modifiers is valid: META alone is a real macro.
      expect(const KeyCombo([Key.meta]).validate, returnsNormally);
    });

    test('ttl_seconds outside 30..=300 is rejected', () {
      expect(
        const PairBeginRequest(ttlSeconds: 29).validate,
        throwsA(isA<ProtocolException>()),
      );
      expect(
        const PairBeginRequest(ttlSeconds: 301).validate,
        throwsA(isA<ProtocolException>()),
      );
      expect(const PairBeginRequest().ttlOrDefault, equals(120));
    });
  });

  // -------------------------------------------------------------------------
  // Signing layouts, checked as raw bytes rather than as JSON.
  // -------------------------------------------------------------------------

  group('signing', () {
    Uint8List fromHex(String s) {
      expect(s.length.isEven, isTrue, reason: 'hex string has an odd length');
      return Uint8List.fromList([
        for (var i = 0; i < s.length; i += 2)
          int.parse(s.substring(i, i + 2), radix: 16),
      ]);
    }

    Map<String, dynamic> load(String name) =>
        _readJson(File('${_fixtureDir.path}/signing/$name'));

    test('session.auth signing layout matches the fixture', () {
      final f = load('session_auth.json');
      final actual = sessionAuthMessage(
        nonce: fromHex(f['nonce_hex'] as String),
        deviceId: f['device_id'] as String,
        hostId: f['host_id'] as String,
      );

      expect(actual.length, equals(f['message_len']));
      expect(actual, equals(fromHex(f['message_hex'] as String)));
    });

    test('pair.request proof signing layout matches the fixture', () {
      final f = load('pair_proof.json');
      final actual = pairProofMessage(
        code: f['code'] as String,
        devicePubkey: fromHex(f['device_pubkey_hex'] as String),
      );

      expect(actual.length, equals(f['message_len']));
      expect(actual, equals(fromHex(f['message_hex'] as String)));
    });
  });
}
