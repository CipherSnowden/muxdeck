//! Canonical key names to Win32 virtual-key codes. `docs/PROTOCOL.md` §5.
//!
//! Re-derived against the canonical key list rather than copied from the pre-rewrite Go
//! implementation: that table predates the canonical names and disagrees with them in a dozen
//! places (`SUPER` vs `META`, `PAGE_UP` vs `PAGEUP`, `GRAVE` vs `BACKQUOTE`, separate
//! `MEDIA_*` keys where the protocol now has `input.media`). The virtual-key *values* are the
//! part worth keeping, and they are transcribed from `winuser.h`.
//!
//! This module is compiled on every platform so the table can be tested anywhere; only the
//! `windows` module actually uses it.

use muxdeck_core::{Key, MediaCommand};

/// Win32 virtual-key codes, transcribed from `winuser.h`.
mod vk {
    pub const BACK: u16 = 0x08;
    pub const TAB: u16 = 0x09;
    pub const RETURN: u16 = 0x0D;
    pub const PAUSE: u16 = 0x13;
    pub const CAPITAL: u16 = 0x14;
    pub const ESCAPE: u16 = 0x1B;
    pub const SPACE: u16 = 0x20;
    pub const PRIOR: u16 = 0x21;
    pub const NEXT: u16 = 0x22;
    pub const END: u16 = 0x23;
    pub const HOME: u16 = 0x24;
    pub const LEFT: u16 = 0x25;
    pub const UP: u16 = 0x26;
    pub const RIGHT: u16 = 0x27;
    pub const DOWN: u16 = 0x28;
    pub const SNAPSHOT: u16 = 0x2C;
    pub const INSERT: u16 = 0x2D;
    pub const DELETE: u16 = 0x2E;

    pub const LWIN: u16 = 0x5B;
    pub const APPS: u16 = 0x5D;

    pub const NUMPAD0: u16 = 0x60;
    pub const MULTIPLY: u16 = 0x6A;
    pub const ADD: u16 = 0x6B;
    pub const SUBTRACT: u16 = 0x6D;
    pub const DECIMAL: u16 = 0x6E;
    pub const DIVIDE: u16 = 0x6F;

    pub const F1: u16 = 0x70;

    pub const NUMLOCK: u16 = 0x90;
    pub const SCROLL: u16 = 0x91;

    pub const LSHIFT: u16 = 0xA0;
    pub const LCONTROL: u16 = 0xA2;
    pub const LMENU: u16 = 0xA4;

    pub const VOLUME_MUTE: u16 = 0xAD;
    pub const VOLUME_DOWN: u16 = 0xAE;
    pub const VOLUME_UP: u16 = 0xAF;
    pub const MEDIA_NEXT_TRACK: u16 = 0xB0;
    pub const MEDIA_PREV_TRACK: u16 = 0xB1;
    pub const MEDIA_STOP: u16 = 0xB2;
    pub const MEDIA_PLAY_PAUSE: u16 = 0xB3;

    pub const OEM_1: u16 = 0xBA; // ;:
    pub const OEM_PLUS: u16 = 0xBB;
    pub const OEM_COMMA: u16 = 0xBC;
    pub const OEM_MINUS: u16 = 0xBD;
    pub const OEM_PERIOD: u16 = 0xBE;
    pub const OEM_2: u16 = 0xBF; // /?
    pub const OEM_3: u16 = 0xC0; // `~
    pub const OEM_4: u16 = 0xDB; // [{
    pub const OEM_5: u16 = 0xDC; // \|
    pub const OEM_6: u16 = 0xDD; // ]}
    pub const OEM_7: u16 = 0xDE; // '"
}

