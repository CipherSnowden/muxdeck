//! Quartz Event Services. `docs/ENGINE.md` §4.2.
//!
//! # This code has never injected a keystroke
//!
//! There is no Mac on the development machine and a CI runner has no desktop session, so
//! everything here is written against Apple's documentation and proved only to *compile* on
//! `macos-latest`. `docs/BUILD-PLAN.md` records that plainly. Treat a bug report from a Mac user
//! as the first real test run, and start with [`post_media`] — the `NSSystemDefined` path is by
//! some distance the most intricate part.

use std::time::Duration;

use core_graphics::display::CGDisplay;
use core_graphics::event::{
    CGEvent, CGEventFlags, CGEventTapLocation, CGEventType, CGMouseButton, EventField,
    ScrollEventUnit,
};
use core_graphics::event_source::{CGEventSource, CGEventSourceStateID};
use core_graphics::geometry::CGPoint;
use muxdeck_core::{Key, MediaCommand, MouseButton};
use objc2::encode::{Encode, Encoding, RefEncode};
use objc2::msg_send;
use objc2::rc::autoreleasepool;
use objc2::runtime::{AnyClass, AnyObject};

use super::keymap::{key_code, media_code, modifier_flag};
use crate::{BackendCapabilities, InputBackend, InputError, MouseEvent};

/// How many lines one scroll notch is worth.
///
/// macOS scroll events are in lines, not the fixed 120-unit detents Windows uses, and three is
/// what a classic wheel mouse reports. It is the one number here likely to want tuning against a
/// real trackpad, so it is named rather than inline.
const LINES_PER_NOTCH: f64 = 3.0;

/// The most UTF-16 units to attach to a single keyboard event.
///
/// `CGEventKeyboardSetUnicodeString` takes an arbitrary length, but long strings are unreliable
/// in practice — characters get dropped somewhere between the event tap and the application — so
/// text is posted in small batches. Twenty is the figure the accessibility tooling community
/// settled on.
const TEXT_CHUNK_UTF16: usize = 20;

pub struct MacosBackend;

impl MacosBackend {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MacosBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBackend for MacosBackend {
    /// Presses in order, holds, releases in reverse.
    ///
    /// **Modifiers become flags on the key event rather than separate key presses**
    /// (`docs/ENGINE.md` §4.2). Posting a real `Command` key-down and then a separate `C`
    /// key-down works in some applications and silently loses the modifier in others, because
    /// what an application reads is the flag field of the event it receives, not the history of
    /// what was pressed before it.
    fn key_combo(&self, keys: &[Key], hold: Duration) -> Result<(), InputError> {
        if keys.is_empty() {
            return Ok(());
        }

        let mut flags = 0u64;
        let mut codes = Vec::with_capacity(keys.len());

        for key in keys {
            match modifier_flag(*key) {
                Some(flag) => flags |= flag,
                None => codes.push(resolve(*key)?),
            }
        }

        // A combo of nothing but modifiers — a button that just holds Shift — has no key event
        // to hang the flags on, so the modifiers are posted as real key presses instead. This is
        // the one case where the flags approach cannot apply.
        if codes.is_empty() {
            codes = keys
                .iter()
                .map(|key| resolve(*key))
                .collect::<Result<_, _>>()?;
        }

        let flags = CGEventFlags::from_bits_truncate(flags);
        let press = post_keys(&codes, true, flags);

        if !hold.is_zero() {
            std::thread::sleep(hold);
        }

        // Released unconditionally: a latched Command key makes every other keystroke on the
        // desktop a shortcut, and it outlives this process.
        let reversed: Vec<u16> = codes.into_iter().rev().collect();
        let release = post_keys(&reversed, false, flags);

        press.and(release)
    }

    /// Types a literal string with `CGEventKeyboardSetUnicodeString`.
    ///
    /// This bypasses the keyboard layout entirely — the OS delivers the characters rather than
    /// key positions — so it is correct on AZERTY, Dvorak and everything else without a
    /// per-layout table. It is the same property `KEYEVENTF_UNICODE` gives on Windows, and the
    /// thing Linux has no equivalent of.
    fn text(&self, text: &str) -> Result<(), InputError> {
        if text.is_empty() {
            return Ok(());
        }

        for chunk in chunk_by_utf16(text, TEXT_CHUNK_UTF16) {
            // The key code is ignored once a unicode string is attached, so it is set to zero
            // rather than to anything meaningful.
            let down = keyboard_event(0, true)?;
            down.set_string(chunk);
            down.post(CGEventTapLocation::HID);

            let up = keyboard_event(0, false)?;
            up.set_string(chunk);
            up.post(CGEventTapLocation::HID);
        }

        Ok(())
    }

