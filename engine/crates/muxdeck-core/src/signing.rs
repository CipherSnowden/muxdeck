//! The exact byte layouts that get signed. `docs/PROTOCOL.md` §4.1 and §4.2.
//!
//! These are the highest-risk few lines in the protocol. Rust and Dart must build a
//! byte-identical buffer; a mismatch authenticates nothing, produces no error message
//! worth reading, and is miserable to diagnose. Both sides are therefore tested against
//! the same fixtures in `protocol/fixtures/signing/`, as raw bytes rather than as JSON.
//!
//! No base64 here on purpose. These functions take and return raw bytes, so this crate
//! needs no codec dependency and there is exactly one place where encoding could go wrong
//! — the caller's, at the edge.

/// Domain separator for the session challenge. 18 ASCII bytes.
pub const SESSION_DOMAIN: &[u8] = b"muxdeck-session-v1";

/// Domain separator for the pairing proof of possession. 15 ASCII bytes.
pub const PAIR_DOMAIN: &[u8] = b"muxdeck-pair-v1";

/// Expected length of a `nonce`, a `device_pubkey` and an `admin_token`, in bytes.
pub const NONCE_LEN: usize = 32;
/// Expected length of an Ed25519 public key, in bytes.
pub const PUBKEY_LEN: usize = 32;
/// Expected length of an Ed25519 signature, in bytes.
pub const SIGNATURE_LEN: usize = 64;

/// The buffer a device signs to answer a `session.hello` challenge.
///
/// ```text
/// b"muxdeck-session-v1" || nonce (32 raw bytes) || device_id (UTF-8) || host_id (UTF-8)
/// ```
///
/// No separators and no terminator. The domain prefix and the trailing `host_id` are what
/// stop a signature captured against one host being replayed at another.
pub fn session_auth_message(nonce: &[u8], device_id: &str, host_id: &str) -> Vec<u8> {
    let mut buf =
        Vec::with_capacity(SESSION_DOMAIN.len() + nonce.len() + device_id.len() + host_id.len());
    buf.extend_from_slice(SESSION_DOMAIN);
    buf.extend_from_slice(nonce);
    buf.extend_from_slice(device_id.as_bytes());
    buf.extend_from_slice(host_id.as_bytes());
    buf
}

/// The buffer a device signs to prove it holds the private half of the key it is
/// registering.
///
/// ```text
/// b"muxdeck-pair-v1" || code (UTF-8 of the 6 digits) || device_pubkey (32 raw bytes)
/// ```
pub fn pair_proof_message(code: &str, device_pubkey: &[u8]) -> Vec<u8> {
    let mut buf = Vec::with_capacity(PAIR_DOMAIN.len() + code.len() + device_pubkey.len());
    buf.extend_from_slice(PAIR_DOMAIN);
    buf.extend_from_slice(code.as_bytes());
    buf.extend_from_slice(device_pubkey);
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn domain_separator_lengths_match_the_spec() {
        // docs/PROTOCOL.md states these byte counts inline; if either changes, every
        // previously issued signature stops verifying, so the numbers are asserted.
        assert_eq!(SESSION_DOMAIN.len(), 18);
        assert_eq!(PAIR_DOMAIN.len(), 15);
    }

    #[test]
    fn session_message_is_a_plain_concatenation() {
        let msg = session_auth_message(
            &[0xAB; NONCE_LEN],
            "d_0000000000000001",
            "h_0000000000000002",
        );
        assert_eq!(msg.len(), 18 + 32 + 18 + 18);
        assert!(msg.starts_with(SESSION_DOMAIN));
        assert!(msg.ends_with(b"h_0000000000000002"));
    }

    #[test]
    fn pair_message_is_a_plain_concatenation() {
        let msg = pair_proof_message("402913", &[0xCD; PUBKEY_LEN]);
        assert_eq!(msg.len(), 15 + 6 + 32);
        assert!(msg.starts_with(PAIR_DOMAIN));
        assert!(msg.ends_with(&[0xCD; PUBKEY_LEN]));
    }
}