/// The virtual-key code for a canonical key name.
///
/// Total: every canonical key maps to something, checked exhaustively by the compiler. There
/// is no "unsupported key" path on Windows.
pub fn virtual_key(key: Key) -> u16 {
    use Key as K;

    match key {
        // Modifiers map to the *left* variant. The protocol has one `CONTROL`, not two, and a
        // macro deck has no reason to distinguish them — applications that care read the
        // scancode, which the backend fills in.
        K::Control => vk::LCONTROL,
        K::Shift => vk::LSHIFT,
        K::Alt => vk::LMENU,
        K::Meta => vk::LWIN,

        // Letters and digits are their ASCII values, which is a Win32 guarantee.
        K::A => b'A' as u16,
        K::B => b'B' as u16,
        K::C => b'C' as u16,
        K::D => b'D' as u16,
        K::E => b'E' as u16,
        K::F => b'F' as u16,
        K::G => b'G' as u16,
        K::H => b'H' as u16,
        K::I => b'I' as u16,
        K::J => b'J' as u16,
        K::K => b'K' as u16,
        K::L => b'L' as u16,
        K::M => b'M' as u16,
        K::N => b'N' as u16,
        K::O => b'O' as u16,
        K::P => b'P' as u16,
        K::Q => b'Q' as u16,
        K::R => b'R' as u16,
        K::S => b'S' as u16,
        K::T => b'T' as u16,
        K::U => b'U' as u16,
        K::V => b'V' as u16,
        K::W => b'W' as u16,
        K::X => b'X' as u16,
        K::Y => b'Y' as u16,
        K::Z => b'Z' as u16,

        K::Digit0 => b'0' as u16,
        K::Digit1 => b'1' as u16,
        K::Digit2 => b'2' as u16,
        K::Digit3 => b'3' as u16,
        K::Digit4 => b'4' as u16,
        K::Digit5 => b'5' as u16,
        K::Digit6 => b'6' as u16,
        K::Digit7 => b'7' as u16,
        K::Digit8 => b'8' as u16,
        K::Digit9 => b'9' as u16,

        // VK_F1..VK_F24 are contiguous.
        K::F1 => vk::F1,
        K::F2 => vk::F1 + 1,
        K::F3 => vk::F1 + 2,
        K::F4 => vk::F1 + 3,
        K::F5 => vk::F1 + 4,
        K::F6 => vk::F1 + 5,
        K::F7 => vk::F1 + 6,
        K::F8 => vk::F1 + 7,
        K::F9 => vk::F1 + 8,
        K::F10 => vk::F1 + 9,
        K::F11 => vk::F1 + 10,
        K::F12 => vk::F1 + 11,
        K::F13 => vk::F1 + 12,
        K::F14 => vk::F1 + 13,
        K::F15 => vk::F1 + 14,
        K::F16 => vk::F1 + 15,
        K::F17 => vk::F1 + 16,
        K::F18 => vk::F1 + 17,
        K::F19 => vk::F1 + 18,
        K::F20 => vk::F1 + 19,
        K::F21 => vk::F1 + 20,
        K::F22 => vk::F1 + 21,
        K::F23 => vk::F1 + 22,
        K::F24 => vk::F1 + 23,

        K::Escape => vk::ESCAPE,
        K::Tab => vk::TAB,
        K::CapsLock => vk::CAPITAL,
        K::Space => vk::SPACE,
        K::Enter => vk::RETURN,
        K::Backspace => vk::BACK,
        K::Delete => vk::DELETE,
        K::Insert => vk::INSERT,
        K::Home => vk::HOME,
        K::End => vk::END,
        K::PageUp => vk::PRIOR,
        K::PageDown => vk::NEXT,
        K::Left => vk::LEFT,
        K::Right => vk::RIGHT,
        K::Up => vk::UP,
        K::Down => vk::DOWN,

        // VK_NUMPAD0..VK_NUMPAD9 are contiguous.
        K::Numpad0 => vk::NUMPAD0,
        K::Numpad1 => vk::NUMPAD0 + 1,
        K::Numpad2 => vk::NUMPAD0 + 2,
        K::Numpad3 => vk::NUMPAD0 + 3,
        K::Numpad4 => vk::NUMPAD0 + 4,
        K::Numpad5 => vk::NUMPAD0 + 5,
        K::Numpad6 => vk::NUMPAD0 + 6,
        K::Numpad7 => vk::NUMPAD0 + 7,
        K::Numpad8 => vk::NUMPAD0 + 8,
        K::Numpad9 => vk::NUMPAD0 + 9,
        K::NumpadAdd => vk::ADD,
        K::NumpadSub => vk::SUBTRACT,
        K::NumpadMul => vk::MULTIPLY,
        K::NumpadDiv => vk::DIVIDE,
        K::NumpadDecimal => vk::DECIMAL,
        // The numpad Enter shares VK_RETURN and is told apart only by the extended flag.
        K::NumpadEnter => vk::RETURN,

        K::Minus => vk::OEM_MINUS,
        K::Equal => vk::OEM_PLUS,
        K::BracketLeft => vk::OEM_4,
        K::BracketRight => vk::OEM_6,
        K::Backslash => vk::OEM_5,
        K::Semicolon => vk::OEM_1,
        K::Quote => vk::OEM_7,
        K::Backquote => vk::OEM_3,
        K::Comma => vk::OEM_COMMA,
        K::Period => vk::OEM_PERIOD,
        K::Slash => vk::OEM_2,

        K::PrintScreen => vk::SNAPSHOT,
        K::ScrollLock => vk::SCROLL,
        K::Pause => vk::PAUSE,
        K::NumLock => vk::NUMLOCK,
        K::Menu => vk::APPS,
    }
}

