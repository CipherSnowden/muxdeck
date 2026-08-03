/// The message envelope, operation names and error payloads. `docs/PROTOCOL.md` §2.
library;

/// The only protocol major version this build speaks.
const int protocolVersion = 1;

/// Anything that can be the `d` of an [Envelope].
abstract interface class Payload {
  Map<String, dynamic> toJson();
}

/// Thrown when a message cannot be understood. Carries the [ErrorCode] the engine would
/// answer with, so the same failure reads the same way on both sides of the wire.
class ProtocolException implements Exception {
  const ProtocolException(this.code, this.message);

  final ErrorCode code;
  final String message;

  @override
  String toString() => 'ProtocolException(${code.wire}): $message';
}

/// Every message is this shape.
///
/// Generic over its payload because `d`'s type is a function of `op` and `t` — and of
/// nothing else. A reader picks the concrete payload from those two fields; the variant
/// suffix on a fixture filename is not an input to that decision (`docs/PROTOCOL.md` §8).
class Envelope<T extends Payload> {
  const Envelope({
    required this.v,
    required this.t,
    required this.op,
    required this.d,
    this.id,
  });

  /// Protocol major version. Reject anything but [protocolVersion].
  final int v;

  /// `req`, `res`, `err` or `evt`.
  final MessageType t;

  /// Correlation ID. Present on `req`, `res` and `err`; absent on `evt`.
  final String? id;

  /// Operation name. A `res` or `err` echoes the op of the `req` it answers.
  final Op op;

  /// Payload. `{}` when empty, never `null`.
  final T d;

  /// Reads an envelope, delegating the payload to [payloadFromJson].
  ///
  /// The caller supplies that function having already decided the payload type from `op`
  /// and `t`; this constructor deliberately does not make that choice itself, so the
  /// mapping lives in exactly one place per consumer.
  static Envelope<T> fromJson<T extends Payload>(
    Map<String, dynamic> json,
    T Function(Map<String, dynamic>) payloadFromJson,
  ) {
    final d = json['d'];
    if (d is! Map<String, dynamic>) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'envelope `d` must be an object, never null',
      );
    }
    return Envelope<T>(
      v: json['v'] as int,
      t: MessageType.fromWire(json['t'] as String),
      id: json['id'] as String?,
      op: Op.parse(json['op'] as String),
      d: payloadFromJson(d),
    );
  }

  Map<String, dynamic> toJson() => <String, dynamic>{
    'v': v,
    't': t.wire,
    if (id != null) 'id': id,
    'op': op.wire,
    'd': d.toJson(),
  };

  bool get isSupportedVersion => v == protocolVersion;

  /// Checks the invariants that hold regardless of which op this is. Payload-level
  /// validation belongs with the payload.
  ///
  /// An `evt` carries its own op name because there is no request to echo, so it has no
  /// correlation ID; every other kind must have one.
  void validate() {
    if (!isSupportedVersion) {
      throw ProtocolException(
        ErrorCode.unsupportedVersion,
        'protocol version $v is not supported',
      );
    }
    if (t == MessageType.evt && id != null) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'an evt must not carry a correlation id',
      );
    }
    if (t != MessageType.evt && id == null) {
      throw const ProtocolException(
        ErrorCode.badRequest,
        'a req, res or err must carry a correlation id',
      );
    }
  }
}

/// The `t` field.
enum MessageType {
  req('req'),
  res('res'),
  err('err'),
  evt('evt');

  const MessageType(this.wire);

  final String wire;

  static MessageType fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown message type "$wire"',
    ),
  );
}

/// An operation name, known or not.
///
/// An unrecognised op parses into an [Op] with a null [known] rather than throwing, so the
/// engine can answer a well-formed `UNKNOWN_OP` that still echoes the correlation ID. A
/// hard failure here would leave the client waiting.
class Op {
  const Op._(this.wire, this.known);

  factory Op.parse(String wire) => Op._(wire, KnownOp.tryFromWire(wire));

  factory Op.of(KnownOp op) => Op._(op.wire, op);

  /// The wire string, whether or not this op is one we know.
  final String wire;

  /// Null when this build does not recognise the op.
  final KnownOp? known;

  @override
  bool operator ==(Object other) => other is Op && other.wire == wire;

  @override
  int get hashCode => wire.hashCode;

  @override
  String toString() => wire;
}

