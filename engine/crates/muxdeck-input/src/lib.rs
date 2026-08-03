//! Input injection: the platform seam.
//!
//! Everything OS-specific in MuxDeck lives behind the [`InputBackend`] trait, so engine
//! logic is testable without touching a real desktop. Platform code goes behind the trait
//! or `#[cfg(target_os)]`, never as inline branching in business logic.
//!
//! Unlike the rest of the workspace this crate needs `unsafe` — `SendInput`, `CGEventPost`
//! and `/dev/uinput` ioctls are all raw FFI — so it deliberately does not forbid it.
//!
//! See `docs/ENGINE.md` §4.

use std::time::Duration;

use muxdeck_core::{Key, MediaCommand, MouseButton};
use thiserror::Error;

pub mod keymap;
mod null;

#[cfg(windows)]
pub mod windows;

// Behind a feature rather than `#[cfg(test)]` so `muxdeck-engine`'s tests can reach it too —
// a `cfg(test)` module is visible only to its own crate. `cargo build` never enables it, so
// the mock stays out of the shipped daemon.
#[cfg(any(test, feature = "mock"))]
pub mod mock;

pub use null::NullBackend;

/// Why an injection failed.
///
/// Every variant carries a message the control panel can show verbatim, because the useful
/// ones are all remediation instructions rather than diagnostics — "add your user to the
/// `input` group" is the whole value of the error.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InputError {
    /// The OS refused the event.
    #[error("{0}")]
    Rejected(String),

    /// This backend cannot express the requested input at all.
    #[error("{0}")]
    Unsupported(String),

    /// Injection is not possible right now, and the message says how to fix it.
    #[error("{0}")]
    NotPermitted(String),
}

/// What a backend can actually do right now.
///
/// Reported to clients in the `Ready` payload so a deck can grey out buttons whose action is
/// unavailable rather than letting them fail at press time (`docs/PROTOCOL.md` §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackendCapabilities {
    /// `input.text` can inject arbitrary Unicode. False on Linux/uinput.
    pub text_unicode: bool,
    pub media_keys: bool,
    pub mouse: bool,
}

/// A mouse action, already resolved from the wire form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseEvent {
    /// Physical pixels, relative to the current cursor position.
    MoveRelative {
        dx: i32,
        dy: i32,
    },
    /// Normalised `0.0..=1.0` across the primary monitor, origin top-left.
    MoveAbsolute {
        x: f64,
        y: f64,
    },
    Click(MouseButton),
    Down(MouseButton),
    Up(MouseButton),
    /// Notches; `1.0` is one detent. Positive `dy` scrolls away from the user, positive `dx`
    /// scrolls right, matching every platform's own wheel convention.
    Scroll {
        dx: f64,
        dy: f64,
    },
}

/// The seam every platform implements.
///
/// Deliberately synchronous and dumb: no sequences, no delays, no timers. `input.key_sequence`
/// is **not** a trait method — the dispatch layer walks the steps itself, calling
/// [`InputBackend::key_combo`] on `spawn_blocking` and sleeping in async code between them.
/// That keeps the platform surface as small as possible: three backends implement five
/// methods, not six, and delays never block a worker thread.
pub trait InputBackend: Send + Sync {
    /// Presses the modifiers in listed order, taps the final non-modifier, holds for `hold`,
    /// then releases everything in reverse order.
    ///
    /// **Release must happen even if a press fails partway.** A latched modifier is a desktop
    /// nobody can use, and it outlives the process that caused it.
    fn key_combo(&self, keys: &[Key], hold: Duration) -> Result<(), InputError>;

    /// Types a literal string. Layout-independent where the platform allows it.
    fn text(&self, text: &str) -> Result<(), InputError>;

    fn media(&self, command: MediaCommand) -> Result<(), InputError>;

    fn mouse(&self, event: MouseEvent) -> Result<(), InputError>;

    /// Whether this backend can inject at all right now.
    ///
    /// This is what lets the control panel say *"grant Accessibility permission"* instead of
    /// buttons silently doing nothing. Called at startup and surfaced through `settings.get`.
    fn preflight(&self) -> Result<(), InputError>;

    /// Per-feature availability, for the `capabilities` block of the `Ready` payload.
    fn capabilities(&self) -> BackendCapabilities;

    /// A short name for logs and the dashboard, e.g. `"sendinput"`.
    fn name(&self) -> &'static str;
}

/// The backend for this platform.
///
/// Returns [`NullBackend`] where no implementation exists yet, so the engine still builds and
/// runs everywhere — `preflight` then fails with a message saying exactly that, rather than
/// the daemon refusing to start.
pub fn platform_backend() -> Box<dyn InputBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(NullBackend::new(
            "input injection for this platform arrives in milestone M7",
        ))
    }
}
