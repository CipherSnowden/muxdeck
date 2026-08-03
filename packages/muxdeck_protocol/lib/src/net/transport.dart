/// The transport seam.
///
/// One implementation exists today ([LanTransport]). The interface is here for two reasons:
/// it is where a USB transport would be added if that decision is ever revisited
/// (`docs/TRANSPORT.md` §5), and — more immediately — it is what lets the handshake, pairing
/// and dispatch logic be tested without a socket, a certificate or a running engine.
library;

/// A bidirectional stream of protocol frames.
///
/// Frames are the raw JSON text of `docs/PROTOCOL.md` §2 envelopes. Encoding and decoding
/// happen above this layer, so a transport never has to understand the protocol.
abstract interface class Transport {
  /// Opens the connection. Throws an [AppError] subclass on failure — in particular
  /// [FingerprintMismatch], which callers must not treat as a retryable error.
  Future<void> connect();

  /// Queues a frame. Not awaited: a keypress must not wait on the network, and a press that
  /// cannot be sent is dropped rather than queued (`docs/CLIENT.md` §7).
  void send(String frame);

  /// Frames from the host, including unsolicited events.
  Stream<String> get frames;

  Future<void> close();
}

/// Implemented by transports that can report the certificate they saw.
///
/// Needed only by manual pairing, where the user typed an address and no fingerprint was
/// carried out of band, so the only way to learn one is to look at what the host presented. The
/// QR path never consults this — it knows the fingerprint before the first byte.
abstract interface class FingerprintReporting {
  String? get presentedFingerprint;
}
