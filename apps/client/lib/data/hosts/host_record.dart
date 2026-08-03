/// A host this device has paired with.
library;

import 'dart:convert';

/// Everything needed to reconnect to a host without pairing again.
///
/// [fingerprint] is the load-bearing field: it is the only thing that authenticates the host,
/// and it is captured out of band from the pairing QR rather than learned from the connection
/// itself. See `docs/CLIENT.md` §3.
class HostRecord {
  const HostRecord({
    required this.hostId,
    required this.hostName,
    required this.address,
    required this.fingerprint,
    required this.deviceId,
  });

  factory HostRecord.fromJson(Map<String, dynamic> json) => HostRecord(
    hostId: json['host_id'] as String,
    hostName: json['host_name'] as String,
    address: json['address'] as String,
    fingerprint: json['fingerprint'] as String,
    deviceId: json['device_id'] as String,
  );

  /// `h_` followed by 16 hex characters. Matches the mDNS TXT `id` record exactly, so a stored
  /// host is matched to a discovery result by plain string equality.
  final String hostId;

  /// Friendly name, for the UI. May go stale if the user renames the host; the ID is what
  /// identifies it.
  final String hostName;

  /// `<ip>:<port>` as last known. Re-learned from mDNS when the host moves, which is why an IP
  /// change does not require re-pairing.
  final String address;

  /// Lowercase hex, 64 characters, SHA-256 over the leaf certificate DER.
  final String fingerprint;

  /// This device's ID as the host knows it.
  final String deviceId;

  HostRecord copyWith({String? hostName, String? address}) => HostRecord(
    hostId: hostId,
    hostName: hostName ?? this.hostName,
    address: address ?? this.address,
    fingerprint: fingerprint,
    deviceId: deviceId,
  );

  Map<String, dynamic> toJson() => <String, dynamic>{
    'host_id': hostId,
    'host_name': hostName,
    'address': address,
    'fingerprint': fingerprint,
    'device_id': deviceId,
  };

  /// `wss://<address>/ws`. `docs/PROTOCOL.md` §1.
  Uri get websocketUri => Uri.parse('wss://$address/ws');

  @override
  bool operator ==(Object other) =>
      other is HostRecord && other.hostId == hostId;

  @override
  int get hashCode => hostId.hashCode;

  @override
  String toString() => 'HostRecord($hostId, $hostName, $address)';
}

/// Parses a pairing QR payload.
///
/// The payload is `muxdeck://pair?addr=&host=&fp=&code=` with parameters in that order
/// (`docs/PROTOCOL.md` §4.2). Order is not enforced here — `Uri` gives a map, and a stricter
/// parser would reject payloads that are perfectly well-formed for no benefit.
///
/// Returns null for anything malformed, so the scanner can keep scanning rather than throwing on
/// every unrelated QR code the camera happens to see.
class PairingPayload {
  const PairingPayload({
    required this.address,
    required this.hostId,
    required this.fingerprint,
    required this.code,
  });

  static PairingPayload? tryParse(String raw) {
    final uri = Uri.tryParse(raw.trim());
    if (uri == null || uri.scheme != 'muxdeck' || uri.host != 'pair') {
      return null;
    }

    final address = uri.queryParameters['addr'];
    final hostId = uri.queryParameters['host'];
    final fingerprint = uri.queryParameters['fp'];
    final code = uri.queryParameters['code'];

    if (address == null ||
        hostId == null ||
        fingerprint == null ||
        code == null) {
      return null;
    }

    // Shape checks, not just presence. A truncated fingerprint would otherwise be stored and
    // then fail every future connection with a mismatch the user cannot explain.
    if (!_isHostId(hostId)) return null;
    if (!_isFingerprint(fingerprint)) return null;
    if (!_isPairingCode(code)) return null;
    if (!address.contains(':')) return null;

    return PairingPayload(
      address: address,
      hostId: hostId,
      fingerprint: fingerprint.toLowerCase(),
      code: code,
    );
  }

  final String address;
  final String hostId;
  final String fingerprint;
  final String code;

  static bool _isHostId(String value) =>
      value.length == 18 &&
      value.startsWith('h_') &&
      _isHex(value.substring(2));

  static bool _isFingerprint(String value) =>
      value.length == 64 && _isHex(value);

  static bool _isPairingCode(String value) =>
      value.length == 6 && value.codeUnits.every((c) => c >= 0x30 && c <= 0x39);

  static bool _isHex(String value) => RegExp(r'^[0-9a-fA-F]+$').hasMatch(value);
}

/// Encodes a list of records for `shared_preferences`.
String encodeHostRecords(List<HostRecord> records) =>
    jsonEncode(records.map((r) => r.toJson()).toList());

/// Decodes what [encodeHostRecords] wrote, tolerating corruption by returning what survives.
///
/// A single unreadable entry loses one host, not every host — the alternative is a user who
/// cannot reach any of their machines because one record went bad.
///
/// **Always returns a growable list**, including on the empty and corrupt paths. Returning
/// `const []` there would save an allocation and hand callers a list that throws on the first
/// `add` or `removeWhere` — which is exactly what [HostStore.save] does, so the very first
/// pairing on a fresh install would fail with "Cannot remove from an unmodifiable list".
List<HostRecord> decodeHostRecords(String? encoded) {
  if (encoded == null || encoded.isEmpty) return <HostRecord>[];

  final List<dynamic> raw;
  try {
    raw = jsonDecode(encoded) as List<dynamic>;
  } catch (_) {
    return <HostRecord>[];
  }

  final records = <HostRecord>[];
  for (final entry in raw) {
    try {
      records.add(HostRecord.fromJson(entry as Map<String, dynamic>));
    } catch (_) {
      continue;
    }
  }
  return records;
}
