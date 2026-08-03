//! Canonical key names to Linux input event codes. `docs/PROTOCOL.md` §5.
//!
//! The values are those of `KEY_*` in `<linux/input-event-codes.h>`, hardcoded rather than
//! re-exported from a crate. They are **stable userspace ABI** — the kernel cannot renumber them
//! without breaking every existing input driver and every recorded event stream — so writing
//! them out costs nothing in correctness and buys a table that compiles and is tested on any
//! host, including the Windows development machine. Only `backend.rs` is Linux-only.
//!
//! Re-derived against the canonical key list rather than copied from the pre-rewrite Go
//! implementation, which predates it and disagrees in a dozen places: `SUPER` for `META`,
//! `PAGE_UP` for `PAGEUP`, `GRAVE` for `BACKQUOTE`, `LEFT_BRACKET` for `BRACKET_LEFT`. The
//! legacy *values* were worth mining; its *names* were not.

use muxdeck_core::{Key, MediaCommand};

/// `KEY_*` codes from `<linux/input-event-codes.h>`.
mod key {
    pub const ESC: u16 = 1;
    pub const N1: u16 = 2;
    pub const N2: u16 = 3;
    pub const N3: u16 = 4;
    pub const N4: u16 = 5;
    pub const N5: u16 = 6;
    pub const N6: u16 = 7;
    pub const N7: u16 = 8;
    pub const N8: u16 = 9;
    pub const N9: u16 = 10;
    pub const N0: u16 = 11;
    pub const MINUS: u16 = 12;
    pub const EQUAL: u16 = 13;
    pub const BACKSPACE: u16 = 14;
    pub const TAB: u16 = 15;
    pub const Q: u16 = 16;
    pub const W: u16 = 17;
    pub const E: u16 = 18;
    pub const R: u16 = 19;
    pub const T: u16 = 20;
    pub const Y: u16 = 21;
    pub const U: u16 = 22;
    pub const I: u16 = 23;
    pub const O: u16 = 24;
    pub const P: u16 = 25;
    pub const LEFTBRACE: u16 = 26;
    pub const RIGHTBRACE: u16 = 27;
    pub const ENTER: u16 = 28;
    pub const LEFTCTRL: u16 = 29;
    pub const A: u16 = 30;
    pub const S: u16 = 31;
    pub const D: u16 = 32;
    pub const F: u16 = 33;
    pub const G: u16 = 34;
    pub const H: u16 = 35;
    pub const J: u16 = 36;
    pub const K: u16 = 37;
    pub const L: u16 = 38;
    pub const SEMICOLON: u16 = 39;
    pub const APOSTROPHE: u16 = 40;
    pub const GRAVE: u16 = 41;
    pub const LEFTSHIFT: u16 = 42;
    pub const BACKSLASH: u16 = 43;
    pub const Z: u16 = 44;
    pub const X: u16 = 45;
    pub const C: u16 = 46;
    pub const V: u16 = 47;
    pub const B: u16 = 48;
    pub const N: u16 = 49;
    pub const M: u16 = 50;
    pub const COMMA: u16 = 51;
    pub const DOT: u16 = 52;
    pub const SLASH: u16 = 53;
    pub const KPASTERISK: u16 = 55;
    pub const LEFTALT: u16 = 56;
    pub const SPACE: u16 = 57;
    pub const CAPSLOCK: u16 = 58;
    pub const F1: u16 = 59;
    pub const F10: u16 = 68;
    pub const NUMLOCK: u16 = 69;
    pub const SCROLLLOCK: u16 = 70;
    pub const KP7: u16 = 71;
    pub const KP8: u16 = 72;
    pub const KP9: u16 = 73;
    pub const KPMINUS: u16 = 74;
    pub const KP4: u16 = 75;
    pub const KP5: u16 = 76;
    pub const KP6: u16 = 77;
    pub const KPPLUS: u16 = 78;
    pub const KP1: u16 = 79;
    pub const KP2: u16 = 80;
    pub const KP3: u16 = 81;
    pub const KP0: u16 = 82;
    pub const KPDOT: u16 = 83;
    pub const F11: u16 = 87;
    pub const F12: u16 = 88;
    pub const KPENTER: u16 = 96;
    pub const KPSLASH: u16 = 98;
    pub const SYSRQ: u16 = 99;
    pub const HOME: u16 = 102;
    pub const UP: u16 = 103;
    pub const PAGEUP: u16 = 104;
    pub const LEFT: u16 = 105;
    pub const RIGHT: u16 = 106;
    pub const END: u16 = 107;
    pub const DOWN: u16 = 108;
    pub const PAGEDOWN: u16 = 109;
    pub const INSERT: u16 = 110;
    pub const DELETE: u16 = 111;
    pub const MUTE: u16 = 113;
    pub const VOLUMEDOWN: u16 = 114;
    pub const VOLUMEUP: u16 = 115;
    pub const PAUSE: u16 = 119;
    pub const LEFTMETA: u16 = 125;
    pub const COMPOSE: u16 = 127;
    pub const STOPCD: u16 = 166;
    pub const F13: u16 = 183;
    pub const F24: u16 = 194;
    pub const PLAYPAUSE: u16 = 164;
    pub const NEXTSONG: u16 = 163;
    pub const PREVIOUSSONG: u16 = 165;
}

