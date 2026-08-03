/// The exact byte layouts that get signed. `docs/PROTOCOL.md` §4.1 and §4.2.
///
/// These are the highest-risk few lines in the protocol. Dart and Rust must build a
/// byte-identical buffer; a mismatch authenticates nothing, produces no error message worth
/// reading, and is miserable to diagnose. Both sides are tested against the same fixtures
/// in `protocol/fixtures/signing/`, as raw bytes rather than as JSON.
///
/// No base64 here on purpose. These functions take and return raw bytes, so there is
/// exactly one place where encoding could go wrong — the caller's, at the edge.
library;

import 'dart:convert';
import 'dart:typed_data';

/// Domain separator for the session challenge. 18 ASCII bytes.
final Uint8List sessionDomain = Uint8List.fromList(
  ascii.encode('muxdeck-session-v1'),
);

/// Domain separator for the pairing proof of possession. 15 ASCII bytes.
final Uint8List pairDomain = Uint8List.fromList(
  ascii.encode('muxdeck-pair-v1'),
);

/// Expected length of a nonce, a device public key and an admin token, in bytes.
const int nonceLength = 32;

/// Expected length of an Ed25519 public key, in bytes.
const int pubkeyLength = 32;

/// Expected length of an Ed25519 signature, in bytes.
const int signatureLength = 64;

/// The buffer a device signs to answer a `session.hello` challenge.
///
/// ```text
/// b"muxdeck-session-v1" || nonce (32 raw bytes) || device_id (UTF-8) || host_id (UTF-8)
/// ```
///
/// No separators and no terminator. The domain prefix and the trailing `hostId` are what
/// stop a signature captured against one host being replayed at another.
Uint8List sessionAuthMessage({
  required List<int> nonce,
  required String deviceId,
  required String hostId,
}) {
  final builder = BytesBuilder(copy: false)
    ..add(sessionDomain)
    ..add(nonce)
    ..add(utf8.encode(deviceId))
    ..add(utf8.encode(hostId));
  return builder.toBytes();
}

/// The buffer a device signs to prove it holds the private half of the key it is
/// registering.
///
/// ```text
/// b"muxdeck-pair-v1" || code (UTF-8 of the 6 digits) || device_pubkey (32 raw bytes)
/// ```
Uint8List pairProofMessage({
  required String code,
  required List<int> devicePubkey,
}) {
  final builder = BytesBuilder(copy: false)
    ..add(pairDomain)
    ..add(utf8.encode(code))
    ..add(devicePubkey);
  return builder.toBytes();
}
