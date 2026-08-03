//! Input injection: the platform seam.
//!
//! Everything OS-specific in MuxDeck lives behind the `InputBackend` trait defined here,
//! so engine logic is testable without touching a real desktop. Platform code goes behind
//! the trait or `#[cfg(target_os)]`, never as inline branching in business logic.
//!
//! Unlike the rest of the workspace this crate needs `unsafe` — `SendInput`,
//! `CGEventPost` and `/dev/uinput` ioctls are all raw FFI — so it deliberately does not
//! forbid it.
//!
//! The trait and `MockBackend` arrive with the Windows backend in milestone M3; macOS and
//! Linux follow in M7. See `docs/ENGINE.md` §4.
