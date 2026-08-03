//! Session handshake payloads. `docs/PROTOCOL.md` §4.1.

use serde::{Deserialize, Serialize};

use crate::envelope::{ErrorCode, ErrorPayload};

/// `session.hello` request. Two mutually exclusive forms.
///
/// Exactly one of `device_id` (a paired deck) and `admin_token` (the local control panel)
/// must be present. Both, or neither, is [`ErrorCode::BadRequest`] — see [`Self::validate`].
/// The absent one is omitted from the wire rather than sent as `null`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HelloRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub device_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub admin_token: Option<String>,
    pub client_version: String,
    pub platform: Platform,
}

impl HelloRequest {
    pub fn validate(&self) -> Result<(), ErrorPayload> {
        match (&self.device_id, &self.admin_token) {
            (Some(_), None) | (None, Some(_)) => Ok(()),
            (Some(_), Some(_)) => Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                "session.hello carries both device_id and admin_token; exactly one is required",
            )),
            (None, None) => Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                "session.hello carries neither device_id nor admin_token; exactly one is required",
            )),
        }
    }
}

/// `session.hello` response: an **internally tagged** union on `mode`.
///
/// The tag is always present and is the only thing a reader needs to pick a branch. This
/// is deliberately not `#[serde(untagged)]`: untagged would silently fall over to the
/// wrong variant when a field is added or renamed, which is the exact failure the tag
/// exists to prevent. The variants must also stay newtype-wrapped around named structs —
/// serde's internal tagging works on newtype and struct variants only, and [`Ready`] has
/// to remain standalone because `session.auth` returns it untagged.
///
/// See `docs/ENGINE.md` §2.1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum HelloResponse {
    Challenge(Challenge),
    Ready(Ready),
}

/// The `mode: "challenge"` branch — answer to the deck form.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Challenge {
    /// 32 random bytes, base64. See `docs/PROTOCOL.md` §2.
    pub nonce: String,
    pub host_id: String,
    pub host_name: String,
}

/// The `mode: "ready"` branch, and also the `session.auth` response.
///
/// **One type, two places.** The `mode` tag is supplied by [`HelloResponse`] on the way out
/// and consumed by it on the way in; serialising `Ready` on its own produces the untagged
/// form `session.auth` requires. `Ready` has no `mode` field of its own and never did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ready {
    pub role: Role,
    pub protocol: u8,
    pub engine_version: String,
    pub host_platform: HostPlatform,
    pub active_profile_id: String,
    pub capabilities: Capabilities,
}

/// What this host can actually do right now, so a client can grey out buttons whose action
/// is unavailable instead of letting them fail at press time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capabilities {
    /// False when `input.text` cannot inject arbitrary Unicode — notably Linux/uinput.
    pub text_unicode: bool,
    /// False when the backend cannot emit media keys.
    pub media_keys: bool,
    /// False when the backend cannot emit mouse events.
    pub mouse: bool,
    /// False when shell execution is disabled.
    pub shell_actions: bool,
}

/// `session.auth` request. Valid only after a [`Challenge`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthRequest {
    /// Ed25519 signature, 64 bytes, base64. Over the buffer built by
    /// [`crate::signing::session_auth_message`].
    pub signature: String,
}

/// The platform a *client* runs on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Platform {
    Ios,
    Android,
    Windows,
    Macos,
    Linux,
}

/// The platform the *engine* runs on. Narrower than [`Platform`]: there is no mobile host.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HostPlatform {
    Windows,
    Macos,
    Linux,
}

/// `deck` for paired devices, `admin` for the local control panel.
///
/// `admin` cannot be requested — the engine grants it only for a loopback peer presenting
/// the local admin token. See `docs/ARCHITECTURE.md` §5.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Deck,
    Admin,
}
