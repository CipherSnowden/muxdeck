//! Pairing payloads. `docs/PROTOCOL.md` §4.2.

use serde::{Deserialize, Serialize};

use crate::envelope::{ErrorCode, ErrorPayload};
use crate::session::Platform;

/// The inclusive range `ttl_seconds` is clamped to. A value outside it is
/// [`ErrorCode::BadRequest`], not silently coerced.
pub const TTL_RANGE_SECONDS: std::ops::RangeInclusive<u32> = 30..=300;

/// The pairing window length used when `ttl_seconds` is omitted.
pub const DEFAULT_TTL_SECONDS: u32 = 120;

/// `pair.request` — callable only during a pairing window, by an unauthenticated socket.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairRequest {
    /// The six-digit one-time code.
    pub code: String,
    /// Ed25519 public key, 32 bytes, base64.
    pub device_pubkey: String,
    pub device_name: String,
    pub platform: Platform,
    /// Ed25519 signature, 64 bytes, base64, over the buffer built by
    /// [`crate::signing::pair_proof_message`]. Proves the device holds the private half of
    /// the key it is registering — without it, anyone who read the QR could register a
    /// public key they do not control.
    pub proof: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairResponse {
    pub device_id: String,
    pub host_id: String,
    pub host_name: String,
}

/// `pair.begin` — admin only. Opens a pairing window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PairBeginRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ttl_seconds: Option<u32>,
}

impl PairBeginRequest {
    /// The window length this request asks for, applying the default when omitted.
    ///
    /// Call [`Self::validate`] first — this does not clamp, because silently clamping an
    /// out-of-range value would hide a client bug.
    pub fn ttl_or_default(&self) -> u32 {
        self.ttl_seconds.unwrap_or(DEFAULT_TTL_SECONDS)
    }

    pub fn validate(&self) -> Result<(), ErrorPayload> {
        match self.ttl_seconds {
            Some(ttl) if !TTL_RANGE_SECONDS.contains(&ttl) => Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                format!(
                    "ttl_seconds {} is outside {}..={}",
                    ttl,
                    TTL_RANGE_SECONDS.start(),
                    TTL_RANGE_SECONDS.end()
                ),
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairBeginResponse {
    pub code: String,
    /// Unix timestamp, seconds.
    pub expires_at: i64,
    /// `muxdeck://pair?addr=&host=&fp=&code=`, parameters in exactly that order.
    pub qr_payload: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairListDevicesResponse {
    pub devices: Vec<DeviceInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceInfo {
    pub device_id: String,
    pub name: String,
    pub platform: Platform,
    /// Unix timestamp, seconds.
    pub paired_at: i64,
    /// Unix timestamp, seconds.
    pub last_seen: i64,
    pub connected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairRevokeRequest {
    pub device_id: String,
}

/// `evt pairing.state`, delivered to `admin` sockets only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PairingState {
    pub active: bool,
    /// Unix timestamp, seconds.
    pub expires_at: i64,
}
