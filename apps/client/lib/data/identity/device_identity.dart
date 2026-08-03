/// This device's Ed25519 identity.
///
/// Generated once on first launch and never transmitted — only the public half ever leaves the
/// device, and only during pairing. `docs/ARCHITECTURE.md` §5.1.
library;

import 'package:muxdeck_protocol/muxdeck_protocol.dart';
import 'dart:typed_data';

import 'package:crypto/crypto.dart';
import 'package:cryptography/cryptography.dart';
import 'package:flutter_secure_storage/flutter_secure_storage.dart';


/// Where the 32-byte Ed25519 seed lives in secure storage.
const _seedKey = 'muxdeck.device.seed';

/// Ed25519 seeds are 32 bytes. `newKeyPairFromSeed` throws `ArgumentError` on anything else.
const _seedLength = 32;

/// The device's keypair, plus the ID derived from it.
class DeviceIdentity {
  const DeviceIdentity({
    required this.keyPair,
    required this.publicKey,
    required this.deviceId,
  });

  final SimpleKeyPair keyPair;

  /// Raw 32 bytes, as they travel on the wire (base64-encoded by the caller).
  final Uint8List publicKey;

  /// `"d_"` followed by 16 lowercase hex characters. `docs/PROTOCOL.md` §2.2.
  final String deviceId;

  /// Signs [message] and returns the raw 64-byte signature.
  ///
  /// The caller supplies the exact bytes to sign — always via `muxdeck_protocol`'s
  /// `sessionAuthMessage` or `pairProofMessage`, never assembled inline. Those layouts are
  /// fixture-tested byte-for-byte against the Rust engine; a disagreement authenticates nothing
  /// and produces no diagnosable symptom.
  Future<Uint8List> sign(List<int> message) async {
    final signature = await Ed25519().sign(message, keyPair: keyPair);
    return Uint8List.fromList(signature.bytes);
  }
}

/// Loads the device identity, generating it on first launch.
class DeviceIdentityStore {
  DeviceIdentityStore({FlutterSecureStorage? storage})
    : _storage =
          storage ??
          const FlutterSecureStorage(
            // resetOnError defaults to true in 10.x, which silently wipes stored data when a
            // read fails — unpairing the device with no message and no way to diagnose it.
            // Losing the key is the worst outcome here, so failures surface instead.
            aOptions: AndroidOptions(resetOnError: false),
            // The default (`unlocked`) makes reads fail on a locked device and permits iCloud
            // sync. A deck may well be launched by a shortcut before first unlock, and a device
            // identity must never leave the device it identifies.
            iOptions: IOSOptions(
              accessibility: KeychainAccessibility.first_unlock_this_device,
            ),
          );

  final FlutterSecureStorage _storage;

  DeviceIdentity? _cached;

  /// The identity for this device, generated on first call and cached thereafter.
  Future<DeviceIdentity> load() async {
    final cached = _cached;
    if (cached != null) return cached;

    final seed = await _readSeed() ?? await _generateAndStoreSeed();
    final identity = await _fromSeed(seed);
    _cached = identity;
    return identity;
  }

  Future<Uint8List?> _readSeed() async {
    final String? stored;
    try {
      stored = await _storage.read(key: _seedKey);
    } catch (e) {
      throw TransportFailed('Could not read this device\'s identity key: $e');
    }
    if (stored == null) return null;

    final bytes = _decodeHex(stored);
    if (bytes == null || bytes.length != _seedLength) {
      // Refusing rather than silently regenerating: a new identity means a new device ID, so
      // every paired host would stop recognising this device with no explanation. Better to say
      // so than to quietly unpair.
      throw const TransportFailed(
        'This device\'s stored identity key is corrupt. Clear the app\'s data and pair again.',
      );
    }
    return bytes;
  }

  Future<Uint8List> _generateAndStoreSeed() async {
    final keyPair = await Ed25519().newKeyPair();
    final seed = Uint8List.fromList(await keyPair.extractPrivateKeyBytes());

    try {
      await _storage.write(key: _seedKey, value: _encodeHex(seed));
    } catch (e) {
      throw TransportFailed('Could not save this device\'s identity key: $e');
    }
    return seed;
  }

  Future<DeviceIdentity> _fromSeed(Uint8List seed) async {
    final keyPair = await Ed25519().newKeyPairFromSeed(seed);
    final publicKey = Uint8List.fromList((await keyPair.extractPublicKey()).bytes);

    return DeviceIdentity(
      keyPair: keyPair,
      publicKey: publicKey,
      deviceId: deviceIdFromPublicKey(publicKey),
    );
  }
}

/// `"d_"` + the first 16 hex characters of SHA-256 over the raw public key.
///
/// The engine derives the same string from the same bytes (`identity.rs`), so neither side has
/// to transmit it and there is exactly one representation to agree on. `docs/PROTOCOL.md` §2.2.
String deviceIdFromPublicKey(List<int> publicKey) {
  final digest = sha256.convert(publicKey).bytes;
  return 'd_${_encodeHex(digest.sublist(0, 8))}';
}

/// Lowercase hex, no separators — the representation used everywhere IDs and fingerprints
/// appear.
String _encodeHex(List<int> bytes) {
  final buffer = StringBuffer();
  for (final byte in bytes) {
    buffer.write(byte.toRadixString(16).padLeft(2, '0'));
  }
  return buffer.toString();
}

/// Returns null when [value] is not valid lowercase hex of even length.
Uint8List? _decodeHex(String value) {
  if (value.length.isOdd) return null;
  final out = Uint8List(value.length ~/ 2);
  for (var i = 0; i < out.length; i++) {
    final byte = int.tryParse(value.substring(i * 2, i * 2 + 2), radix: 16);
    if (byte == null) return null;
    out[i] = byte;
  }
  return out;
}
