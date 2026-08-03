//! The pairing window, its one-time code, and proof-of-possession verification.
//!
//! `docs/ARCHITECTURE.md` §5.2 and `docs/PROTOCOL.md` §4.2.

use std::time::{Duration, Instant};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use ed25519_dalek::{Signature, VerifyingKey};
use muxdeck_core::pairing::{DEFAULT_TTL_SECONDS, TTL_RANGE_SECONDS};
use muxdeck_core::signing::{pair_proof_message, PUBKEY_LEN, SIGNATURE_LEN};
use muxdeck_core::{ErrorCode, PairRequest};

use crate::error::{EngineError, Result};
use crate::identity::generate_otp;
use crate::registry::unix_now;

/// An open pairing window.
#[derive(Debug, Clone)]
pub struct PairingWindow {
    code: String,
    opened_at: Instant,
    ttl: Duration,
    /// Unix seconds, for the wire. Kept alongside the monotonic clock rather than derived from
    /// it: `Instant` decides whether the window is open, because a wall clock can jump.
    expires_at: i64,
}

impl PairingWindow {
    /// Opens a window, generating a fresh six-digit code.
    ///
    /// `ttl_seconds` must already have passed `PairBeginRequest::validate`; out-of-range values
    /// are rejected there rather than clamped here, so a client bug surfaces instead of being
    /// silently corrected.
    pub fn open(ttl_seconds: Option<u32>) -> Result<Self> {
        let ttl_seconds = ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS);
        debug_assert!(TTL_RANGE_SECONDS.contains(&ttl_seconds));

        Ok(Self {
            code: generate_otp()?,
            opened_at: Instant::now(),
            ttl: Duration::from_secs(u64::from(ttl_seconds)),
            expires_at: unix_now() + i64::from(ttl_seconds),
        })
    }

    pub fn code(&self) -> &str {
        &self.code
    }

    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }

    pub fn is_open(&self) -> bool {
        self.opened_at.elapsed() < self.ttl
    }

    /// Builds the QR payload. `docs/PROTOCOL.md` §4.2.
    ///
    /// Parameter order is fixed at `addr`, `host`, `fp`, `code` and none of the four needs
    /// percent-encoding: the address is numeric and the IDs and fingerprint are hex.
    pub fn qr_payload(&self, addr: &str, host_id: &str, fingerprint: &str) -> String {
        format!(
            "muxdeck://pair?addr={addr}&host={host_id}&fp={fingerprint}&code={}",
            self.code
        )
    }
}

/// Checks a `pair.request` against an open window and returns the verified public key.
///
/// Both the code and the proof are checked before anything is written, and the code is compared
/// in constant time so a wrong guess leaks nothing about how wrong it was.
pub fn verify_pair_request(
    window: Option<&PairingWindow>,
    request: &PairRequest,
) -> Result<[u8; PUBKEY_LEN]> {
    let window = window.filter(|w| w.is_open()).ok_or_else(|| {
        EngineError::wire(
            ErrorCode::PairingClosed,
            "the host is not accepting new devices right now",
        )
    })?;

    if !constant_time_eq(window.code.as_bytes(), request.code.as_bytes()) {
        return Err(EngineError::wire(
            ErrorCode::BadCode,
            "incorrect pairing code",
        ));
    }

    let pubkey = decode_fixed::<PUBKEY_LEN>(&request.device_pubkey, "device_pubkey")?;
    let proof = decode_fixed::<SIGNATURE_LEN>(&request.proof, "proof")?;

    let verifying_key = VerifyingKey::from_bytes(&pubkey).map_err(|_| {
        EngineError::wire(
            ErrorCode::BadRequest,
            "device_pubkey is not a valid Ed25519 key",
        )
    })?;
    let signature = Signature::from_bytes(&proof);
    let message = pair_proof_message(&request.code, &pubkey);

    verifying_key
        .verify_strict(&message, &signature)
        .map_err(|_| {
            EngineError::wire(
                ErrorCode::BadSignature,
                "proof of possession did not verify against device_pubkey",
            )
        })?;

    Ok(pubkey)
}

/// Decodes a base64 field that must be exactly `N` bytes.
///
/// A wrong length is `BAD_REQUEST` rather than `BAD_SIGNATURE`: it is a malformed message, not a
/// failed verification. `docs/PROTOCOL.md` §2.
pub fn decode_fixed<const N: usize>(value: &str, field: &'static str) -> Result<[u8; N]> {
    let bytes = BASE64.decode(value).map_err(|_| {
        EngineError::wire(
            ErrorCode::BadRequest,
            format!("{field} is not valid base64"),
        )
    })?;
    bytes.as_slice().try_into().map_err(|_| {
        EngineError::wire(
            ErrorCode::BadRequest,
            format!(
                "{field} must decode to exactly {N} bytes, got {}",
                bytes.len()
            ),
        )
    })
}

