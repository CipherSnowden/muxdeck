//! A backend that records what it was asked to do instead of doing it.
//!
//! Every engine test runs against this, so the suite never touches the real desktop — and so
//! the ordering guarantees in [`InputBackend::key_combo`] can be asserted directly rather than
//! inferred from whether something typed.

use std::sync::Mutex;
use std::time::Duration;

use muxdeck_core::{Key, MediaCommand};

use crate::{BackendCapabilities, InputBackend, InputError, MouseEvent};

/// One thing the backend was asked to do.
#[derive(Debug, Clone, PartialEq)]
pub enum Call {
    KeyDown(Key),
    KeyUp(Key),
    Text(String),
    Media(MediaCommand),
    Mouse(MouseEvent),
}

/// Records calls into a `Vec`. `#[cfg(test)]` only.
pub struct MockBackend {
    calls: Mutex<Vec<Call>>,
    /// When set, the key at this index fails to press. Used to prove that a combo still
    /// releases what it managed to press.
    fail_press_at: Option<usize>,
    capabilities: BackendCapabilities,
}

impl MockBackend {
    pub fn new() -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            fail_press_at: None,
            capabilities: BackendCapabilities {
                text_unicode: true,
                media_keys: true,
                mouse: true,
            },
        }
    }

    /// A backend whose `index`-th key press fails.
    pub fn failing_at(index: usize) -> Self {
        Self {
            fail_press_at: Some(index),
            ..Self::new()
        }
    }

    pub fn calls(&self) -> Vec<Call> {
        self.calls.lock().expect("mock lock poisoned").clone()
    }

    fn record(&self, call: Call) {
        self.calls.lock().expect("mock lock poisoned").push(call);
    }
}

impl Default for MockBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBackend for MockBackend {
    fn key_combo(&self, keys: &[Key], _hold: Duration) -> Result<(), InputError> {
        let mut pressed: Vec<Key> = Vec::with_capacity(keys.len());
        let mut failure = None;

        for (index, key) in keys.iter().enumerate() {
            if self.fail_press_at == Some(index) {
                failure = Some(InputError::Rejected(format!("mock refused {key:?}")));
                break;
            }
            self.record(Call::KeyDown(*key));
            pressed.push(*key);
        }

        // Release in reverse, whatever happened above. This mirrors what a real backend must
        // do: bailing out early would leave a modifier latched across the whole desktop.
        for key in pressed.iter().rev() {
            self.record(Call::KeyUp(*key));
        }

        match failure {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    fn text(&self, text: &str) -> Result<(), InputError> {
        self.record(Call::Text(text.to_string()));
        Ok(())
    }

    fn media(&self, command: MediaCommand) -> Result<(), InputError> {
        self.record(Call::Media(command));
        Ok(())
    }

    fn mouse(&self, event: MouseEvent) -> Result<(), InputError> {
        self.record(Call::Mouse(event));
        Ok(())
    }

    fn preflight(&self) -> Result<(), InputError> {
        Ok(())
    }

    fn capabilities(&self) -> BackendCapabilities {
        self.capabilities
    }

    fn name(&self) -> &'static str {
        "mock"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_combo_presses_in_order_and_releases_in_reverse() {
        let backend = MockBackend::new();
        backend
            .key_combo(&[Key::Control, Key::Shift, Key::Escape], Duration::ZERO)
            .expect("combo");

        assert_eq!(
            backend.calls(),
            vec![
                Call::KeyDown(Key::Control),
                Call::KeyDown(Key::Shift),
                Call::KeyDown(Key::Escape),
                Call::KeyUp(Key::Escape),
                Call::KeyUp(Key::Shift),
                Call::KeyUp(Key::Control),
            ]
        );
    }

    #[test]
    fn a_failure_partway_still_releases_what_was_pressed() {
        // The bug this prevents: a modifier left latched across the entire desktop, which
        // survives the process that caused it and needs a reboot or a manual keypress to fix.
        let backend = MockBackend::failing_at(2);
        let error = backend
            .key_combo(&[Key::Control, Key::Shift, Key::A], Duration::ZERO)
            .expect_err("the third press fails");

        assert!(matches!(error, InputError::Rejected(_)));
        assert_eq!(
            backend.calls(),
            vec![
                Call::KeyDown(Key::Control),
                Call::KeyDown(Key::Shift),
                Call::KeyUp(Key::Shift),
                Call::KeyUp(Key::Control),
            ],
            "everything pressed must be released, in reverse, even on failure"
        );
    }

    #[test]
    fn a_lone_modifier_is_a_valid_combo() {
        // `["META"]` alone is a real macro — it opens the Start menu.
        let backend = MockBackend::new();
        backend
            .key_combo(&[Key::Meta], Duration::ZERO)
            .expect("combo");
        assert_eq!(
            backend.calls(),
            vec![Call::KeyDown(Key::Meta), Call::KeyUp(Key::Meta)]
        );
    }
}