/// The `KEY_*` code for a canonical key name.
///
/// Total: every canonical key maps to something, checked exhaustively by the compiler.
pub fn key_code(key: Key) -> u16 {
    use Key as K;

    match key {
        // Modifiers map to the *left* variant. The protocol has one `CONTROL`, not two, and a
        // macro deck has no reason to distinguish them.
        K::Control => key::LEFTCTRL,
        K::Shift => key::LEFTSHIFT,
        K::Alt => key::LEFTALT,
        K::Meta => key::LEFTMETA,

        // Letters are *not* contiguous on Linux: the codes follow the physical QWERTY rows, so
        // A is 30 and B is 48. Enumerated rather than computed for exactly that reason.
        K::A => key::A,
        K::B => key::B,
        K::C => key::C,
        K::D => key::D,
        K::E => key::E,
        K::F => key::F,
        K::G => key::G,
        K::H => key::H,
        K::I => key::I,
        K::J => key::J,
        K::K => key::K,
        K::L => key::L,
        K::M => key::M,
        K::N => key::N,
        K::O => key::O,
        K::P => key::P,
        K::Q => key::Q,
        K::R => key::R,
        K::S => key::S,
        K::T => key::T,
        K::U => key::U,
        K::V => key::V,
        K::W => key::W,
        K::X => key::X,
        K::Y => key::Y,
        K::Z => key::Z,

        // Digits run 1..9 then 0 — KEY_0 follows KEY_9, matching the keyboard row rather than
        // numeric order.
        K::Digit0 => key::N0,
        K::Digit1 => key::N1,
        K::Digit2 => key::N2,
        K::Digit3 => key::N3,
        K::Digit4 => key::N4,
        K::Digit5 => key::N5,
        K::Digit6 => key::N6,
        K::Digit7 => key::N7,
        K::Digit8 => key::N8,
        K::Digit9 => key::N9,

        // F1..F10 are contiguous; F11 and F12 sit elsewhere; F13..F24 are contiguous again.
        K::F1 => key::F1,
        K::F2 => key::F1 + 1,
        K::F3 => key::F1 + 2,
        K::F4 => key::F1 + 3,
        K::F5 => key::F1 + 4,
        K::F6 => key::F1 + 5,
        K::F7 => key::F1 + 6,
        K::F8 => key::F1 + 7,
        K::F9 => key::F1 + 8,
        K::F10 => key::F10,
        K::F11 => key::F11,
        K::F12 => key::F12,
        K::F13 => key::F13,
        K::F14 => key::F13 + 1,
        K::F15 => key::F13 + 2,
        K::F16 => key::F13 + 3,
        K::F17 => key::F13 + 4,
        K::F18 => key::F13 + 5,
        K::F19 => key::F13 + 6,
        K::F20 => key::F13 + 7,
        K::F21 => key::F13 + 8,
        K::F22 => key::F13 + 9,
        K::F23 => key::F13 + 10,
        K::F24 => key::F24,

        K::Escape => key::ESC,
        K::Tab => key::TAB,
        K::CapsLock => key::CAPSLOCK,
        K::Space => key::SPACE,
        K::Enter => key::ENTER,
        K::Backspace => key::BACKSPACE,
        K::Delete => key::DELETE,
        K::Insert => key::INSERT,
        K::Home => key::HOME,
        K::End => key::END,
        K::PageUp => key::PAGEUP,
        K::PageDown => key::PAGEDOWN,
        K::Left => key::LEFT,
        K::Right => key::RIGHT,
        K::Up => key::UP,
        K::Down => key::DOWN,

        // Numpad digits are in keypad layout order — KP7 is the lowest code, not KP0.
        K::Numpad0 => key::KP0,
        K::Numpad1 => key::KP1,
        K::Numpad2 => key::KP2,
        K::Numpad3 => key::KP3,
        K::Numpad4 => key::KP4,
        K::Numpad5 => key::KP5,
        K::Numpad6 => key::KP6,
        K::Numpad7 => key::KP7,
        K::Numpad8 => key::KP8,
        K::Numpad9 => key::KP9,
        K::NumpadAdd => key::KPPLUS,
        K::NumpadSub => key::KPMINUS,
        K::NumpadMul => key::KPASTERISK,
        K::NumpadDiv => key::KPSLASH,
        K::NumpadDecimal => key::KPDOT,
        // Unlike Windows, Linux gives the numpad Enter its own code rather than distinguishing
        // it by a flag — so no extended-key handling is needed here.
        K::NumpadEnter => key::KPENTER,

        K::Minus => key::MINUS,
        K::Equal => key::EQUAL,
        K::BracketLeft => key::LEFTBRACE,
        K::BracketRight => key::RIGHTBRACE,
        K::Backslash => key::BACKSLASH,
        K::Semicolon => key::SEMICOLON,
        K::Quote => key::APOSTROPHE,
        K::Backquote => key::GRAVE,
        K::Comma => key::COMMA,
        K::Period => key::DOT,
        K::Slash => key::SLASH,

        K::PrintScreen => key::SYSRQ,
        K::ScrollLock => key::SCROLLLOCK,
        K::Pause => key::PAUSE,
        K::NumLock => key::NUMLOCK,
        K::Menu => key::COMPOSE,
    }
}

