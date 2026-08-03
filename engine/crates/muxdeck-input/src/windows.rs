//! Windows input injection through `SendInput`. `docs/ENGINE.md` §4.1.

use std::time::Duration;

use muxdeck_core::{Key, MediaCommand, MouseButton};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyW, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT,
    KEYBD_EVENT_FLAGS, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP, KEYEVENTF_UNICODE, MAPVK_VK_TO_VSC,
    MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_HWHEEL, MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP,
    MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE, MOUSEEVENTF_RIGHTDOWN,
    MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL, MOUSEINPUT, VIRTUAL_KEY,
};

use crate::keymap::{is_extended, media_key, virtual_key};
use crate::{BackendCapabilities, InputBackend, InputError, MouseEvent};

/// One wheel detent. `WHEEL_DELTA` from `winuser.h`.
const WHEEL_DELTA: f64 = 120.0;

/// `MOUSEEVENTF_ABSOLUTE` coordinates are normalised to this range across the primary monitor.
const ABSOLUTE_RANGE: f64 = 65535.0;

pub struct WindowsBackend;

impl WindowsBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBackend for WindowsBackend {
    /// Presses in order, holds, releases in reverse.
    ///
    /// The press batch goes out as **one** `SendInput` call. Separate calls can be interleaved
    /// by real user input, which drops modifiers and turns `CONTROL+C` into a bare `C` in
    /// whatever window has focus. The release batch is likewise a single call.
    fn key_combo(&self, keys: &[Key], hold: Duration) -> Result<(), InputError> {
        if keys.is_empty() {
            return Ok(());
        }

        let press: Vec<INPUT> = keys.iter().map(|key| key_event(*key, false)).collect();
        let release: Vec<INPUT> = keys.iter().rev().map(|key| key_event(*key, true)).collect();

        let press_result = send(&press);

        if !hold.is_zero() {
            std::thread::sleep(hold);
        }

        // Released unconditionally: if the press batch partly landed, skipping the release
        // would latch a modifier across the whole desktop, and that outlives this process.
        let release_result = send(&release);

        press_result.and(release_result)
    }

    /// Types a literal string with `KEYEVENTF_UNICODE`.
    ///
    /// This bypasses the keyboard layout entirely — the OS delivers the character rather than
    /// a key position — so it is correct on AZERTY, Dvorak and everything else without a
    /// per-layout table.
    fn text(&self, text: &str) -> Result<(), InputError> {
        if text.is_empty() {
            return Ok(());
        }

        // Two events per UTF-16 code unit, so a character outside the BMP costs four: its
        // surrogate pair must be sent as two units, each with its own down and up.
        let mut events = Vec::with_capacity(text.len() * 2);
        for unit in text.encode_utf16() {
            events.push(unicode_event(unit, false));
            events.push(unicode_event(unit, true));
        }
        send(&events)
    }

    /// Media keys are virtual-key only — there is no scancode to send with them.
    fn media(&self, command: MediaCommand) -> Result<(), InputError> {
        let vk = media_key(command);
        let events = [
            raw_key_event(vk, 0, KEYBD_EVENT_FLAGS(0)),
            raw_key_event(vk, 0, KEYEVENTF_KEYUP),
        ];
        send(&events)
    }

