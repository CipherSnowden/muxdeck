//! Input injection payloads and the canonical key table. `docs/PROTOCOL.md` §4.3 and §5.

use serde::{Deserialize, Serialize};

use crate::envelope::{ErrorCode, ErrorPayload};

/// `input.key_combo`.
///
/// Modifiers are pressed in listed order, the final non-modifier key is tapped, then all
/// are released in reverse order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyCombo {
    pub keys: Vec<Key>,
    /// Holds **the entire combo** — every key down — before releasing in reverse order.
    ///
    /// `Option` rather than a defaulted `u32` so the field round-trips exactly: a
    /// `key_sequence` step that omits it must not gain one on the way back out.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hold_ms: Option<u32>,
}

impl KeyCombo {
    pub fn hold_ms_or_default(&self) -> u32 {
        self.hold_ms.unwrap_or(0)
    }

    /// `docs/PROTOCOL.md` §4.3.
    ///
    /// Zero non-modifiers is valid — `["META"]` alone is a real macro. Two or more is
    /// almost always a mistake, and `input.key_sequence` exists for the deliberate case.
    pub fn validate(&self) -> Result<(), ErrorPayload> {
        if self.keys.is_empty() {
            return Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                "input.key_combo requires at least one key",
            ));
        }
        let non_modifiers = self.keys.iter().filter(|k| !k.is_modifier()).count();
        if non_modifiers > 1 {
            return Err(ErrorPayload::new(
                ErrorCode::BadRequest,
                "input.key_combo accepts at most one non-modifier key; use input.key_sequence",
            ));
        }
        Ok(())
    }
}

/// `input.key_sequence` — several combos in order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeySequence {
    pub steps: Vec<SequenceStep>,
}

impl KeySequence {
    pub fn validate(&self) -> Result<(), ErrorPayload> {
        for step in &self.steps {
            if let SequenceStep::Combo(combo) = step {
                combo.validate()?;
            }
        }
        Ok(())
    }
}

/// One step of a sequence: either a combo or a pause.
///
/// Untagged because the wire carries no discriminant here — unlike
/// [`crate::session::HelloResponse`], where a tag exists and must be used. The two shapes
/// are told apart by their required fields, which is safe only because `keys` and
/// `delay_ms` are both mandatory in their respective variants. Do not give either a serde
/// default, or an unrelated payload will start matching the wrong arm.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SequenceStep {
    Combo(KeyCombo),
    Delay { delay_ms: u32 },
}

/// `input.text` — type a literal string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextRequest {
    pub text: String,
    /// Pause between characters, milliseconds. `0` means as fast as the OS allows.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub delay_ms: Option<u32>,
}

impl TextRequest {
    pub fn delay_ms_or_default(&self) -> u32 {
        self.delay_ms.unwrap_or(0)
    }
}

/// `input.media`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaRequest {
    pub command: MediaCommand,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MediaCommand {
    PlayPause,
    Next,
    Prev,
    Stop,
    VolumeUp,
    VolumeDown,
    Mute,
}

/// `input.mouse`, internally tagged on `action`.
///
/// `MoveAbs` is normalised `0.0..1.0` across the **primary monitor**, origin top-left,
/// because the client has no idea what resolution the host runs. `Scroll` is in notches;
/// the engine converts per platform.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum MouseRequest {
    /// Physical pixels, relative to the current cursor position.
    MoveRel {
        dx: i32,
        dy: i32,
    },
    MoveAbs {
        x: f64,
        y: f64,
    },
    Click {
        button: MouseButton,
    },
    Down {
        button: MouseButton,
    },
    Up {
        button: MouseButton,
    },
    /// Notches; `1.0` is one detent.
    Scroll {
        dx: f64,
        dy: f64,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// A canonical key name. `docs/PROTOCOL.md` §5.
///
/// Uppercase, ASCII, no aliases. The engine maps these to platform scancodes; this enum
/// exists rather than a bare string so that mapping is exhaustive at compile time.
///
/// `META` is the Windows key on Windows and Linux, and Command on macOS. The engine does
/// **not** auto-swap `CONTROL`/`META` on macOS — profiles are per-host, so the user maps
/// what they want.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum Key {
    // Modifiers
    Control,
    Shift,
    Alt,
    Meta,

    // Letters
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    H,
    I,
    J,
    K,
    L,
    M,
    N,
    O,
    P,
    Q,
    R,
    S,
    T,
    U,
    V,
    W,
    X,
    Y,
    Z,

    // Digits
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,

    // Function
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,

    // Navigation
    Escape,
    Tab,
    #[serde(rename = "CAPSLOCK")]
    CapsLock,
    Space,
    Enter,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    #[serde(rename = "PAGEUP")]
    PageUp,
    #[serde(rename = "PAGEDOWN")]
    PageDown,
    Left,
    Right,
    Up,
    Down,

    // Numpad
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSub,
    NumpadMul,
    NumpadDiv,
    NumpadDecimal,
    NumpadEnter,

    // Symbols
    Minus,
    Equal,
    BracketLeft,
    BracketRight,
    Backslash,
    Semicolon,
    Quote,
    Backquote,
    Comma,
    Period,
    Slash,

    // System
    #[serde(rename = "PRINTSCREEN")]
    PrintScreen,
    #[serde(rename = "SCROLLLOCK")]
    ScrollLock,
    Pause,
    #[serde(rename = "NUMLOCK")]
    NumLock,
    Menu,
}

impl Key {
    /// `CONTROL`, `SHIFT`, `ALT` and `META` are the modifiers for the purposes of the
    /// combo rules in `docs/PROTOCOL.md` §4.3; everything else is a non-modifier.
    pub fn is_modifier(&self) -> bool {
        matches!(self, Key::Control | Key::Shift | Key::Alt | Key::Meta)
    }
}