    fn media(&self, command: MediaCommand) -> Result<(), InputError> {
        let code = media_code(command).ok_or_else(|| {
            InputError::Unsupported(format!(
                "macOS has no {command:?} key: the hardware has never had one, so nothing \
                 listens for it. Use PLAY_PAUSE instead."
            ))
        })?;

        post_media(code, true)?;
        post_media(code, false)
    }

    fn mouse(&self, event: MouseEvent) -> Result<(), InputError> {
        match event {
            MouseEvent::MoveRelative { dx, dy } => {
                let at = cursor_location()?;
                let to = CGPoint::new(at.x + f64::from(dx), at.y + f64::from(dy));
                post_mouse(CGEventType::MouseMoved, to, CGMouseButton::Left, 0)
            }
            MouseEvent::MoveAbsolute { x, y } => {
                let display = CGDisplay::main();
                // Clamped rather than rejected: a client sending 1.2 means "the far edge", and
                // refusing the press would be a worse answer than putting the cursor there.
                let to = CGPoint::new(
                    x.clamp(0.0, 1.0) * display.pixels_wide() as f64,
                    y.clamp(0.0, 1.0) * display.pixels_high() as f64,
                );
                post_mouse(CGEventType::MouseMoved, to, CGMouseButton::Left, 0)
            }
            MouseEvent::Click(button) => {
                let at = cursor_location()?;
                let (down, up, button) = button_events(button);
                let pressed = post_mouse(down, at, button, 1);
                let released = post_mouse(up, at, button, 1);
                pressed.and(released)
            }
            MouseEvent::Down(button) => {
                let at = cursor_location()?;
                let (down, _, button) = button_events(button);
                post_mouse(down, at, button, 1)
            }
            MouseEvent::Up(button) => {
                let at = cursor_location()?;
                let (_, up, button) = button_events(button);
                post_mouse(up, at, button, 1)
            }
            MouseEvent::Scroll { dx, dy } => {
                let vertical = (dy * LINES_PER_NOTCH) as i32;
                let horizontal = (dx * LINES_PER_NOTCH) as i32;
                if vertical == 0 && horizontal == 0 {
                    return Ok(());
                }
                let event = CGEvent::new_scroll_event(
                    source()?,
                    ScrollEventUnit::LINE,
                    2,
                    vertical,
                    horizontal,
                    0,
                )
                .map_err(|_| InputError::Rejected("could not build a scroll event".to_string()))?;
                event.post(CGEventTapLocation::HID);
                Ok(())
            }
        }
    }

    /// Whether the process holds **Accessibility** permission.
    ///
    /// Without it `CGEventPost` returns no error and does nothing at all, which is the single
    /// most confusing failure on this platform — every button appears to work and none of them
    /// do. This is what lets the control panel say so up front (`docs/SERVER.md` §6).
    ///
    /// Deliberately the non-prompting check: a daemon that pops a system dialog at startup, from
    /// no window and possibly at login, is worse than one that reports the problem and lets the
    /// panel walk the user through System Settings.
    fn preflight(&self) -> Result<(), InputError> {
        // SAFETY: a parameterless framework predicate with no arguments to get wrong.
        if unsafe { AXIsProcessTrusted() } {
            Ok(())
        } else {
            Err(InputError::NotPermitted(
                "MuxDeck does not have Accessibility permission, so keystrokes are silently \
                 discarded. Open System Settings > Privacy & Security > Accessibility, add \
                 MuxDeck, and turn it on. The permission is remembered per application binary, \
                 so moving or replacing the app requires granting it again."
                    .to_string(),
            ))
        }
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_unicode: true,
            media_keys: true,
            mouse: true,
        }
    }

    fn name(&self) -> &'static str {
        "cgevent"
    }
}

/// The virtual key code for a key, or an error naming what macOS cannot express.
fn resolve(key: Key) -> Result<u16, InputError> {
    key_code(key).ok_or_else(|| {
        InputError::Unsupported(format!(
            "macOS has no {key:?} key. Apple keyboards have never had Print Screen, Scroll Lock, \
             Pause, a Menu key, or F21 to F24, and there is no key code that means any of them."
        ))
    })
}

/// A fresh event source.
///
/// Built per call rather than cached because `CGEventSource` is a Core Foundation object with no
/// thread-safety guarantee, and [`InputBackend`] is `Sync`. Creating one is cheap next to the
/// press rate of a human thumb.
fn source() -> Result<CGEventSource, InputError> {
    // `HIDSystemState` rather than `CombinedSessionState`: the event should behave as though it
    // came from real hardware, which is what makes modifier flags apply the way applications
    // expect.
    CGEventSource::new(CGEventSourceStateID::HIDSystemState)
        .map_err(|_| InputError::Rejected("could not create an event source".to_string()))
}