    fn mouse(&self, event: MouseEvent) -> Result<(), InputError> {
        let events: Vec<INPUT> = match event {
            MouseEvent::MoveRelative { dx, dy } => {
                vec![mouse_event(dx, dy, 0, MOUSEEVENTF_MOVE)]
            }
            MouseEvent::MoveAbsolute { x, y } => {
                // Clamped rather than rejected: a client sending 1.2 means "the far edge", and
                // refusing the press would be a worse answer than putting the cursor there.
                let to_absolute = |value: f64| (value.clamp(0.0, 1.0) * ABSOLUTE_RANGE) as i32;
                vec![mouse_event(
                    to_absolute(x),
                    to_absolute(y),
                    0,
                    MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                )]
            }
            MouseEvent::Click(button) => {
                let (down, up) = button_flags(button);
                vec![mouse_event(0, 0, 0, down), mouse_event(0, 0, 0, up)]
            }
            MouseEvent::Down(button) => vec![mouse_event(0, 0, 0, button_flags(button).0)],
            MouseEvent::Up(button) => vec![mouse_event(0, 0, 0, button_flags(button).1)],
            MouseEvent::Scroll { dx, dy } => {
                let mut events = Vec::new();
                if dy != 0.0 {
                    events.push(mouse_event(
                        0,
                        0,
                        (dy * WHEEL_DELTA) as i32,
                        MOUSEEVENTF_WHEEL,
                    ));
                }
                if dx != 0.0 {
                    events.push(mouse_event(
                        0,
                        0,
                        (dx * WHEEL_DELTA) as i32,
                        MOUSEEVENTF_HWHEEL,
                    ));
                }
                events
            }
        };

        if events.is_empty() {
            return Ok(());
        }
        send(&events)
    }

    /// Always available on Windows.
    ///
    /// There is no permission to request: `SendInput` works for any interactive process. The
    /// one real limit is UIPI — a non-elevated daemon cannot inject into an elevated window —
    /// but that is per-window and cannot be detected here, so it is reported when a press to
    /// such a window fails rather than pre-emptively refusing to start.
    fn preflight(&self) -> Result<(), InputError> {
        Ok(())
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_unicode: true,
            media_keys: true,
            mouse: true,
        }
    }

    fn name(&self) -> &'static str {
        "sendinput"
    }
}

/// Sends a batch, and reports how far it got.
fn send(events: &[INPUT]) -> Result<(), InputError> {
    // SAFETY: `events` is a valid slice for its length, and `size_of::<INPUT>()` is exactly
    // what SendInput validates against. A wrong size makes every call fail with
    // ERROR_INVALID_PARAMETER, which presents as "keys silently do nothing".
    let inserted = unsafe { SendInput(events, std::mem::size_of::<INPUT>() as i32) };

    if inserted as usize == events.len() {
        Ok(())
    } else {
        let error = windows::core::Error::from_thread();
        Err(InputError::Rejected(format!(
            "SendInput accepted {inserted} of {} events: {error}. \
             A non-elevated daemon cannot inject into an elevated window.",
            events.len()
        )))
    }
}

/// A key event carrying both the virtual key and its scancode.
///
/// **Both fields are filled in, and `KEYEVENTF_SCANCODE` is deliberately not set.** Ordinary
/// applications read `wVk`, which is layout-correct because `MapVirtualKeyW` resolves the
/// scancode against the *current* layout. Games and anything on raw input read `wScan`, which
/// they need to see a real physical key. Setting `KEYEVENTF_SCANCODE` would satisfy only the
/// second group and would send the wrong letter on a non-US layout; sending no scancode would
/// satisfy only the first and would make the deck useless in games.
fn key_event(key: Key, up: bool) -> INPUT {
    let vk = virtual_key(key);

    // SAFETY: MapVirtualKeyW is a pure lookup against the current keyboard layout and cannot
    // fail; it returns 0 for a key with no scancode, which is a valid value to send.
    let scan = unsafe { MapVirtualKeyW(u32::from(vk), MAPVK_VK_TO_VSC) } as u16;

    let mut flags = KEYBD_EVENT_FLAGS(0);
    if is_extended(key) {
        flags |= KEYEVENTF_EXTENDEDKEY;
    }
    if up {
        flags |= KEYEVENTF_KEYUP;
    }

    raw_key_event(vk, scan, flags)
}

/// One UTF-16 code unit, delivered as a character rather than a key.
fn unicode_event(unit: u16, up: bool) -> INPUT {
    let mut flags = KEYEVENTF_UNICODE;
    if up {
        flags |= KEYEVENTF_KEYUP;
    }
    // wVk must be zero for a unicode event; the code unit travels in wScan.
    raw_key_event(0, unit, flags)
}