/// Compares two byte strings without leaking where they first differ.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    use muxdeck_core::Platform;

    fn signing_key() -> SigningKey {
        SigningKey::from_bytes(&[42u8; 32])
    }

    fn request_for(window: &PairingWindow, code_override: Option<&str>) -> PairRequest {
        let key = signing_key();
        let pubkey = key.verifying_key().to_bytes();
        let code = code_override.unwrap_or(window.code()).to_string();
        let proof = key.sign(&pair_proof_message(&code, &pubkey));

        PairRequest {
            code,
            device_pubkey: BASE64.encode(pubkey),
            device_name: "Cipher's iPad".into(),
            platform: Platform::Ios,
            proof: BASE64.encode(proof.to_bytes()),
        }
    }

    #[test]
    fn a_correct_request_verifies() {
        let window = PairingWindow::open(Some(120)).expect("open");
        let request = request_for(&window, None);
        let pubkey = verify_pair_request(Some(&window), &request).expect("verify");
        assert_eq!(pubkey, signing_key().verifying_key().to_bytes());
    }

    #[test]
    fn a_wrong_code_is_rejected() {
        let window = PairingWindow::open(Some(120)).expect("open");
        let wrong = if window.code() == "000000" {
            "111111"
        } else {
            "000000"
        };
        let request = request_for(&window, Some(wrong));

        let err = verify_pair_request(Some(&window), &request).expect_err("wrong code");
        assert_eq!(err.to_payload().code, ErrorCode::BadCode);
    }

    #[test]
    fn a_proof_signed_over_a_different_code_is_rejected() {
        // The attack this blocks: someone who saw the QR code replaying a proof they captured
        // from an earlier pairing window.
        let window = PairingWindow::open(Some(120)).expect("open");
        let mut request = request_for(&window, None);
        let key = signing_key();
        let pubkey = key.verifying_key().to_bytes();
        request.proof = BASE64.encode(key.sign(&pair_proof_message("999999", &pubkey)).to_bytes());

        let err = verify_pair_request(Some(&window), &request).expect_err("stale proof");
        assert_eq!(err.to_payload().code, ErrorCode::BadSignature);
    }

    #[test]
    fn a_proof_by_a_key_the_sender_does_not_hold_is_rejected() {
        // Without the proof, anyone who read the QR could register a public key they do not
        // control. `docs/PROTOCOL.md` §4.2.
        let window = PairingWindow::open(Some(120)).expect("open");
        let mut request = request_for(&window, None);
        let impostor = SigningKey::from_bytes(&[7u8; 32]);
        request.device_pubkey = BASE64.encode(impostor.verifying_key().to_bytes());

        let err = verify_pair_request(Some(&window), &request).expect_err("mismatched key");
        assert_eq!(err.to_payload().code, ErrorCode::BadSignature);
    }

    #[test]
    fn no_window_means_pairing_closed() {
        let window = PairingWindow::open(Some(120)).expect("open");
        let request = request_for(&window, None);

        let err = verify_pair_request(None, &request).expect_err("no window");
        assert_eq!(err.to_payload().code, ErrorCode::PairingClosed);
    }

    #[test]
    fn an_expired_window_means_pairing_closed() {
        let mut window = PairingWindow::open(Some(30)).expect("open");
        window.ttl = Duration::from_millis(0);
        let request = request_for(&window, None);

        assert!(!window.is_open());
        let err = verify_pair_request(Some(&window), &request).expect_err("expired");
        assert_eq!(err.to_payload().code, ErrorCode::PairingClosed);
    }

    #[test]
    fn a_wrong_length_key_is_bad_request_not_bad_signature() {
        let window = PairingWindow::open(Some(120)).expect("open");
        let mut request = request_for(&window, None);
        request.device_pubkey = BASE64.encode([1u8; 16]);

        let err = verify_pair_request(Some(&window), &request).expect_err("short key");
        assert_eq!(
            err.to_payload().code,
            ErrorCode::BadRequest,
            "a malformed field is not a failed verification"
        );
    }

    #[test]
    fn the_qr_payload_has_the_documented_parameter_order() {
        let window = PairingWindow::open(Some(120)).expect("open");
        let payload = window.qr_payload("192.168.1.42:47654", "h_a91c4d2e8f019b37", "abc123");

        assert!(payload.starts_with("muxdeck://pair?addr="));
        let addr = payload.find("addr=").expect("addr");
        let host = payload.find("host=").expect("host");
        let fp = payload.find("fp=").expect("fp");
        let code = payload.find("code=").expect("code");
        assert!(
            addr < host && host < fp && fp < code,
            "order is addr, host, fp, code"
        );
        assert!(payload.ends_with(window.code()));
    }

    #[test]
    fn constant_time_eq_still_compares_correctly() {
        assert!(constant_time_eq(b"402913", b"402913"));
        assert!(!constant_time_eq(b"402913", b"402914"));
        assert!(!constant_time_eq(b"402913", b"40291"));
    }
}
