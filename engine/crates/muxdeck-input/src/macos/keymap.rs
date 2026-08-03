//! Canonical key names to macOS virtual key codes. `docs/PROTOCOL.md` §5.
//!
//! The values are the `kVK_*` constants from Carbon's `<HIToolbox/Events.h>`, hardcoded rather
//! than read from the framework. They are **stable ABI** — they describe physical key positions
//! on the original Apple Extended Keyboard and have not changed since Mac OS 8 — so writing them
//! out costs nothing in correctness and buys a table that compiles and is tested on any host,
//! including the Windows development machine. Only `backend.rs` is macOS-only.
//!
//! **These are positions, not characters.** `kVK_ANSI_A` is 0x00 because A sits at the left of
//! the home row on a US keyboard, not because A is the first letter. On a French layout the same
//! code produces Q. That is the same trade Windows makes with `wVk`, and it is what makes a
//! `CONTROL+A` shortcut land on the key the user's application expects.

use muxdeck_core::{Key, MediaCommand};

/// `kVK_*` from `<HIToolbox/Events.h>`.
///
/// Ordered by code rather than alphabetically, because the ordering *is* the keyboard: reading
/// down the list walks the physical rows, which is the only way to sanity-check a transcription.
mod vk {
    pub const ANSI_A: u16 = 0x00;
    pub const ANSI_S: u16 = 0x01;
    pub const ANSI_D: u16 = 0x02;
    pub const ANSI_F: u16 = 0x03;
    pub const ANSI_H: u16 = 0x04;
    pub const ANSI_G: u16 = 0x05;
    pub const ANSI_Z: u16 = 0x06;
    pub const ANSI_X: u16 = 0x07;
    pub const ANSI_C: u16 = 0x08;
    pub const ANSI_V: u16 = 0x09;
    pub const ANSI_B: u16 = 0x0B;
    pub const ANSI_Q: u16 = 0x0C;
    pub const ANSI_W: u16 = 0x0D;
    pub const ANSI_E: u16 = 0x0E;
    pub const ANSI_R: u16 = 0x0F;
    pub const ANSI_Y: u16 = 0x10;
    pub const ANSI_T: u16 = 0x11;
    pub const ANSI_1: u16 = 0x12;
    pub const ANSI_2: u16 = 0x13;
    pub const ANSI_3: u16 = 0x14;
    pub const ANSI_4: u16 = 0x15;
    pub const ANSI_6: u16 = 0x16;
    pub const ANSI_5: u16 = 0x17;
    pub const ANSI_EQUAL: u16 = 0x18;
    pub const ANSI_9: u16 = 0x19;
    pub const ANSI_7: u16 = 0x1A;
    pub const ANSI_MINUS: u16 = 0x1B;
    pub const ANSI_8: u16 = 0x1C;
    pub const ANSI_0: u16 = 0x1D;
    pub const ANSI_RIGHT_BRACKET: u16 = 0x1E;
    pub const ANSI_O: u16 = 0x1F;
    pub const ANSI_U: u16 = 0x20;
    pub const ANSI_LEFT_BRACKET: u16 = 0x21;
    pub const ANSI_I: u16 = 0x22;
    pub const ANSI_P: u16 = 0x23;
    pub const RETURN: u16 = 0x24;
    pub const ANSI_L: u16 = 0x25;
    pub const ANSI_J: u16 = 0x26;
    pub const ANSI_QUOTE: u16 = 0x27;
    pub const ANSI_K: u16 = 0x28;
    pub const ANSI_SEMICOLON: u16 = 0x29;
    pub const ANSI_BACKSLASH: u16 = 0x2A;
    pub const ANSI_COMMA: u16 = 0x2B;
    pub const ANSI_SLASH: u16 = 0x2C;
    pub const ANSI_N: u16 = 0x2D;
    pub const ANSI_M: u16 = 0x2E;
    pub const ANSI_PERIOD: u16 = 0x2F;
    pub const TAB: u16 = 0x30;
    pub const SPACE: u16 = 0x31;
    pub const ANSI_GRAVE: u16 = 0x32;
    /// The key labelled *delete* on a Mac keyboard, which is backspace everywhere else.
    pub const DELETE: u16 = 0x33;
    pub const ESCAPE: u16 = 0x35;
    pub const COMMAND: u16 = 0x37;
    pub const SHIFT: u16 = 0x38;
    pub const CAPS_LOCK: u16 = 0x39;
    pub const OPTION: u16 = 0x3A;
    pub const CONTROL: u16 = 0x3B;
    pub const F17: u16 = 0x40;
    pub const ANSI_KEYPAD_DECIMAL: u16 = 0x41;
    pub const ANSI_KEYPAD_MULTIPLY: u16 = 0x43;
    pub const ANSI_KEYPAD_PLUS: u16 = 0x45;
    /// *Clear* on a Mac keyboard. It occupies the position a PC keyboard gives to Num Lock, and
    /// macOS reports a PC Num Lock press as this code.
    pub const ANSI_KEYPAD_CLEAR: u16 = 0x47;
    pub const ANSI_KEYPAD_DIVIDE: u16 = 0x4B;
    pub const ANSI_KEYPAD_ENTER: u16 = 0x4C;
    pub const ANSI_KEYPAD_MINUS: u16 = 0x4E;
    pub const F18: u16 = 0x4F;
    pub const F19: u16 = 0x50;
    pub const ANSI_KEYPAD_0: u16 = 0x52;
    pub const ANSI_KEYPAD_1: u16 = 0x53;
    pub const ANSI_KEYPAD_2: u16 = 0x54;
    pub const ANSI_KEYPAD_3: u16 = 0x55;
    pub const ANSI_KEYPAD_4: u16 = 0x56;
    pub const ANSI_KEYPAD_5: u16 = 0x57;
    pub const ANSI_KEYPAD_6: u16 = 0x58;
    pub const ANSI_KEYPAD_7: u16 = 0x59;
    pub const F20: u16 = 0x5A;
    pub const ANSI_KEYPAD_8: u16 = 0x5B;
    pub const ANSI_KEYPAD_9: u16 = 0x5C;
    pub const F5: u16 = 0x60;
    pub const F6: u16 = 0x61;
    pub const F7: u16 = 0x62;
    pub const F3: u16 = 0x63;
    pub const F8: u16 = 0x64;
    pub const F9: u16 = 0x65;
    pub const F11: u16 = 0x67;
    pub const F13: u16 = 0x69;
    pub const F16: u16 = 0x6A;
    pub const F14: u16 = 0x6B;
    pub const F10: u16 = 0x6D;
    pub const F12: u16 = 0x6F;
    pub const F15: u16 = 0x71;
    /// *Help* on a Mac keyboard, which is where a PC keyboard puts Insert.
    pub const HELP: u16 = 0x72;
    pub const HOME: u16 = 0x73;
    pub const PAGE_UP: u16 = 0x74;
    /// The forward-delete key — what every other platform simply calls *delete*.
    pub const FORWARD_DELETE: u16 = 0x75;
    pub const F4: u16 = 0x76;
    pub const END: u16 = 0x77;
    pub const F2: u16 = 0x78;
    pub const PAGE_DOWN: u16 = 0x79;
    pub const F1: u16 = 0x7A;
    pub const LEFT_ARROW: u16 = 0x7B;
    pub const RIGHT_ARROW: u16 = 0x7C;
    pub const DOWN_ARROW: u16 = 0x7D;
    pub const UP_ARROW: u16 = 0x7E;
}

