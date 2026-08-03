//! Linux input injection through `/dev/uinput`. `docs/ENGINE.md` §4.3.
//!
//! The keycode table is plain data and compiles everywhere, so it is tested on the development
//! machine like any other module. Only [`backend`] touches the kernel, and only that is gated.

pub mod keymap;

#[cfg(target_os = "linux")]
pub mod backend;

#[cfg(target_os = "linux")]
pub use backend::LinuxBackend;
