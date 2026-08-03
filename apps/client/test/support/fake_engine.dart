/// A real engine, in-process, at the transport seam.
///
/// `docs/CLIENT.md` §8 asks for a fake in-process engine so the whole
/// discovery → pair → connect → press flow is testable without hardware. This is that engine,
/// built as a [Transport] rather than as a TLS WebSocket server on loopback.
///
/// The seam is the point. Everything the client does above [Transport] — envelope framing,
/// correlation IDs, the two-step handshake, the signing buffers, the error-code branches — is
/// exercised unchanged. What is skipped below it is the socket and the certificate, neither of
/// which the client's own logic decides anything from once a frame has arrived. A loopback
/// server would cost a generated certificate per test run and buy coverage of `dart:io`.
///
/// **The cryptography is not faked.** Both signatures the protocol defines are verified here
/// with a real [Ed25519] verifier over a buffer built by `muxdeck_protocol`'s own
/// `sessionAuthMessage` and `pairProofMessage`. That is the whole value of this file: those
/// buffers must be byte-identical to the ones the Rust engine builds, and a fake that accepted
/// any signature would pass just as happily while the client signed the wrong bytes.
library;

import 'dart:async';
import 'dart:convert';
import 'dart:math';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:cryptography/cryptography.dart';
import 'package:muxdeck_client/data/identity/device_identity.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

/// A well-formed but obviously invented certificate fingerprint — the one from
/// `docs/PROTOCOL.md` §4.2's `qr_payload` example.
///
/// Shape matters here even though the value does not: the client checks the length and the
/// alphabet before it ever compares one, so a placeholder like `'fingerprint'` would fail
/// [PairingPayload.tryParse] for a reason that has nothing to do with the test.
const fakeFingerprint =
    '3b1f8c07d2a94e65b0c3f7128d4a6e590fb27c8d41e93a05672bd8f4c1e0a937';

/// An engine that speaks the wire protocol without a wire.
class FakeEngine implements Transport, FingerprintReporting {
  FakeEngine._({
    required this.hostKeyPair,
    required this.hostId,
    required this.hostName,
    required this.fingerprint,
    required this.engineVersion,
  });

  /// Builds an engine with a freshly generated host identity.
  ///
  /// Asynchronous because key generation is, and the host ID is a function of the key — there is
  /// no honest way to hand back a fully-formed engine from a constructor.
  static Future<FakeEngine> create({
    String hostName = 'ENIGMA-ENTROPY',
    String fingerprint = fakeFingerprint,
    String engineVersion = '0.1.0',
  }) async {
    final keyPair = await Ed25519().newKeyPair();
    final publicKey = await keyPair.extractPublicKey();

    return FakeEngine._(
      hostKeyPair: keyPair,
      hostId: _hostIdFromPublicKey(publicKey.bytes),
      hostName: hostName,
      fingerprint: fingerprint,
      engineVersion: engineVersion,
    );
  }

  /// Held rather than used. Nothing in the protocol asks the host to sign anything, but the host
  /// ID is derived from this key, and deriving it from a real one is what makes the ID a real
  /// 18-character `h_…` string instead of a literal a test could accidentally depend on.
  final SimpleKeyPair hostKeyPair;

  /// `"h_"` followed by 16 lowercase hex characters. `docs/PROTOCOL.md` §2.2.
  final String hostId;

  final String hostName;

  /// What a client would have pinned. Never checked here — TLS is below this seam — but it is
  /// reported, so the manual-pairing path, which stores whatever the host presented, has
  /// something to store.
  final String fingerprint;

  final String engineVersion;

  /// Satisfies the manual-pairing path in `PairingController`, which asks the transport what
  /// certificate it saw because nothing was carried out of band on that route.
  @override
  String? get presentedFingerprint => fingerprint;

  /// Every `input.*` request this engine received, as whole envelopes.
  ///
  /// The envelope rather than just the payload, so a test asserting that a button press
  /// dispatched the right op (`docs/CLIENT.md` §8) can read `['op']` and `['d']` from one place.
  final List<Map<String, dynamic>> receivedInput = <Map<String, dynamic>>[];

