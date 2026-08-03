//! macOS input injection through Quartz Event Services. `docs/ENGINE.md` §4.2.
//!
//! The virtual key table is plain data and compiles everywhere, so it is tested on the
//! development machine like any other module. Only [`backend`] touches the frameworks, and only
//! that is gated.

pub mod keymap;

#[cfg(target_os = "macos")]
pub mod backend;

#[cfg(target_os = "macos")]
pub use backend::MacosBackend;