/// The `KEY_*` code for a media command.
pub fn media_code(command: MediaCommand) -> u16 {
    match command {
        MediaCommand::PlayPause => key::PLAYPAUSE,
        MediaCommand::Next => key::NEXTSONG,
        MediaCommand::Prev => key::PREVIOUSSONG,
        MediaCommand::Stop => key::STOPCD,
        MediaCommand::VolumeUp => key::VOLUMEUP,
        MediaCommand::VolumeDown => key::VOLUMEDOWN,
        MediaCommand::Mute => key::MUTE,
    }
}

/// Every code the backend may ever emit.
///
/// **A uinput device must declare each key at creation time**; one not registered is silently
/// dropped rather than rejected. So this list is not an optimisation — it is the difference
/// between a working key and a dead one, and it must stay exhaustive.
pub fn all_codes() -> Vec<u16> {
    let mut codes: Vec<u16> = Key::ALL.iter().map(|k| key_code(*k)).collect();
    codes.extend(MediaCommand::ALL.iter().map(|c| media_code(*c)));
    codes.sort_unstable();
    codes.dedup();
    codes
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_key_maps_into_the_valid_range() {
        for key in Key::ALL {
            let code = key_code(*key);
            assert!(code > 0, "{key:?} maps to 0, which is KEY_RESERVED");
            // KEY_MAX is 0x2ff; anything beyond is not a key code at all.
            assert!(
                code < 0x300,
                "{key:?} maps to {code}, outside the key range"
            );
        }
    }

    #[test]
    fn no_two_keys_share_a_code() {
        // Unlike Windows — where Enter and numpad Enter legitimately share a virtual key and are
        // told apart by a flag — Linux gives every key its own code. Any collision here is a
        // transcription error.
        let mut seen = std::collections::HashMap::new();
        for key in Key::ALL {
            let code = key_code(*key);
            if let Some(previous) = seen.insert(code, key) {
                panic!("{key:?} and {previous:?} both map to {code}");
            }
        }
    }

    #[test]
    fn letters_follow_the_keyboard_rows_not_the_alphabet() {
        // The trap this guards: assuming KEY_A..KEY_Z are contiguous like the Windows virtual
        // keys are. They are not — the codes follow the physical QWERTY rows.
        assert_eq!(key_code(Key::A), 30);
        assert_eq!(key_code(Key::Q), 16);
        assert_eq!(key_code(Key::Z), 44);
        assert_ne!(
            key_code(Key::B),
            key_code(Key::A) + 1,
            "computing letters by offset would silently produce the wrong keys"
        );
    }

    #[test]
    fn digits_run_one_to_nine_then_zero() {
        // KEY_0 follows KEY_9, matching the keyboard row rather than numeric order.
        assert_eq!(key_code(Key::Digit1), 2);
        assert_eq!(key_code(Key::Digit9), 10);
        assert_eq!(key_code(Key::Digit0), 11);
    }

    #[test]
    fn function_key_runs_are_contiguous_where_the_kernel_says_they_are() {
        assert_eq!(key_code(Key::F1), 59);
        assert_eq!(key_code(Key::F9), 67);
        // F10, F11 and F12 are *not* a continuation of F1..F9.
        assert_eq!(key_code(Key::F10), 68);
        assert_eq!(key_code(Key::F11), 87);
        assert_eq!(key_code(Key::F12), 88);
        // F13..F24 start a fresh contiguous run.
        assert_eq!(key_code(Key::F13), 183);
        assert_eq!(key_code(Key::F24), 194);
    }

    #[test]
    fn the_numpad_follows_keypad_layout_order() {
        // KP7 is the lowest code, not KP0 — the codes run in the order the keys are laid out.
        assert_eq!(key_code(Key::Numpad7), 71);
        assert_eq!(key_code(Key::Numpad0), 82);
        assert_eq!(key_code(Key::NumpadEnter), 96);
        assert_ne!(
            key_code(Key::NumpadEnter),
            key_code(Key::Enter),
            "Linux gives the numpad Enter its own code, unlike Windows"
        );
    }

    #[test]
    fn every_media_command_maps_into_the_media_block() {
        for command in MediaCommand::ALL {
            let code = media_code(*command);
            assert!(
                (113..=166).contains(&code),
                "{command:?} maps to {code}, outside the volume and media block"
            );
        }
    }

    #[test]
    fn the_registration_list_covers_everything_the_backend_can_send() {
        // A key not declared at device creation is silently dropped, so this list missing an
        // entry is a dead button rather than an error.
        let codes = all_codes();
        for key in Key::ALL {
            assert!(
                codes.contains(&key_code(*key)),
                "{key:?} would not be registered"
            );
        }
        for command in MediaCommand::ALL {
            assert!(
                codes.contains(&media_code(*command)),
                "{command:?} would not be registered"
            );
        }
    }
}