  final _frames = StreamController<String>.broadcast();
  final _random = Random.secure();

  /// Public keys of paired devices, by the device ID derived from them.
  final _devices = <String, SimplePublicKey>{};

  /// The six digits a `pair.request` must quote, or null when no window is open.
  String? _pairingCode;

  /// The nonce this connection is waiting on an answer to, and who it was issued to.
  ///
  /// Per-connection rather than per-device: a challenge is consumed by the `session.auth` that
  /// answers it, and one arriving with no challenge outstanding is `NOT_AUTHENTICATED` rather
  /// than a signature to check.
  Uint8List? _nonce;
  String? _challengedDeviceId;

  var _connected = false;

  // --- Test controls ------------------------------------------------------------------------

  /// Opens a pairing window that will accept [code].
  ///
  /// No expiry. Wall-clock time is the one thing a test cannot afford to wait on, and an expired
  /// window is indistinguishable from a closed one on the wire — both are `PAIRING_CLOSED`, so
  /// [closePairingWindow] already covers it.
  void openPairingWindow(String code) => _pairingCode = code;

  void closePairingWindow() => _pairingCode = null;

  /// Registers a device without going through `pair.request`, for tests about what happens
  /// *after* pairing.
  ///
  /// Derives the device ID with the client's own [deviceIdFromPublicKey] rather than a copy of
  /// the rule: the engine and the client agreeing on that derivation is a property under test
  /// elsewhere, not something this fake should get a second opinion on.
  String registerDevice(List<int> publicKey) {
    final deviceId = deviceIdFromPublicKey(publicKey);
    _devices[deviceId] = SimplePublicKey(publicKey, type: KeyPairType.ed25519);
    return deviceId;
  }

  /// Drops the connection the way a dead socket does — the frame stream ends and everything
  /// waiting on a response fails.
  ///
  /// Identical to [close] by construction, and named separately anyway: a test about
  /// reconnection is describing a host that went away, not a client that hung up, and the two
  /// read very differently at the call site even though the layer above cannot tell them apart.
  Future<void> simulateDisconnect() => close();

  // --- Transport ----------------------------------------------------------------------------

  @override
  Future<void> connect() async => _connected = true;

  @override
  void send(String frame) {
    if (!_connected) throw const TransportFailed('Not connected.');

    // Never answered inline. A caller awaiting a response would otherwise be completed inside
    // its own `send`, which no socket does and which hides ordering bugs that only appear once
    // a real network is in the way.
    unawaited(Future.microtask(() => _handle(frame)));
  }

  @override
  Stream<String> get frames => _frames.stream;

  @override
  Future<void> close() async {
    _connected = false;
    _nonce = null;
    _challengedDeviceId = null;
    if (!_frames.isClosed) await _frames.close();
  }

  // --- Dispatch -----------------------------------------------------------------------------

  Future<void> _handle(String frame) async {
    final Map<String, dynamic> envelope;
    try {
      envelope = jsonDecode(frame) as Map<String, dynamic>;
    } catch (_) {
      // Nothing to echo an ID from, so nothing to answer. A real engine closes the socket here;
      // staying quiet keeps the failure looking like a timeout, which is what a test would
      // assert on either way.
      return;
    }

    final id = envelope['id'] as String?;
    final op = envelope['op'] as String?;
    if (id == null || op == null) return;

    final payload =
        envelope['d'] as Map<String, dynamic>? ?? const <String, dynamic>{};

    switch (KnownOp.tryFromWire(op)) {
      case KnownOp.sessionHello:
        await _hello(id: id, op: op, payload: payload);
      case KnownOp.sessionAuth:
        await _auth(id: id, op: op, payload: payload);
      case KnownOp.pairRequest:
        await _pair(id: id, op: op, payload: payload);
      case KnownOp.systemPing:
        _respond(
          id: id,
          op: op,
          payload: <String, dynamic>{
            't_client': payload['t_client'],
            't_engine': DateTime.now().millisecondsSinceEpoch,
          },
        );
      case KnownOp.inputKeyCombo ||
          KnownOp.inputKeySequence ||
          KnownOp.inputText ||
          KnownOp.inputMedia ||
          KnownOp.inputMouse:
        receivedInput.add(envelope);
        _respond(id: id, op: op, payload: const <String, dynamic>{});
      default:
        _refuse(
          id: id,
          op: op,
          code: ErrorCode.unknownOp,
          message: 'this fake engine does not implement "$op"',
        );
    }
  }

