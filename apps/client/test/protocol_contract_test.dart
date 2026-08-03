/// The parts of the wire contract the client can get wrong on its own.
///
/// Everything here is pure: identifier derivation, QR payload parsing, host-record persistence.
/// No widgets, no sockets, no engine — those flows go through `test/support/fake_engine.dart`,
/// and mixing them in here would mean a parser bug and a handshake bug failing the same test.
///
/// The bias throughout is towards malformed input. A QR scanner sees every code the camera
/// passes over, and a stored record survives an app upgrade that changed its shape; the happy
/// paths are one line each because they are not where this code fails.
library;

import 'dart:convert';

import 'package:flutter_test/flutter_test.dart';
import 'package:muxdeck_client/data/hosts/host_record.dart';
import 'package:muxdeck_client/data/identity/device_identity.dart';

/// A syntactically valid pairing payload, matching `docs/PROTOCOL.md` §4.2's example.
const _validPayload =
    'muxdeck://pair'
    '?addr=192.168.1.42:47654'
    '&host=h_a91c4d2e8f019b37'
    '&fp=3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937'
    '&code=402913';

void main() {
  group('PairingPayload.tryParse', () {
    test('reads all four parameters from a well-formed payload', () {
      final payload = PairingPayload.tryParse(_validPayload);

      expect(payload, isNotNull);
      expect(payload!.address, '192.168.1.42:47654');
      expect(payload.hostId, 'h_a91c4d2e8f019b37');
      expect(
        payload.fingerprint,
        '3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937',
      );
      expect(payload.code, '402913');
    });

    test('lowercases the fingerprint', () {
      // Stored as-is it would fail every future comparison against a `Digest.toString()`, which
      // is always lowercase — a mismatch the user would read as "this host's identity changed".
      final payload = PairingPayload.tryParse(
        _validPayload.replaceFirst('fp=3b1f8c07', 'fp=3B1F8C07'),
      );

      expect(payload!.fingerprint, startsWith('3b1f8c07'));
    });

    test('rejects a payload that is not muxdeck://pair', () {
      // The scanner sees every code in front of the camera, so anything unrecognised has to be
      // ignorable rather than fatal.
      expect(
        PairingPayload.tryParse(_validPayload.replaceFirst('muxdeck://', 'https://')),
        isNull,
      );
    });

    test('rejects a payload missing any one of the four parameters', () {
      const parameters = {
        'addr': '&addr=192.168.1.42:47654',
        'host': '&host=h_a91c4d2e8f019b37',
        'fp': '&fp=3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937',
        'code': '&code=402913',
      };

      // Rebuilt from the parts rather than string-surgeried out of the valid payload, so that
      // dropping the first parameter leaves a payload that is otherwise still well-formed.
      for (final omitted in parameters.keys) {
        final raw = StringBuffer('muxdeck://pair?');
        for (final entry in parameters.entries) {
          if (entry.key != omitted) raw.write(entry.value);
        }

        expect(
          PairingPayload.tryParse(raw.toString()),
          isNull,
          reason: 'a payload without $omitted must not parse',
        );
      }
    });

    test('rejects a fingerprint that is one character short', () {
      // The case the length check exists for: a truncated fingerprint is storable and looks
      // right, and then fails every connection with a mismatch the user cannot explain.
      expect(
        PairingPayload.tryParse(
          _validPayload.replaceFirst(
            'fp=3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937',
            'fp=3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a93',
          ),
        ),
        isNull,
      );
    });

    test('rejects a fingerprint that is not hex', () {
      expect(
        PairingPayload.tryParse(
          _validPayload.replaceFirst(
            'fp=3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937',
            'fp=zzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzzz',
          ),
        ),
        isNull,
      );
    });

    test('rejects a five-digit code', () {
      expect(
        PairingPayload.tryParse(
          _validPayload.replaceFirst('code=402913', 'code=40291'),
        ),
        isNull,
      );
    });

    test('rejects a host ID of the wrong length', () {
      expect(
        PairingPayload.tryParse(
          _validPayload.replaceFirst('host=h_a91c4d2e8f019b37', 'host=h_a91c4d2e'),
        ),
        isNull,
      );
    });

    test('rejects an address with no port', () {
      expect(
        PairingPayload.tryParse(
          _validPayload.replaceFirst('addr=192.168.1.42:47654', 'addr=192.168.1.42'),
        ),
        isNull,
      );
    });

    test('rejects arbitrary text', () {
      // A URL parser accepts a bare word as a relative reference, so this reaches the scheme
      // check rather than throwing — which is exactly the path a random QR code takes.
      expect(PairingPayload.tryParse('WIFI:S=CoffeeShop;T=WPA;P=hunter2;;'), isNull);
      expect(PairingPayload.tryParse('not a uri at all'), isNull);
      expect(PairingPayload.tryParse(''), isNull);
    });
  });

  group('deviceIdFromPublicKey', () {
    // A public key's bytes are all this derivation sees, so a fixed array is as good as a real
    // key and keeps the expectation reproducible.
    final publicKey = List<int>.generate(32, (index) => index);

    test('produces d_ followed by 16 lowercase hex characters', () {
      final deviceId = deviceIdFromPublicKey(publicKey);

      expect(deviceId, hasLength(18));
      expect(deviceId, matches(RegExp(r'^d_[0-9a-f]{16}$')));
    });

    test('is deterministic for the same key', () {
      // Load-bearing: the engine derives the same ID from the same bytes rather than
      // transmitting it (`docs/PROTOCOL.md` §2.2), and pairing fails loudly if the two disagree.
      expect(
        deviceIdFromPublicKey(publicKey),
        deviceIdFromPublicKey(List<int>.from(publicKey)),
      );
    });

    test('differs for a different key', () {
      expect(
        deviceIdFromPublicKey(publicKey),
        isNot(deviceIdFromPublicKey(List<int>.generate(32, (index) => index + 1))),
      );
    });
  });

  group('HostRecord', () {
    const record = HostRecord(
      hostId: 'h_a91c4d2e8f019b37',
      hostName: 'ENIGMA-ENTROPY',
      address: '192.168.1.42:47654',
      fingerprint: '3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937',
      deviceId: 'd_7f3a91c2b4e05d18',
    );

    test('survives a JSON round trip with every field intact', () {
      // Equality is by host ID alone, so it cannot stand in for this — a round trip that dropped
      // the fingerprint would still compare equal and then fail to connect.
      final restored = HostRecord.fromJson(
        jsonDecode(jsonEncode(record.toJson())) as Map<String, dynamic>,
      );

      expect(restored.hostId, record.hostId);
      expect(restored.hostName, record.hostName);
      expect(restored.address, record.address);
      expect(restored.fingerprint, record.fingerprint);
      expect(restored.deviceId, record.deviceId);
    });

    test('builds a wss:// URI ending in /ws', () {
      expect(record.websocketUri.toString(), 'wss://192.168.1.42:47654/ws');
    });
  });

  group('decodeHostRecords', () {
    test('returns nothing for absent or empty storage', () {
      expect(decodeHostRecords(null), isEmpty);
      expect(decodeHostRecords(''), isEmpty);
    });

    test('returns nothing rather than throwing on unparseable JSON', () {
      expect(decodeHostRecords('{not json'), isEmpty);
      // Valid JSON, wrong shape: an object where a list was written.
      expect(decodeHostRecords('{"host_id":"h_a91c4d2e8f019b37"}'), isEmpty);
    });

    test('keeps the readable entries when one is corrupt', () {
      // The whole reason the decoder swallows errors per entry: one bad record must cost the
      // user one host, not every machine they own.
      final encoded = encodeHostRecords(const [
        HostRecord(
          hostId: 'h_a91c4d2e8f019b37',
          hostName: 'ENIGMA-ENTROPY',
          address: '192.168.1.42:47654',
          fingerprint: '3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937',
          deviceId: 'd_7f3a91c2b4e05d18',
        ),
        HostRecord(
          hostId: 'h_0123456789abcdef',
          hostName: 'WORKSTATION',
          address: '192.168.1.43:47654',
          fingerprint: 'ac2f9b1d80e347c5a6b1029e4f7d3c58091b6a2e7d4f8c130b5e9a627c4d8f01',
          deviceId: 'd_7f3a91c2b4e05d18',
        ),
      ]);

      final withCorruption = jsonDecode(encoded) as List<dynamic>;
      (withCorruption.first as Map<String, dynamic>).remove('fingerprint');

      final survivors = decodeHostRecords(jsonEncode(withCorruption));

      expect(survivors, hasLength(1));
      expect(survivors.single.hostId, 'h_0123456789abcdef');
    });
  });
}