/// Every op defined by the protocol.
///
/// Exhaustive against the capability matrix in `docs/ARCHITECTURE.md` §5.4 — that table
/// and this enum are checked against each other, so an op added to one without the other
/// is a bug.
enum KnownOp {
  sessionHello('session.hello'),
  sessionAuth('session.auth'),
  pairRequest('pair.request'),
  pairBegin('pair.begin'),
  pairCancel('pair.cancel'),
  pairListDevices('pair.list_devices'),
  pairRevoke('pair.revoke'),
  systemPing('system.ping'),
  inputKeyCombo('input.key_combo'),
  inputKeySequence('input.key_sequence'),
  inputText('input.text'),
  inputMedia('input.media'),
  inputMouse('input.mouse'),
  actionRun('action.run'),
  actionList('action.list'),
  actionSet('action.set'),
  actionDelete('action.delete'),
  profileGet('profile.get'),
  profileList('profile.list'),
  profileSubscribe('profile.subscribe'),
  profileActivate('profile.activate'),
  profileSet('profile.set'),
  profileDelete('profile.delete'),
  telemetrySubscribe('telemetry.subscribe'),
  settingsGet('settings.get'),
  settingsSet('settings.set'),

  // Events. These appear only with `t: "evt"` and never as a request.
  profileChanged('profile.changed'),
  telemetryUpdate('telemetry.update'),
  deviceChanged('device.changed'),
  pairingState('pairing.state'),
  engineShutdown('engine.shutdown');

  const KnownOp(this.wire);

  final String wire;

  static KnownOp? tryFromWire(String wire) {
    for (final op in values) {
      if (op.wire == wire) return op;
    }
    return null;
  }

  /// True for the five ops that only ever appear as `t: "evt"`.
  bool get isEvent => const {
    KnownOp.profileChanged,
    KnownOp.telemetryUpdate,
    KnownOp.deviceChanged,
    KnownOp.pairingState,
    KnownOp.engineShutdown,
  }.contains(this);
}

/// An empty payload. Serialises to `{}`, which is what the protocol requires — never
/// `null`.
class Empty implements Payload {
  const Empty();

  factory Empty.fromJson(Map<String, dynamic> _) => const Empty();

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{};

  @override
  bool operator ==(Object other) => other is Empty;

  @override
  int get hashCode => 0;
}

/// The payload of an `err` message. `docs/PROTOCOL.md` §2.1.
class ErrorPayload implements Payload {
  const ErrorPayload(this.code, this.message);

  factory ErrorPayload.fromJson(Map<String, dynamic> json) => ErrorPayload(
    ErrorCode.fromWire(json['code'] as String),
    json['message'] as String,
  );

  final ErrorCode code;
  final String message;

  @override
  Map<String, dynamic> toJson() => <String, dynamic>{
    'code': code.wire,
    'message': message,
  };
}

/// `docs/PROTOCOL.md` §2.1.
enum ErrorCode {
  /// Malformed envelope or payload.
  badRequest('BAD_REQUEST'),

  /// `v` is not 1.
  unsupportedVersion('UNSUPPORTED_VERSION'),

  /// No such op.
  unknownOp('UNKNOWN_OP'),

  /// Op requires a completed session handshake.
  notAuthenticated('NOT_AUTHENTICATED'),

  /// Role lacks capability for this op.
  notAuthorized('NOT_AUTHORIZED'),

  /// Not in pairing mode, or the window expired.
  pairingClosed('PAIRING_CLOSED'),

  /// Wrong one-time pairing code.
  badCode('BAD_CODE'),

  /// Device ID not in the registry.
  unknownDevice('UNKNOWN_DEVICE'),

  /// Challenge signature did not verify.
  badSignature('BAD_SIGNATURE'),

  /// The OS refused the input event.
  injectionFailed('INJECTION_FAILED'),

  /// Profile or action does not exist.
  notFound('NOT_FOUND'),

  /// Feature is switched off, e.g. shell execution.
  disabled('DISABLED'),

  /// Engine bug. Always logged with a trace ID.
  internal('INTERNAL');

  const ErrorCode(this.wire);

  final String wire;

  static ErrorCode fromWire(String wire) => values.firstWhere(
    (v) => v.wire == wire,
    orElse: () => throw ProtocolException(
      ErrorCode.badRequest,
      'unknown error code "$wire"',
    ),
  );
}