/// Keys that live on the extended scan-code page and need `KEYEVENTF_EXTENDEDKEY`.
///
/// **This flag is not cosmetic.** Without it the OS delivers the numpad twin instead — Delete
/// arrives as numpad `.`, Home as numpad `7`, and the arrows as their numpad digits. It is the
/// single most common way a Windows key table looks right and behaves wrong.
pub fn is_extended(key: Key) -> bool {
    use Key as K;

    matches!(
        key,
        K::Insert
            | K::Delete
            | K::Home
            | K::End
            | K::PageUp
            | K::PageDown
            | K::Left
            | K::Right
            | K::Up
            | K::Down
            | K::NumLock
            | K::PrintScreen
            | K::NumpadDiv
            | K::NumpadEnter
            | K::Menu
    )
}

/// The virtual-key code for a media command.
///
/// Media keys are **virtual-key only** — there is no meaningful scancode for them, so the
/// backend must not try to send one.
pub fn media_key(command: MediaCommand) -> u16 {
    match command {
        MediaCommand::PlayPause => vk::MEDIA_PLAY_PAUSE,
        MediaCommand::Next => vk::MEDIA_NEXT_TRACK,
        MediaCommand::Prev => vk::MEDIA_PREV_TRACK,
        MediaCommand::Stop => vk::MEDIA_STOP,
        MediaCommand::VolumeUp => vk::VOLUME_UP,
        MediaCommand::VolumeDown => vk::VOLUME_DOWN,
        MediaCommand::Mute => vk::VOLUME_MUTE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every canonical key, so the table can be swept exhaustively.
    const ALL_KEYS: &[Key] = &[
        Key::Control,
        Key::Shift,
        Key::Alt,
        Key::Meta,
        Key::A,
        Key::B,
        Key::C,
        Key::D,
        Key::E,
        Key::F,
        Key::G,
        Key::H,
        Key::I,
        Key::J,
        Key::K,
        Key::L,
        Key::M,
        Key::N,
        Key::O,
        Key::P,
        Key::Q,
        Key::R,
        Key::S,
        Key::T,
        Key::U,
        Key::V,
        Key::W,
        Key::X,
        Key::Y,
        Key::Z,
        Key::Digit0,
        Key::Digit1,
        Key::Digit2,
        Key::Digit3,
        Key::Digit4,
        Key::Digit5,
        Key::Digit6,
        Key::Digit7,
        Key::Digit8,
        Key::Digit9,
        Key::F1,
        Key::F2,
        Key::F3,
        Key::F4,
        Key::F5,
        Key::F6,
        Key::F7,
        Key::F8,
        Key::F9,
        Key::F10,
        Key::F11,
        Key::F12,
        Key::F13,
        Key::F14,
        Key::F15,
        Key::F16,
        Key::F17,
        Key::F18,
        Key::F19,
        Key::F20,
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
        Key::Escape,
        Key::Tab,
        Key::CapsLock,
        Key::Space,
        Key::Enter,
        Key::Backspace,
        Key::Delete,
        Key::Insert,
        Key::Home,
        Key::End,
        Key::PageUp,
        Key::PageDown,
        Key::Left,
        Key::Right,
        Key::Up,
        Key::Down,
        Key::Numpad0,
        Key::Numpad1,
        Key::Numpad2,
        Key::Numpad3,
        Key::Numpad4,
        Key::Numpad5,
        Key::Numpad6,
        Key::Numpad7,
        Key::Numpad8,
        Key::Numpad9,
        Key::NumpadAdd,
        Key::NumpadSub,
        Key::NumpadMul,
        Key::NumpadDiv,
        Key::NumpadDecimal,
        Key::NumpadEnter,
        Key::Minus,
        Key::Equal,
        Key::BracketLeft,
        Key::BracketRight,
        Key::Backslash,
        Key::Semicolon,
        Key::Quote,
        Key::Backquote,
        Key::Comma,
        Key::Period,
        Key::Slash,
        Key::PrintScreen,
        Key::ScrollLock,
        Key::Pause,
        Key::NumLock,
        Key::Menu,
    ];

    #[test]
    fn every_key_maps_to_a_real_virtual_key() {
        for key in ALL_KEYS {
            let vk = virtual_key(*key);
            assert!(vk > 0, "{key:?} maps to 0, which is not a virtual key");
            assert!(vk <= 0xFF, "{key:?} maps to {vk:#x}, outside the VK range");
        }
    }

    #[test]
    fn letters_and_digits_are_their_ascii_values() {
        assert_eq!(virtual_key(Key::A), 0x41);
        assert_eq!(virtual_key(Key::Z), 0x5A);
        assert_eq!(virtual_key(Key::Digit0), 0x30);
        assert_eq!(virtual_key(Key::Digit9), 0x39);
    }

    #[test]
    fn contiguous_ranges_line_up() {
        assert_eq!(virtual_key(Key::F1), 0x70);
        assert_eq!(virtual_key(Key::F12), 0x7B);
        assert_eq!(virtual_key(Key::F24), 0x87);
        assert_eq!(virtual_key(Key::Numpad0), 0x60);
        assert_eq!(virtual_key(Key::Numpad9), 0x69);
    }

    #[test]
    fn only_enter_and_numpad_enter_share_a_virtual_key() {
        // Sharing is legitimate for exactly one pair, told apart by the extended flag. Any
        // other collision means two canonical names were mapped to the same physical key by
        // mistake, which a sweep like this is the only way to notice.
        let mut collisions: Vec<(Key, Key)> = Vec::new();
        for (i, a) in ALL_KEYS.iter().enumerate() {
            for b in &ALL_KEYS[i + 1..] {
                if virtual_key(*a) == virtual_key(*b) {
                    collisions.push((*a, *b));
                }
            }
        }
        assert_eq!(
            collisions,
            vec![(Key::Enter, Key::NumpadEnter)],
            "unexpected virtual-key collisions"
        );
        assert!(!is_extended(Key::Enter));
        assert!(is_extended(Key::NumpadEnter));
    }

    #[test]
    fn the_keys_that_would_otherwise_arrive_as_numpad_twins_are_extended() {
        // Regression guard for the classic Windows bug: without the extended flag, Delete
        // arrives as numpad ".", Home as numpad "7", and the arrows as their digits.
        for key in [
            Key::Delete,
            Key::Insert,
            Key::Home,
            Key::End,
            Key::PageUp,
            Key::PageDown,
            Key::Up,
            Key::Down,
            Key::Left,
            Key::Right,
        ] {
            assert!(is_extended(key), "{key:?} must carry KEYEVENTF_EXTENDEDKEY");
        }
    }

    #[test]
    fn ordinary_keys_are_not_extended() {
        for key in [
            Key::A,
            Key::Space,
            Key::F1,
            Key::Numpad0,
            Key::Shift,
            Key::Comma,
        ] {
            assert!(!is_extended(key), "{key:?} must not be flagged extended");
        }
    }

    #[test]
    fn every_media_command_maps_into_the_documented_range() {
        for command in [
            MediaCommand::PlayPause,
            MediaCommand::Next,
            MediaCommand::Prev,
            MediaCommand::Stop,
            MediaCommand::VolumeUp,
            MediaCommand::VolumeDown,
            MediaCommand::Mute,
        ] {
            let vk = media_key(command);
            assert!(
                (0xAD..=0xB3).contains(&vk),
                "{command:?} maps to {vk:#x}, outside the media/volume VK block"
            );
        }
    }
}