fn keyboard_event(code: u16, down: bool) -> Result<CGEvent, InputError> {
    CGEvent::new_keyboard_event(source()?, code, down)
        .map_err(|_| InputError::Rejected("could not build a keyboard event".to_string()))
}

/// Posts one key event per code, all carrying the same modifier flags.
fn post_keys(codes: &[u16], down: bool, flags: CGEventFlags) -> Result<(), InputError> {
    for code in codes {
        let event = keyboard_event(*code, down)?;
        // Set on the key-up as well as the key-down: real hardware reports the modifier as still
        // held while the key is released, and applications that track flag transitions notice
        // when it is not.
        event.set_flags(flags);
        event.post(CGEventTapLocation::HID);
    }
    Ok(())
}

fn cursor_location() -> Result<CGPoint, InputError> {
    // An event created from a source with no type carries the current cursor position, which is
    // the documented way to read it without an event tap.
    Ok(CGEvent::new(source()?)
        .map_err(|_| InputError::Rejected("could not read the cursor position".to_string()))?
        .location())
}

fn post_mouse(
    event_type: CGEventType,
    at: CGPoint,
    button: CGMouseButton,
    click_state: i64,
) -> Result<(), InputError> {
    let event = CGEvent::new_mouse_event(source()?, event_type, at, button)
        .map_err(|_| InputError::Rejected("could not build a mouse event".to_string()))?;

    if click_state > 0 {
        // Without a click state a synthetic click is delivered but not counted, and applications
        // that distinguish single from double clicks — which is most of them — ignore it.
        event.set_integer_value_field(EventField::MOUSE_EVENT_CLICK_STATE, click_state);
    }

    event.post(CGEventTapLocation::HID);
    Ok(())
}

fn button_events(button: MouseButton) -> (CGEventType, CGEventType, CGMouseButton) {
    match button {
        MouseButton::Left => (
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGMouseButton::Left,
        ),
        MouseButton::Right => (
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGMouseButton::Right,
        ),
        MouseButton::Middle => (
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGMouseButton::Center,
        ),
    }
}

/// Splits a string into pieces of at most `limit` UTF-16 code units, never inside a character.
fn chunk_by_utf16(text: &str, limit: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    let mut units = 0;

    for (index, character) in text.char_indices() {
        let width = character.len_utf16();
        if units + width > limit {
            chunks.push(&text[start..index]);
            start = index;
            units = 0;
        }
        units += width;
    }

    if start < text.len() {
        chunks.push(&text[start..]);
    }
    chunks
}

// --- the NSSystemDefined path -------------------------------------------------------------
//
// Media keys are not keyboard events on macOS. They arrive as `NSEventTypeSystemDefined` with
// subtype 8, the key code and its up/down state packed into `data1` (`docs/ENGINE.md` §4.2).
// There is no Core Graphics constructor for such an event, so the only way to build one is
// `+[NSEvent otherEventWithType:...]` and then take its `CGEvent` — which is why this is the one
// place in the workspace that speaks Objective-C.

/// AppKit is linked for `NSEvent` alone; nothing else in the daemon touches it.
#[link(name = "AppKit", kind = "framework")]
extern "C" {}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    fn AXIsProcessTrusted() -> bool;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    /// Posting a raw `CGEventRef`, because the one here comes from `NSEvent` rather than from a
    /// `core-graphics` constructor.
    fn CGEventPost(tap: u32, event: *mut OpaqueCGEvent);
}

/// `kCGHIDEventTap` — inject at the lowest point, as though from real hardware.
const HID_EVENT_TAP: u32 = 0;

/// `NSEventTypeSystemDefined`.
const SYSTEM_DEFINED: usize = 14;

/// `NX_SUBTYPE_AUX_CONTROL_BUTTONS` — the subtype the media keys travel under.
const AUX_CONTROL_BUTTONS: i16 = 8;

/// An opaque `CGEventRef` target.
///
/// Declared here rather than borrowed from `core-graphics` so the Objective-C return type can
/// carry a matching encoding; the runtime checks these in debug builds.
#[repr(C)]
struct OpaqueCGEvent {
    _private: [u8; 0],
}

unsafe impl RefEncode for OpaqueCGEvent {
    const ENCODING_REF: Encoding = Encoding::Pointer(&Encoding::Struct("__CGEvent", &[]));
}

/// `NSPoint`, which on 64-bit macOS is `CGPoint`.
#[repr(C)]
#[derive(Clone, Copy)]
struct NSPoint {
    x: f64,
    y: f64,
}

unsafe impl Encode for NSPoint {
    const ENCODING: Encoding = Encoding::Struct("CGPoint", &[Encoding::Double, Encoding::Double]);
}