  /// `session.hello`, deck form. `docs/PROTOCOL.md` §4.1.
  Future<void> _hello({
    required String id,
    required String op,
    required Map<String, dynamic> payload,
  }) async {
    final deviceId = payload['device_id'] as String?;
    if (deviceId == null) {
      // The admin form exists in the protocol but not here: it belongs to the desktop panel over
      // loopback, and this fake stands in for a host talking to a deck.
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.badRequest,
        message:
            'session.hello without device_id; this fake speaks only the deck form',
      );
      return;
    }

    if (!_devices.containsKey(deviceId)) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.unknownDevice,
        message: 'device $deviceId is not in the registry',
      );
      return;
    }

    final nonce = Uint8List.fromList(
      List<int>.generate(nonceLength, (_) => _random.nextInt(256)),
    );
    _nonce = nonce;
    _challengedDeviceId = deviceId;

    _respond(
      id: id,
      op: op,
      payload: Challenge(
        nonce: base64Encode(nonce),
        hostId: hostId,
        hostName: hostName,
      ).toJson(),
    );
  }

  /// `session.auth`. The signature is verified for real, against the buffer
  /// `muxdeck_protocol` builds.
  Future<void> _auth({
    required String id,
    required String op,
    required Map<String, dynamic> payload,
  }) async {
    final nonce = _nonce;
    final deviceId = _challengedDeviceId;
    final devicePublicKey = deviceId == null ? null : _devices[deviceId];
    if (nonce == null || deviceId == null || devicePublicKey == null) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.notAuthenticated,
        message: 'session.auth with no challenge outstanding',
      );
      return;
    }

    final signature = _decodeExactly(payload['signature'], signatureLength);
    if (signature == null) {
      // A wrong length is a malformed message, not a failed verification —
      // `docs/PROTOCOL.md` §2 is explicit that this is `BAD_REQUEST`.
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.badRequest,
        message: 'signature must be $signatureLength bytes, base64',
      );
      return;
    }

    final verified = await Ed25519().verify(
      sessionAuthMessage(nonce: nonce, deviceId: deviceId, hostId: hostId),
      signature: Signature(signature, publicKey: devicePublicKey),
    );
    if (!verified) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.badSignature,
        message: 'the challenge signature did not verify',
      );
      return;
    }

    // Consumed. A nonce that answered once must not answer again.
    _nonce = null;
    _challengedDeviceId = null;

    _respond(id: id, op: op, payload: _ready.toJson());
  }

  /// `pair.request`. `docs/PROTOCOL.md` §4.2.
  ///
  /// Checked in the order the protocol distinguishes them — window, then code, then proof — so
  /// a client that gets the code wrong is told so rather than being told its key is bad.
  Future<void> _pair({
    required String id,
    required String op,
    required Map<String, dynamic> payload,
  }) async {
    final openCode = _pairingCode;
    if (openCode == null) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.pairingClosed,
        message: 'no pairing window is open',
      );
      return;
    }

    final code = payload['code'];
    if (code is! String || code != openCode) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.badCode,
        message: 'that is not the current pairing code',
      );
      return;
    }

    final pubkey = _decodeExactly(payload['device_pubkey'], pubkeyLength);
    final proof = _decodeExactly(payload['proof'], signatureLength);
    if (pubkey == null || proof == null) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.badRequest,
        message:
            'device_pubkey must be $pubkeyLength bytes and proof $signatureLength, base64',
      );
      return;
    }

    final verified = await Ed25519().verify(
      pairProofMessage(code: code, devicePubkey: pubkey),
      signature: Signature(
        proof,
        // Deliberately the key being registered, not one already on file: the proof is what
        // establishes that this device holds the private half of the key it is presenting.
        publicKey: SimplePublicKey(pubkey, type: KeyPairType.ed25519),
      ),
    );
    if (!verified) {
      _refuse(
        id: id,
        op: op,
        code: ErrorCode.badSignature,
        message: 'the proof of possession did not verify',
      );
      return;
    }

    // The window stays open. A real engine burns the code on success; leaving it open here means
    // a test that pairs twice is testing what it meant to, and [closePairingWindow] is right
    // there when the closed case is the point.
    final deviceId = registerDevice(pubkey);

    _respond(
      id: id,
      op: op,
      payload: PairResponse(
        deviceId: deviceId,
        hostId: hostId,
        hostName: hostName,
      ).toJson(),
    );
  }

  /// What this host claims it can do. `shell_actions` is false because
  /// `docs/ARCHITECTURE.md` §5.5 has it off by default, and a fake that reported otherwise would
  /// let a capability-gating bug through.
  Ready get _ready => Ready(
    role: Role.deck,
    protocol: protocolVersion,
    engineVersion: engineVersion,
    hostPlatform: HostPlatform.windows,
    activeProfileId: 'p_default',
    capabilities: const Capabilities(
      textUnicode: true,
      mediaKeys: true,
      mouse: true,
      shellActions: false,
    ),
  );

  // --- Framing ------------------------------------------------------------------------------

  /// A `res`, echoing the request's `id` and `op` as `docs/PROTOCOL.md` §2 requires.
  void _respond({
    required String id,
    required String op,
    required Map<String, dynamic> payload,
  }) => _emit(type: MessageType.res, id: id, op: op, payload: payload);

  /// An `err`, carrying the code the client branches on.
  void _refuse({
    required String id,
    required String op,
    required ErrorCode code,
    required String message,
  }) => _emit(
    type: MessageType.err,
    id: id,
    op: op,
    payload: ErrorPayload(code, message).toJson(),
  );

  void _emit({
    required MessageType type,
    required String id,
    required String op,
    required Map<String, dynamic> payload,
  }) {
    if (_frames.isClosed) return;
    _frames.add(
      jsonEncode(<String, dynamic>{
        'v': protocolVersion,
        't': type.wire,
        'id': id,
        'op': op,
        'd': payload,
      }),
    );
  }

  /// Base64 of exactly [expectedLength] bytes, or null.
  ///
  /// One function for every binary field because §2 gives them all a fixed decoded length and
  /// the same verdict when they miss it.
  Uint8List? _decodeExactly(Object? value, int expectedLength) {
    if (value is! String) return null;
    try {
      final bytes = base64Decode(value);
      return bytes.length == expectedLength ? bytes : null;
    } on FormatException {
      return null;
    }
  }
}

/// `"h_"` + the first 16 hex characters of SHA-256 over the raw host public key.
/// `docs/PROTOCOL.md` §2.2 — the device rule with a different prefix.
String _hostIdFromPublicKey(List<int> publicKey) {
  final digest = sha256.convert(publicKey).toString();
  return 'h_${digest.substring(0, 16)}';
}

/// A transport that never connects.
///
/// Exists so the failure branches can be tested by the error they produce rather than by
/// contriving a network condition: [FingerprintMismatch] and [HostUnreachable] have to reach the
/// user differently, and the only thing that distinguishes them above the transport is which
/// [AppError] came out of [connect].
class FailingTransport implements Transport {
  const FailingTransport(this.error);

  final AppError error;

  @override
  Future<void> connect() async => throw error;

  @override
  void send(String frame) => throw const TransportFailed('Not connected.');

  @override
  Stream<String> get frames => const Stream<String>.empty();

  @override
  Future<void> close() async {}
}
