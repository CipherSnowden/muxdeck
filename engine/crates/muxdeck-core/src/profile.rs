//! Deck layouts. `docs/PROTOCOL.md` §4.5 and §6.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::envelope::Op;

/// One deck layout: a grid of pages of buttons.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub grid: Grid,
    pub pages: Vec<Page>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grid {
    pub cols: u16,
    pub rows: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Page {
    pub id: String,
    pub name: String,
    /// Buttons are sparse: a grid cell with no button is empty.
    pub buttons: Vec<Button>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Button {
    pub id: String,
    pub pos: Position,
    pub label: String,
    /// A name from the curated icon map shipped in `packages/muxdeck_protocol`. Unknown
    /// names fall back to a filled dot rather than rendering blank.
    pub icon: String,
    /// `#RRGGBB`.
    pub color: String,
    pub haptic: Haptic,
    /// Serialised even when `null`, because the wire carries an explicit `null` here and
    /// dropping the key would change the message.
    pub on_tap: Option<ButtonAction>,
    pub on_long_press: Option<ButtonAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Position {
    pub col: u16,
    pub row: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Haptic {
    None,
    Light,
    Medium,
    Heavy,
}

/// An embedded `{ op, d }` pair.
///
/// `d` stays a raw [`Value`] on purpose: the op it belongs to is only known at dispatch
/// time, and the engine re-checks both the op's permissibility and the payload's shape
/// when the button is pressed rather than trusting what was stored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ButtonAction {
    pub op: Op,
    pub d: Value,
}

/// The payload of `profile.get`'s response, `profile.set`'s request and the
/// `profile.changed` event — one wrapper type for all three, which is the reason the
/// Profile is wrapped rather than being the payload itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfileWrapper {
    pub profile: Profile,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileGetRequest {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileListResponse {
    pub profiles: Vec<ProfileSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileSummary {
    pub id: String,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileActivateRequest {
    pub profile_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileDeleteRequest {
    pub profile_id: String,
}
