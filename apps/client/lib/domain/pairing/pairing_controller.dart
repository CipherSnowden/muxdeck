/// Pairing: QR payload in, stored host record out. `docs/ARCHITECTURE.md` §5.2.
library;

import 'dart:convert';

import 'package:flutter_riverpod/flutter_riverpod.dart';
import 'package:muxdeck_protocol/muxdeck_protocol.dart';

import '../../data/hosts/host_record.dart';
import '../../data/hosts/host_store.dart';
import '../../data/identity/device_identity.dart';
import '../../providers.dart';
import '../session/session_controller.dart';

sealed class PairingFlowState {
  const PairingFlowState();
}

class PairingIdle extends PairingFlowState {
  const PairingIdle();
}

class PairingInProgress extends PairingFlowState {
  const PairingInProgress();
}

class PairingSucceeded extends PairingFlowState {
  const PairingSucceeded(this.host);

  final HostRecord host;
}

class PairingFailed extends PairingFlowState {
  const PairingFailed(this.error);

  final AppError error;
}

/// Builds a transport for an address whose fingerprint is known but which is not yet paired.
typedef PairingTransportFactory =
    Transport Function({required String address, required String fingerprint});

/// Runs the pairing exchange.
class PairingController extends Notifier<PairingFlowState> {
  PairingTransportFactory get _transportFactory =>
      ref.read(pairingTransportFactoryProvider);
  DeviceIdentityStore get _identityStore =>
      ref.read(deviceIdentityStoreProvider);
  HostStore get _hostStore => ref.read(hostStoreProvider);

  @override
  PairingFlowState build() => const PairingIdle();

  /// Pairs using a scanned QR payload.
  Future<void> pairFromQr(String rawPayload) async {
    final payload = PairingPayload.tryParse(rawPayload);
    if (payload == null) {
      state = const PairingFailed(
        PairingRejected('That QR code is not a MuxDeck pairing code.'),
      );
      return;
    }
    await pair(
      address: payload.address,
      hostId: payload.hostId,
      fingerprint: payload.fingerprint,
      code: payload.code,
    );
  }

  /// Pairs using a manually entered address and code.
  ///
  /// The host ID and fingerprint are unknown on this path, so they are learned from the
  /// `pair.request` response and the certificate actually presented. That is a weaker guarantee
  /// than the QR path — nothing was carried out of band, so this is trust-on-first-use — and it
  /// exists because mDNS is the part most likely to be blocked by a router
  /// (`docs/ARCHITECTURE.md` §4).
  Future<void> pairManually({required String address, required String code}) =>
      pair(address: address, hostId: null, fingerprint: null, code: code);

  Future<void> pair({
    required String address,
    required String? hostId,
    required String? fingerprint,
    required String code,
  }) async {
    state = const PairingInProgress();

    Transport? transport;
    ProtocolClient? client;

    try {
      final identity = await _identityStore.load();

      transport = _transportFactory(
        address: address,
        fingerprint: fingerprint ?? '',
      );
      await transport.connect();
      client = ProtocolClient(transport);

      // Proof of possession, built by muxdeck_protocol and fixture-tested byte-for-byte against
      // the engine. Without it, anyone who photographed the QR could register a public key they
      // do not hold the private half of. `docs/PROTOCOL.md` §4.2.
      final proof = await identity.sign(
        pairProofMessage(code: code, devicePubkey: identity.publicKey),
      );

      final response = await client.request(KnownOp.pairRequest, {
        'code': code,
        'device_pubkey': base64Encode(identity.publicKey),
        'device_name': await _deviceName(),
        'platform': currentPlatform.wire,
        'proof': base64Encode(proof),
      });

      final pairResponse = PairResponse.fromJson(response);

      // The engine derives the device ID from the public key exactly as this client does, so a
      // disagreement means the two are not hashing the same bytes — worth failing on rather
      // than storing a record that can never authenticate.
      if (pairResponse.deviceId != identity.deviceId) {
        throw const PairingRejected(
          'The host assigned a different device ID than expected. This is a bug; please report it.',
        );
      }

      final record = HostRecord(
        hostId: pairResponse.hostId,
        hostName: pairResponse.hostName,
        address: address,
        // On the QR path the fingerprint was pinned before a byte was exchanged. On the manual
        // path it is whatever the host presented, which is only as trustworthy as the network
        // was at that moment.
        fingerprint: fingerprint ?? _presentedFingerprint(transport),
        deviceId: pairResponse.deviceId,
      );

      await _hostStore.save(record);
      await _hostStore.setLastHostId(record.hostId);
      state = PairingSucceeded(record);
    } on AppError catch (error) {
      state = PairingFailed(_describe(error));
    } catch (error) {
      state = PairingFailed(PairingRejected('$error'));
    } finally {
      await client?.close();
      await transport?.close();
    }
  }

  /// Turns a wire error into something that says what to do next.
  AppError _describe(AppError error) {
    if (error is EngineRefused) {
      return switch (error.code) {
        'BAD_CODE' => const PairingRejected(
          'That pairing code is not correct. Check the code on your computer and try again.',
        ),
        'PAIRING_CLOSED' => const PairingRejected(
          'The pairing window has closed. Open a new one on your computer and try again.',
        ),
        'BAD_SIGNATURE' => const PairingRejected(
          'This device could not prove it owns its identity key. Try again, and if it keeps '
          'failing, clear the app data and re-pair.',
        ),
        _ => PairingRejected(error.message),
      };
    }
    return error;
  }

  void reset() => state = const PairingIdle();

  /// What the host shows in its device list.
  Future<String> _deviceName() async {
    // A real device name needs `device_info_plus`, which is not a dependency yet and buys one
    // string. The user can rename the device from the desktop panel, which is where they are
    // already looking when they pair.
    return switch (currentPlatform) {
      Platform.ios => 'iPhone or iPad',
      Platform.android => 'Android device',
      _ => 'MuxDeck client',
    };
  }

  /// The fingerprint of the certificate the host actually presented.
  String _presentedFingerprint(Transport transport) {
    // Only meaningful on the manual path, where nothing was known in advance.
    // The cast reads as redundant but is required: `Transport` and `FingerprintReporting` are
    // unrelated interfaces, so `is` does not promote the local inside a conditional expression.
    if (transport is FingerprintReporting) {
      return (transport as FingerprintReporting).presentedFingerprint ?? '';
    }
    return '';
  }
}