/// The virtual key code for a canonical key name, or `None` if macOS has no such key.
///
/// Unlike the Windows and Linux tables this is partial, and deliberately so: macOS keyboards
/// have never had Print Screen, Scroll Lock, Pause, a Menu key, or F21–F24, and there is no
/// virtual key code that means any of them. Sending an approximation would fire whatever *is* at
/// that code — mapping Print Screen to F13 because that is where a PC keyboard puts it would
/// trigger an application's F13 binding, not take a screenshot. Returning `None` lets the
/// backend refuse with a message the user can read instead.
pub fn key_code(key: Key) -> Option<u16> {
    use Key as K;

    let code = match key {
        // The protocol has one CONTROL, one SHIFT, one ALT and one META; each maps to the left
        // physical key. Note ALT is Option and META is Command — the deck's META button is the
        // one macOS users reach for in almost every shortcut.
        K::Control => vk::CONTROL,
        K::Shift => vk::SHIFT,
        K::Alt => vk::OPTION,
        K::Meta => vk::COMMAND,

        // Letters are scattered: A is 0x00 and B is 0x0B, because the codes follow the physical
        // key positions of the Apple Extended Keyboard. Enumerated for exactly that reason.
        K::A => vk::ANSI_A,
        K::B => vk::ANSI_B,
        K::C => vk::ANSI_C,
        K::D => vk::ANSI_D,
        K::E => vk::ANSI_E,
        K::F => vk::ANSI_F,
        K::G => vk::ANSI_G,
        K::H => vk::ANSI_H,
        K::I => vk::ANSI_I,
        K::J => vk::ANSI_J,
        K::K => vk::ANSI_K,
        K::L => vk::ANSI_L,
        K::M => vk::ANSI_M,
        K::N => vk::ANSI_N,
        K::O => vk::ANSI_O,
        K::P => vk::ANSI_P,
        K::Q => vk::ANSI_Q,
        K::R => vk::ANSI_R,
        K::S => vk::ANSI_S,
        K::T => vk::ANSI_T,
        K::U => vk::ANSI_U,
        K::V => vk::ANSI_V,
        K::W => vk::ANSI_W,
        K::X => vk::ANSI_X,
        K::Y => vk::ANSI_Y,
        K::Z => vk::ANSI_Z,

        // Digits are not in numeric order either — 5 and 6 are swapped relative to what an
        // offset calculation would give, which is the classic transcription bug here.
        K::Digit0 => vk::ANSI_0,
        K::Digit1 => vk::ANSI_1,
        K::Digit2 => vk::ANSI_2,
        K::Digit3 => vk::ANSI_3,
        K::Digit4 => vk::ANSI_4,
        K::Digit5 => vk::ANSI_5,
        K::Digit6 => vk::ANSI_6,
        K::Digit7 => vk::ANSI_7,
        K::Digit8 => vk::ANSI_8,
        K::Digit9 => vk::ANSI_9,

        // The function keys are in no order whatsoever: F1 is 0x7A, F2 is 0x78, F13 is 0x69.
        K::F1 => vk::F1,
        K::F2 => vk::F2,
        K::F3 => vk::F3,
        K::F4 => vk::F4,
        K::F5 => vk::F5,
        K::F6 => vk::F6,
        K::F7 => vk::F7,
        K::F8 => vk::F8,
        K::F9 => vk::F9,
        K::F10 => vk::F10,
        K::F11 => vk::F11,
        K::F12 => vk::F12,
        K::F13 => vk::F13,
        K::F14 => vk::F14,
        K::F15 => vk::F15,
        K::F16 => vk::F16,
        K::F17 => vk::F17,
        K::F18 => vk::F18,
        K::F19 => vk::F19,
        K::F20 => vk::F20,
        // Carbon stops at F20. No Apple keyboard has ever had more.
        K::F21 | K::F22 | K::F23 | K::F24 => return None,

        K::Escape => vk::ESCAPE,
        K::Tab => vk::TAB,
        K::CapsLock => vk::CAPS_LOCK,
        K::Space => vk::SPACE,
        K::Enter => vk::RETURN,
        // The naming here is genuinely inverted relative to every other platform, and getting it
        // backwards means BACKSPACE deletes forwards.
        K::Backspace => vk::DELETE,
        K::Delete => vk::FORWARD_DELETE,
        K::Insert => vk::HELP,
        K::Home => vk::HOME,
        K::End => vk::END,
        K::PageUp => vk::PAGE_UP,
        K::PageDown => vk::PAGE_DOWN,
        K::Left => vk::LEFT_ARROW,
        K::Right => vk::RIGHT_ARROW,
        K::Up => vk::UP_ARROW,
        K::Down => vk::DOWN_ARROW,

        K::Numpad0 => vk::ANSI_KEYPAD_0,
        K::Numpad1 => vk::ANSI_KEYPAD_1,
        K::Numpad2 => vk::ANSI_KEYPAD_2,
        K::Numpad3 => vk::ANSI_KEYPAD_3,
        K::Numpad4 => vk::ANSI_KEYPAD_4,
        K::Numpad5 => vk::ANSI_KEYPAD_5,
        K::Numpad6 => vk::ANSI_KEYPAD_6,
        K::Numpad7 => vk::ANSI_KEYPAD_7,
        K::Numpad8 => vk::ANSI_KEYPAD_8,
        K::Numpad9 => vk::ANSI_KEYPAD_9,
        K::NumpadAdd => vk::ANSI_KEYPAD_PLUS,
        K::NumpadSub => vk::ANSI_KEYPAD_MINUS,
        K::NumpadMul => vk::ANSI_KEYPAD_MULTIPLY,
        K::NumpadDiv => vk::ANSI_KEYPAD_DIVIDE,
        K::NumpadDecimal => vk::ANSI_KEYPAD_DECIMAL,
        // Like Linux and unlike Windows, the keypad Enter has its own code rather than being an
        // extended-flag variant of Return.
        K::NumpadEnter => vk::ANSI_KEYPAD_ENTER,
        // Clear sits where a PC keyboard puts Num Lock, and macOS reports that key as Clear.
        K::NumLock => vk::ANSI_KEYPAD_CLEAR,

        K::Minus => vk::ANSI_MINUS,
        K::Equal => vk::ANSI_EQUAL,
        K::BracketLeft => vk::ANSI_LEFT_BRACKET,
        K::BracketRight => vk::ANSI_RIGHT_BRACKET,
        K::Backslash => vk::ANSI_BACKSLASH,
        K::Semicolon => vk::ANSI_SEMICOLON,
        K::Quote => vk::ANSI_QUOTE,
        K::Backquote => vk::ANSI_GRAVE,
        K::Comma => vk::ANSI_COMMA,
        K::Period => vk::ANSI_PERIOD,
        K::Slash => vk::ANSI_SLASH,

        // No Mac keyboard has ever had these, and no virtual key code means them.
        K::PrintScreen | K::ScrollLock | K::Pause | K::Menu => return None,
    };

    Some(code)
}