fn raw_key_event(vk: u16, scan: u16, flags: KEYBD_EVENT_FLAGS) -> INPUT {
    INPUT {
        r#type: INPUT_KEYBOARD,
        Anonymous: INPUT_0 {
            ki: KEYBDINPUT {
                wVk: VIRTUAL_KEY(vk),
                wScan: scan,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn mouse_event(
    dx: i32,
    dy: i32,
    data: i32,
    flags: windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) -> INPUT {
    INPUT {
        r#type: INPUT_MOUSE,
        Anonymous: INPUT_0 {
            mi: MOUSEINPUT {
                dx,
                dy,
                mouseData: data as u32,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            },
        },
    }
}

fn button_flags(
    button: MouseButton,
) -> (
    windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
    windows::Win32::UI::Input::KeyboardAndMouse::MOUSE_EVENT_FLAGS,
) {
    match button {
        MouseButton::Left => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
        MouseButton::Right => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
        MouseButton::Middle => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These build events without sending them, so they run on a CI runner with no desktop
    // session. Anything that actually injects is #[ignore]d below.

    #[test]
    fn a_key_event_carries_both_the_virtual_key_and_a_scancode() {
        let event = key_event(Key::A, false);
        // SAFETY: the union was just written as a keyboard event.
        let ki = unsafe { event.Anonymous.ki };
        assert_eq!(ki.wVk.0, 0x41);
        assert_ne!(ki.wScan, 0, "games and raw-input apps read wScan, not wVk");
        assert_eq!(ki.dwFlags & KEYEVENTF_KEYUP, KEYBD_EVENT_FLAGS(0));
    }

    #[test]
    fn extended_keys_carry_the_flag_and_ordinary_ones_do_not() {
        let delete = unsafe { key_event(Key::Delete, false).Anonymous.ki };
        assert_eq!(
            delete.dwFlags & KEYEVENTF_EXTENDEDKEY,
            KEYEVENTF_EXTENDEDKEY
        );

        let letter = unsafe { key_event(Key::A, false).Anonymous.ki };
        assert_eq!(letter.dwFlags & KEYEVENTF_EXTENDEDKEY, KEYBD_EVENT_FLAGS(0));
    }

    #[test]
    fn a_unicode_event_sends_the_code_unit_and_no_virtual_key() {
        let event = unicode_event(0x263A, false);
        let ki = unsafe { event.Anonymous.ki };
        assert_eq!(ki.wVk.0, 0, "wVk must be zero for a unicode event");
        assert_eq!(ki.wScan, 0x263A);
        assert_eq!(ki.dwFlags & KEYEVENTF_UNICODE, KEYEVENTF_UNICODE);
    }

    #[test]
    fn the_input_struct_is_the_size_win32_expects() {
        // A layout mismatch makes SendInput reject every call with ERROR_INVALID_PARAMETER,
        // which presents as "the buttons do nothing" rather than as an error.
        let expected = if cfg!(target_pointer_width = "64") {
            40
        } else {
            28
        };
        assert_eq!(std::mem::size_of::<INPUT>(), expected);
    }

    // --- manual smoke tests -------------------------------------------------
    // Run with: cargo test -p muxdeck-input -- --ignored --test-threads=1
    // CI runners have no desktop session to inject into, so these never run there.

    #[test]
    #[ignore = "types into whatever window has focus"]
    fn smoke_types_text() {
        WindowsBackend::new().text("muxdeck").expect("text");
    }

    #[test]
    #[ignore = "sends a real key combo to whatever window has focus"]
    fn smoke_selects_all() {
        WindowsBackend::new()
            .key_combo(&[Key::Control, Key::A], Duration::ZERO)
            .expect("combo");
    }
}
