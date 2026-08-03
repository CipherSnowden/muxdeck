//! A backend for platforms that do not have one yet.
//!
//! Keeps the daemon startable everywhere: rather than refusing to run, it starts, reports
//! `preflight` failure with a message naming the reason, and advertises no capabilities — so
//! a connected deck greys those buttons out instead of watching them silently do nothing.

use std::time::Duration;

use muxdeck_core::{Key, MediaCommand};

use crate::{BackendCapabilities, InputBackend, InputError, MouseEvent};

pub struct NullBackend {
    reason: &'static str,
}

impl NullBackend {
    pub fn new(reason: &'static str) -> Self {
        Self { reason }
    }

    fn refuse<T>(&self) -> Result<T, InputError> {
        Err(InputError::Unsupported(self.reason.to_string()))
    }
}

impl InputBackend for NullBackend {
    fn key_combo(&self, _keys: &[Key], _hold: Duration) -> Result<(), InputError> {
        self.refuse()
    }

    fn text(&self, _text: &str) -> Result<(), InputError> {
        self.refuse()
    }

    fn media(&self, _command: MediaCommand) -> Result<(), InputError> {
        self.refuse()
    }

    fn mouse(&self, _event: MouseEvent) -> Result<(), InputError> {
        self.refuse()
    }

    fn preflight(&self) -> Result<(), InputError> {
        Err(InputError::NotPermitted(self.reason.to_string()))
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            text_unicode: false,
            media_keys: false,
            mouse: false,
        }
    }

    fn name(&self) -> &'static str {
        "none"
    }
}