/// Whether a key is a modifier, and which `CGEventFlags` bit it sets.
///
/// macOS wants modifiers expressed as *flags on the key event* rather than as separate key
/// presses (`docs/ENGINE.md` §4.2), so the backend has to tell the two apart before it can build
/// anything. Values are `kCGEventFlagMask*` from `<CoreGraphics/CGEventTypes.h>`.
pub fn modifier_flag(key: Key) -> Option<u64> {
    match key {
        Key::Shift => Some(1 << 17),
        Key::Control => Some(1 << 18),
        Key::Alt => Some(1 << 19),
        Key::Meta => Some(1 << 20),
        _ => None,
    }
}

/// The `NX_KEYTYPE_*` code for a media command, or `None` if macOS has no such key.
///
/// Values are from IOKit's `<IOKit/hidsystem/ev_keymap.h>`. These are a **separate namespace**
/// from the virtual key codes above — they travel in an `NSSystemDefined` event, not a keyboard
/// event, which is why the backend has a whole second path for them.
pub fn media_code(command: MediaCommand) -> Option<u8> {
    let code = match command {
        MediaCommand::VolumeUp => 0,   // NX_KEYTYPE_SOUND_UP
        MediaCommand::VolumeDown => 1, // NX_KEYTYPE_SOUND_DOWN
        MediaCommand::Mute => 7,       // NX_KEYTYPE_MUTE
        MediaCommand::PlayPause => 16, // NX_KEYTYPE_PLAY
        MediaCommand::Next => 17,      // NX_KEYTYPE_NEXT
        MediaCommand::Prev => 18,      // NX_KEYTYPE_PREVIOUS
        // There is no NX_KEYTYPE_STOP. macOS never shipped a stop key: play/pause is the only
        // transport control the hardware has, and no application listens for a stop that cannot
        // be pressed.
        MediaCommand::Stop => return None,
    };
    Some(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The keys macOS genuinely cannot express, listed once so a change to the table has to
    /// change this list too rather than silently growing the hole.
    const UNMAPPED: &[Key] = &[
        Key::F21,
        Key::F22,
        Key::F23,
        Key::F24,
        Key::PrintScreen,
        Key::ScrollLock,
        Key::Pause,
        Key::Menu,
    ];

    #[test]
    fn every_key_either_maps_or_is_a_known_gap() {
        for key in Key::ALL {
            let mapped = key_code(*key).is_some();
            let expected = !UNMAPPED.contains(key);
            assert_eq!(
                mapped, expected,
                "{key:?} is on the wrong side of the gap list"
            );
        }
    }

    #[test]
    fn every_mapped_key_is_a_real_virtual_key_code() {
        // Virtual key codes are 7-bit: the hardware only ever had 128 positions.
        for key in Key::ALL {
            if let Some(code) = key_code(*key) {
                assert!(
                    code < 0x80,
                    "{key:?} maps to {code:#x}, outside the 7-bit range"
                );
            }
        }
    }

    #[test]
    fn no_two_keys_share_a_code() {
        let mut seen = std::collections::HashMap::new();
        for key in Key::ALL {
            let Some(code) = key_code(*key) else { continue };
            if let Some(previous) = seen.insert(code, key) {
                panic!("{key:?} and {previous:?} both map to {code:#x}");
            }
        }
    }

    #[test]
    fn backspace_and_delete_are_not_swapped() {
        // Apple's `kVK_Delete` is the backspace key and `kVK_ForwardDelete` is the delete key.
        // Taking the names at face value makes BACKSPACE delete forwards, which is the single
        // easiest mistake to make in this table.
        assert_eq!(key_code(Key::Backspace), Some(0x33));
        assert_eq!(key_code(Key::Delete), Some(0x75));
    }

    #[test]
    fn letters_and_digits_follow_key_positions_not_order() {
        // The trap: assuming these runs are contiguous, as the Windows virtual keys are.
        assert_eq!(key_code(Key::A), Some(0x00));
        assert_eq!(key_code(Key::B), Some(0x0B));
        assert_ne!(key_code(Key::B), key_code(Key::A).map(|c| c + 1));
        // 5 and 6 are the pair an offset calculation gets backwards.
        assert_eq!(key_code(Key::Digit5), Some(0x17));
        assert_eq!(key_code(Key::Digit6), Some(0x16));
    }

    #[test]
    fn function_keys_are_in_no_order_at_all() {
        assert_eq!(key_code(Key::F1), Some(0x7A));
        assert_eq!(key_code(Key::F2), Some(0x78));
        assert_eq!(key_code(Key::F13), Some(0x69));
        assert_eq!(key_code(Key::F20), Some(0x5A));
    }

    #[test]
    fn exactly_the_four_protocol_modifiers_carry_a_flag() {
        for key in Key::ALL {
            assert_eq!(
                modifier_flag(*key).is_some(),
                key.is_modifier(),
                "{key:?} is on the wrong side of the modifier split"
            );
        }
    }

    #[test]
    fn modifier_flags_do_not_overlap() {
        let mut combined = 0u64;
        for key in [Key::Control, Key::Shift, Key::Alt, Key::Meta] {
            let flag = modifier_flag(key).expect("modifier");
            assert_eq!(combined & flag, 0, "{key:?} reuses a bit");
            combined |= flag;
        }
    }

    #[test]
    fn every_media_command_maps_except_stop() {
        for command in MediaCommand::ALL {
            let mapped = media_code(*command);
            if matches!(command, MediaCommand::Stop) {
                assert!(mapped.is_none(), "macOS has no stop key");
            } else {
                assert!(mapped.is_some(), "{command:?} must map");
            }
        }
    }
}
