/// Session handshake payloads. `docs/PROTOCOL.md` §4.1.
library;

import 'envelope.dart';

/// `session.hello` request. Two mutually exclusive forms.
///
/// Exactly one of [deviceId] (a paired deck) and [adminToken] (the local control panel)
/// must be present. Both, or neither, is `BAD_REQUEST` — see [validate]. The absent one is
/// omitted from the wire rather than sent as `null`.
class HelloRequest implements Payload {
  const HelloRequest({
    required this.clientVersion,
    required this.platform,
    this.deviceId,
    this.adminToken,
  });

  factory HelloRequest.fromJson(Map<String, dynamic> json) => HelloRequest(
    deviceId: json['device_id'] as String?,
    adminToken: json['admin_token'] as String?,
    clientVersion: json['client_version'] as String,
    platform: Platform.fromWire(json['platform'] as String),
  );

  final String? deviceId;
  final String? adminToken;
  final String clientVersion;
  final Platform platform;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    if (deviceId != null) 'device_id': deviceId,
    if (adminToken != null) 'admin_token': adminToken,
    'client_version': clientVersion,
    'platform': platform.wire,
  };

  void validate() {
    final hasDevice = deviceId != null;
    final hasToken = adminToken != null;
    if (hasDevice && hasToken) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'session.hello carries both device_id and admin_token; exactly one is required',
      );
    }
    if (!hasDevice && !hasToken) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'session.hello carries neither device_id nor admin_token; exactly one is required',
      );
    }
  }
}

/// `session.hello` response: an internally tagged union on `mode`.
///
/// The tag is always present and is the only thing that picks a branch — never infer the
/// shape from which optional fields happen to be set. An unrecognised `mode` is a hard
/// failure, not a field to skip past.
sealed class HelloResponse implements Payload {
  const HelloResponse();

  factory HelloResponse.fromJson(Map<String, dynamic> json) {
    final mode = json['mode'];
    return switch (mode) {
      'challenge' => Challenge.fromJson(json),
      'ready' => ReadyResponse(Ready.fromJson(json)),
      _ => throw ProtocolException(
        ErrorCode.badRequest,
        'unrecognised session.hello mode "$mode"',
      ),
    };
  }
}

/// The `mode: "challenge"` branch — answer to the deck form.
class Challenge extends HelloResponse {
  const Challenge({
    required this.nonce,
    required this.hostId,
    required this.hostName,
  });

  factory Challenge.fromJson(Map<String, dynamic> json) => Challenge(
    nonce: json['nonce'] as String,
    hostId: json['host_id'] as String,
    hostName: json['host_name'] as String,
  );

  /// 32 random bytes, base64.
  final String nonce;
  final String hostId;
  final String hostName;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'mode': 'challenge',
    'nonce': nonce,
    'host_id': hostId,
    'host_name': hostName,
  };
}

/// The `mode: "ready"` branch, wrapping the shared [Ready] payload.
///
/// The wrapper exists because the tag belongs to the union, not to [Ready] — which is
/// returned untagged as the `session.auth` response and has no `mode` field of its own.
class ReadyResponse extends HelloResponse {
  const ReadyResponse(this.ready);

  final Ready ready;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'mode': 'ready',
    ...ready.toJson(),
  };
}

/// The `session.auth` response, and the body of the `mode: "ready"` branch.
///
/// **One type, two places.** [fromJson] tolerates a `mode` key so it parses correctly
/// whether it arrived inside the union or on its own, and [toJson] never emits one.
class Ready implements Payload {
  const Ready({
    required this.role,
    required this.protocol,
    required this.engineVersion,
    required this.hostPlatform,
    required this.activeProfileId,
    required this.capabilities,
  });

  factory Ready.fromJson(Map<String, dynamic> json) => Ready(
    role: Role.fromWire(json['role'] as String),
    protocol: json['protocol'] as int,
    engineVersion: json['engine_version'] as String,
    hostPlatform: HostPlatform.fromWire(json['host_platform'] as String),
    activeProfileId: json['active_profile_id'] as String,
    capabilities: Capabilities.fromJson(
      json['capabilities'] as Map<String, dynamic>,
    ),
  );

  final Role role;
  final int protocol;
  final String engineVersion;
  final HostPlatform hostPlatform;
  final String activeProfileId;
  final Capabilities capabilities;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'role': role.wire,
    'protocol': protocol,
    'engine_version': engineVersion,
    'host_platform': hostPlatform.wire,
    'active_profile_id': activeProfileId,
    'capabilities': capabilities.toJson(),
  };
}

/// What this host can actually do right now, so the deck can grey out buttons whose action
/// is unavailable instead of letting them fail at press time.
class Capabilities {
  const Capabilities({
    required this.textUnicode,
    required this.mediaKeys,
    required this.mouse,
    required this.shellActions,
  });

  factory Capabilities.fromJson(Map<String, dynamic> json) => Capabilities(
    textUnicode: json['text_unicode'] as bool,
    mediaKeys: json['media_keys'] as bool,
    mouse: json['mouse'] as bool,
    shellActions: json['shell_actions'] as bool,
  );

  /// False when `input.text` cannot inject arbitrary Unicode — notably Linux/uinput.
  final bool textUnicode;
  final bool mediaKeys;
  final bool mouse;

  /// False when shell execution is disabled.
  final bool shellActions;

  Map<String, dynamic> toJson() => <String, dynamic>{
    'text_unicode': textUnicode,
    'media_keys': mediaKeys,
    'mouse': mouse,
    'shell_actions': shellActions,
  };
}

/// `session.auth` request. Valid only after a [Challenge].
class AuthRequest implements Payload {
  const AuthRequest(this.signature);

  factory AuthRequest.fromJson(Map<String, dynamic> json) =>
      AuthRequest(json['signature'] as String);

  /// Ed25519 signature, 64 bytes, base64.
  final String signature;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{'signature': signature};
}

/// The platform a *client* runs on.
enum Platform {
  ios('ios'),
  android('android'),
  windows('windows'),
  macos('macos'),
  linux('linux');

  const Platform(this.wire);

  final String wire;

  static Platform fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown platform "$wire"',
    ),
  );
}

/// The platform the *engine* runs on. Narrower than [Platform]: there is no mobile host.
enum HostPlatform {
  windows('windows'),
  macos('macos'),
  linux('linux');

  const HostPlatform(this.wire);

  final String wire;

  static HostPlatform fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown host platform "$wire"',
    ),
  );
}

/// `deck` for paired devices, `admin` for the local control panel.
///
/// `admin` cannot be requested — the engine grants it only for a loopback peer presenting
/// the local admin token. See `docs/ARCHITECTURE.md` §5.4.
enum Role {
  deck('deck'),
  admin('admin');

  const Role(this.wire);

  final String wire;

  static Role fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () =>
        throw ProtocolException(ErrorCode.badRequest, 'unknown role "$wire"'),
  );
}