/// Posts one media key transition.
fn post_media(code: u8, down: bool) -> Result<(), InputError> {
    // NSEvent hands back an autoreleased object. Without a pool on this thread it is never
    // freed, and a daemon that runs for weeks would leak one per button press.
    autoreleasepool(|_| {
        let class = AnyClass::get(c"NSEvent").ok_or_else(|| {
            InputError::Rejected("the NSEvent class is missing: AppKit did not load".to_string())
        })?;

        // `data1` packs the key code in the high half and the up/down state in the low half.
        // The modifier flags carry the same state again — redundant, but both are what the
        // system's own media keys send, and applications check whichever they please.
        let state: isize = if down { 0xA } else { 0xB };
        let data1 = (isize::from(code) << 16) | (state << 8);
        let flags: usize = if down { 0xA00 } else { 0xB00 };

        // SAFETY: the selector and its argument types are those of
        // `+[NSEvent otherEventWithType:location:modifierFlags:timestamp:windowNumber:context:
        // subtype:data1:data2:]`, unchanged since Mac OS X 10.0. A nil context is documented as
        // the value to pass.
        let event: *mut AnyObject = unsafe {
            msg_send![
                class,
                otherEventWithType: SYSTEM_DEFINED,
                location: NSPoint { x: 0.0, y: 0.0 },
                modifierFlags: flags,
                timestamp: 0.0f64,
                windowNumber: 0isize,
                context: std::ptr::null_mut::<AnyObject>(),
                subtype: AUX_CONTROL_BUTTONS,
                data1: data1,
                data2: -1isize,
            ]
        };

        if event.is_null() {
            return Err(InputError::Rejected(
                "NSEvent refused to build a media key event".to_string(),
            ));
        }

        // SAFETY: `-[NSEvent CGEvent]` on an event that exists. The returned reference is owned
        // by the NSEvent and stays valid until the pool drains, which is after the post.
        let raw: *mut OpaqueCGEvent = unsafe { msg_send![event, CGEvent] };
        if raw.is_null() {
            return Err(InputError::Rejected(
                "the media key event has no CGEvent representation".to_string(),
            ));
        }

        // SAFETY: a live CGEventRef and a valid tap constant.
        unsafe { CGEventPost(HID_EVENT_TAP, raw) };
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // These build values without posting them, so they run on a CI runner with no desktop
    // session. Nothing that injects can be tested at all — see the module comment.

    #[test]
    fn a_combo_splits_into_flags_and_key_codes() {
        let mut flags = 0u64;
        let mut codes = Vec::new();
        for key in [Key::Meta, Key::Shift, Key::A] {
            match modifier_flag(key) {
                Some(flag) => flags |= flag,
                None => codes.push(key_code(key).expect("A maps")),
            }
        }
        assert_eq!(
            codes,
            vec![0x00],
            "only the non-modifier becomes a key event"
        );
        assert_eq!(flags, (1 << 20) | (1 << 17));
        assert_eq!(
            CGEventFlags::from_bits_truncate(flags),
            CGEventFlags::CGEventFlagCommand | CGEventFlags::CGEventFlagShift
        );
    }

    #[test]
    fn keys_macos_lacks_are_refused_with_an_explanation() {
        let error = resolve(Key::PrintScreen).expect_err("must be refused");
        assert!(matches!(error, InputError::Unsupported(_)));
        assert!(error.to_string().contains("Print Screen"));
    }

    #[test]
    fn text_is_chunked_without_splitting_characters() {
        let chunks = chunk_by_utf16("abcdef", 2);
        assert_eq!(chunks, vec!["ab", "cd", "ef"]);
        assert_eq!(chunks.concat(), "abcdef");
    }

    #[test]
    fn a_character_wider_than_the_limit_still_travels_whole() {
        // An emoji is two UTF-16 units. Splitting it would send half a surrogate pair, which
        // renders as a replacement character rather than failing.
        let chunks = chunk_by_utf16("a😀b", 2);
        assert!(chunks.iter().all(|c| c.chars().count() >= 1));
        assert_eq!(chunks.concat(), "a😀b");
        assert!(
            chunks.contains(&"😀"),
            "the emoji must stay in one piece: {chunks:?}"
        );
    }

    #[test]
    fn the_media_state_bits_are_packed_where_the_system_expects() {
        // NX_KEYTYPE_PLAY is 16; down is 0xA and up is 0xB in the low half.
        let code = media_code(MediaCommand::PlayPause).expect("play maps");
        let down = (isize::from(code) << 16) | (0xA << 8);
        let up = (isize::from(code) << 16) | (0xB << 8);
        assert_eq!(down, 0x10_0A00);
        assert_eq!(up, 0x10_0B00);
    }
}
