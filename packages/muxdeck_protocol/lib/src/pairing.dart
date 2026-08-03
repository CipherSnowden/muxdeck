/// Pairing payloads. `docs/PROTOCOL.md` §4.2.
library;

import 'envelope.dart';
import 'session.dart';

/// The inclusive bounds `ttl_seconds` must fall within. Outside them is `BAD_REQUEST`, not
/// silent coercion.
const int minTtlSeconds = 30;
const int maxTtlSeconds = 300;

/// The pairing window length used when `ttl_seconds` is omitted.
const int defaultTtlSeconds = 120;

/// `pair.request` — callable only during a pairing window, by an unauthenticated socket.
class PairRequest implements Payload {
  const PairRequest({
    required this.code,
    required this.devicePubkey,
    required this.deviceName,
    required this.platform,
    required this.proof,
  });

  factory PairRequest.fromJson(Map<String, dynamic> json) => PairRequest(
    code: json['code'] as String,
    devicePubkey: json['device_pubkey'] as String,
    deviceName: json['device_name'] as String,
    platform: Platform.fromWire(json['platform'] as String),
    proof: json['proof'] as String,
  );

  /// The six-digit one-time code.
  final String code;

  /// Ed25519 public key, 32 bytes, base64.
  final String devicePubkey;
  final String deviceName;
  final Platform platform;

  /// Ed25519 signature, 64 bytes, base64. Proves the device holds the private half of the
  /// key it is registering — without it, anyone who read the QR could register a public
  /// key they do not control.
  final String proof;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'code': code,
    'device_pubkey': devicePubkey,
    'device_name': deviceName,
    'platform': platform.wire,
    'proof': proof,
  };
}

class PairResponse implements Payload {
  const PairResponse({
    required this.deviceId,
    required this.hostId,
    required this.hostName,
  });

  factory PairResponse.fromJson(Map<String, dynamic> json) => PairResponse(
    deviceId: json['device_id'] as String,
    hostId: json['host_id'] as String,
    hostName: json['host_name'] as String,
  );

  final String deviceId;
  final String hostId;
  final String hostName;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'device_id': deviceId,
    'host_id': hostId,
    'host_name': hostName,
  };
}

/// `pair.begin` — admin only. Opens a pairing window.
class PairBeginRequest implements Payload {
  const PairBeginRequest({this.ttlSeconds});

  factory PairBeginRequest.fromJson(Map<String, dynamic> json) =>
      PairBeginRequest(ttlSeconds: json['ttl_seconds'] as int?);

  final int? ttlSeconds;

  /// The window length this request asks for, applying the default when omitted.
  ///
  /// Call [validate] first — this does not clamp, because silently clamping an
  /// out-of-range value would hide a client bug.
  int get ttlOrDefault => ttlSeconds ?? defaultTtlSeconds;

  void validate() {
    final ttl = ttlSeconds;
    if (ttl != null && (ttl < minTtlSeconds || ttl > maxTtlSeconds)) {
      throw ProtocolException(
        ErrorCode.badRequest,
        'ttl_seconds $ttl is outside $minTtlSeconds..=$maxTtlSeconds',
      );
    }
  }

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    if (ttlSeconds != null) 'ttl_seconds': ttlSeconds,
  };
}

class PairBeginResponse implements Payload {
  const PairBeginResponse({
    required this.code,
    required this.expiresAt,
    required this.qrPayload,
  });

  factory PairBeginResponse.fromJson(Map<String, dynamic> json) =>
      PairBeginResponse(
        code: json['code'] as String,
        expiresAt: json['expires_at'] as int,
        qrPayload: json['qr_payload'] as String,
      );

  final String code;

  /// Unix timestamp, seconds.
  final int expiresAt;

  /// `muxdeck://pair?addr=&host=&fp=&code=`, parameters in exactly that order.
  final String qrPayload;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'code': code,
    'expires_at': expiresAt,
    'qr_payload': qrPayload,
  };
}

class PairListDevicesResponse implements Payload {
  const PairListDevicesResponse(this.devices);

  factory PairListDevicesResponse.fromJson(Map<String, dynamic> json) =>
      PairListDevicesResponse(
        (json['devices'] as List<dynamic>)
            .map((e) => DeviceInfo.fromJson(e as Map<String, dynamic>))
            .toList(),
      );

  final List<DeviceInfo> devices;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'devices': devices.map((d) => d.toJson()).toList(),
  };
}

class DeviceInfo {
  const DeviceInfo({
    required this.deviceId,
    required this.name,
    required this.platform,
    required this.pairedAt,
    required this.lastSeen,
    required this.connected,
  });

  factory DeviceInfo.fromJson(Map<String, dynamic> json) => DeviceInfo(
    deviceId: json['device_id'] as String,
    name: json['name'] as String,
    platform: Platform.fromWire(json['platform'] as String),
    pairedAt: json['paired_at'] as int,
    lastSeen: json['last_seen'] as int,
    connected: json['connected'] as bool,
  );

  final String deviceId;
  final String name;
  final Platform platform;

  /// Unix timestamp, seconds.
  final int pairedAt;

  /// Unix timestamp, seconds.
  final int lastSeen;
  final bool connected;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'device_id': deviceId,
    'name': name,
    'platform': platform.wire,
    'paired_at': pairedAt,
    'last_seen': lastSeen,
    'connected': connected,
  };
}

class PairRevokeRequest implements Payload {
  const PairRevokeRequest(this.deviceId);

  factory PairRevokeRequest.fromJson(Map<String, dynamic> json) =>
      PairRevokeRequest(json['device_id'] as String);

  final String deviceId;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'device_id': deviceId};
}

/// `evt pairing.state`, delivered to `admin` sockets only.
class PairingState implements Payload {
  const PairingState({required this.active, required this.expiresAt});

  factory PairingState.fromJson(Map<String, dynamic> json) => PairingState(
    active: json['active'] as bool,
    expiresAt: json['expires_at'] as int,
  );

  final bool active;

  /// Unix timestamp, seconds.
  final int expiresAt;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'active': active,
    'expires_at': expiresAt,
  };
}
