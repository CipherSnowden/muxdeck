//! The uinput virtual device. `docs/ENGINE.md` §4.3.
//!
//! uinput rather than XTEST because it is kernel-level, so it works under **both X11 and
//! Wayland** — XTEST is an X11 extension and does nothing on a Wayland session, which is now the
//! default on most desktops.

use std::sync::Mutex;
use std::time::Duration;

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, EventType, InputEvent, KeyCode};
use muxdeck_core::{Key, MediaCommand};

use super::keymap::{all_codes, key_code, media_code};
use crate::{BackendCapabilities, InputBackend, InputError, MouseEvent};

/// How long to wait after creating the device before sending the first event.
///
/// **Not optional.** udev has to notice the new device and the compositor has to open it; events
/// sent before that are accepted by the kernel and delivered to nobody. The symptom is that the
/// first keystroke after startup silently vanishes, which is a classic uinput bug and miserable
/// to diagnose because everything reports success.
const SETTLE: Duration = Duration::from_millis(100);

/// The name the device advertises. Appears in `/proc/bus/input/devices` and in logs.
const DEVICE_NAME: &str = "MuxDeck virtual keyboard";

pub struct LinuxBackend {
    /// `None` when the device could not be created — `preflight` explains why.
    ///
    /// A `Mutex` because `evdev` needs `&mut` to emit and the trait takes `&self`; injection is
    /// already serialised onto a blocking thread by the dispatch layer, so this is never
    /// contended in practice.
    device: Mutex<Option<VirtualDevice>>,
    /// Why creation failed, kept so every later call can explain itself rather than just
    /// refusing.
    failure: Option<InputError>,
}

impl LinuxBackend {
    pub fn new() -> Self {
        match Self::create() {
            Ok(device) => {
                // Settle once here rather than before each injection, so the cost is paid at
                // startup instead of on the first keypress a user makes.
                std::thread::sleep(SETTLE);
                Self {
                    device: Mutex::new(Some(device)),
                    failure: None,
                }
            }
            Err(error) => Self {
                device: Mutex::new(None),
                failure: Some(error),
            },
        }
    }

    fn create() -> Result<VirtualDevice, InputError> {
        let mut keys = AttributeSet::<KeyCode>::new();
        // Every code the backend may ever send must be declared now: a key not registered at
        // creation is silently dropped rather than rejected.
        for code in all_codes() {
            keys.insert(KeyCode(code));
        }

        VirtualDevice::builder()
            .map_err(describe_open_failure)?
            .name(DEVICE_NAME)
            .with_keys(&keys)
            .map_err(|e| InputError::Rejected(format!("could not declare keys: {e}")))?
            .build()
            .map_err(|e| InputError::Rejected(format!("could not create the virtual device: {e}")))
    }

    /// Emits a batch and terminates it with `SYN_REPORT`.
    ///
    /// **The synchronisation event is what makes the batch visible.** Without it the kernel
    /// holds the events and nothing is ever delivered — again with no error anywhere.
    fn emit(&self, events: &[InputEvent]) -> Result<(), InputError> {
        let mut guard = self.device.lock().map_err(|_| {
            InputError::Rejected("the virtual device lock was poisoned".to_string())
        })?;

        let device = guard.as_mut().ok_or_else(|| {
            self.failure.clone().unwrap_or_else(|| {
                InputError::NotPermitted("the virtual keyboard is unavailable".to_string())
            })
        })?;

        // `evdev` appends the SYN_REPORT for us on `emit`, which is why events are passed as one
        // batch rather than one at a time.
        device
            .emit(events)
            .map_err(|e| InputError::Rejected(format!("could not send input events: {e}")))
    }

    fn key_event(code: u16, pressed: bool) -> InputEvent {
        // `InputEvent::new` takes the raw event type, not the enum.
        InputEvent::new(EventType::KEY.0, code, if pressed { 1 } else { 0 })
    }
}

impl Default for LinuxBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl InputBackend for LinuxBackend {
    fn key_combo(&self, keys: &[Key], hold: Duration) -> Result<(), InputError> {
        if keys.is_empty() {
            return Ok(());
        }

        let press: Vec<InputEvent> = keys
            .iter()
            .map(|k| Self::key_event(key_code(*k), true))
            .collect();
        let release: Vec<InputEvent> = keys
            .iter()
            .rev()
            .map(|k| Self::key_event(key_code(*k), false))
            .collect();

        let press_result = self.emit(&press);

        if !hold.is_zero() {
            std::thread::sleep(hold);
        }

        // Released unconditionally: a modifier left held down affects every window on the
        // desktop and outlives this process.
        let release_result = self.emit(&release);

        press_result.and(release_result)
    }

    /// Not supported, and deliberately so.
    ///
    /// uinput speaks keycodes, not characters — there is no equivalent of Windows'
    /// `KEYEVENTF_UNICODE`. Producing arbitrary text would mean reading the user's active layout,
    /// working out which keycode-plus-modifier combination yields each character, and failing
    /// anyway for anything not on their keyboard.
    ///
    /// Rather than half-work, this refuses and `capabilities.text_unicode` reports `false`, so a
    /// deck greys those buttons out instead of letting them fail at press time
    /// (`docs/PROTOCOL.md` §4.1). This is the known Linux limitation `docs/ENGINE.md` §4.3
    /// documents.
    fn text(&self, _text: &str) -> Result<(), InputError> {
        Err(InputError::Unsupported(
            "typing text is not supported on Linux: the kernel input layer works in key codes, \
             not characters, so what a key produces depends on the active layout. Use a key \
             combination instead."
                .to_string(),
        ))
    }

    fn media(&self, command: MediaCommand) -> Result<(), InputError> {
        let code = media_code(command);
        self.emit(&[Self::key_event(code, true), Self::key_event(code, false)])
    }

    /// Not supported by this device.
    ///
    /// A uinput device declares what it can produce at creation, and this one is a keyboard.
    /// Mouse support would mean a second device with `EV_REL`/`EV_ABS` axes and button
    /// capabilities — worth adding, but a separate piece of work rather than something to bolt
    /// onto a keyboard.
    fn mouse(&self, _event: MouseEvent) -> Result<(), InputError> {
        Err(InputError::Unsupported(
            "mouse control is not available on Linux yet".to_string(),
        ))
    }

    fn preflight(&self) -> Result<(), InputError> {
        match &self.failure {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }

    fn capabilities(&self) -> BackendCapabilities {
        BackendCapabilities {
            // See `text` — a real limitation, reported honestly so the deck can grey out.
            text_unicode: false,
            media_keys: true,
            mouse: false,
        }
    }

    fn name(&self) -> &'static str {
        "uinput"
    }
}

/// Turns a failure to open `/dev/uinput` into something the user can act on.
///
/// The permission case is the overwhelmingly common one, and the message is the entire value of
/// the error — the control panel shows it verbatim (`docs/SERVER.md` §6).
fn describe_open_failure(error: std::io::Error) -> InputError {
    use std::io::ErrorKind;

    match error.kind() {
        ErrorKind::PermissionDenied => InputError::NotPermitted(
            "MuxDeck cannot open /dev/uinput. Add your user to the 'input' group and log out \
             and back in:\n\n    sudo usermod -aG input $USER\n\nIf that is not enough, install \
             the udev rule printed by `muxdeckd service install`."
                .to_string(),
        ),
        ErrorKind::NotFound => InputError::NotPermitted(
            "/dev/uinput does not exist. Load the kernel module with `sudo modprobe uinput`, \
             and add 'uinput' to /etc/modules-load.d/ to have it loaded at boot."
                .to_string(),
        ),
        _ => InputError::NotPermitted(format!("could not open /dev/uinput: {error}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Creating a real device needs write access to /dev/uinput, which an ordinary CI runner does
    // not have. Run explicitly with:
    //   cargo test -p muxdeck-input -- --ignored --test-threads=1

    #[test]
    #[ignore = "needs write access to /dev/uinput"]
    fn a_virtual_device_can_be_created_and_used() {
        let backend = LinuxBackend::new();
        backend
            .preflight()
            .expect("preflight must pass when /dev/uinput is writable");

        backend
            .key_combo(&[Key::Control, Key::A], Duration::ZERO)
            .expect("a combo must be accepted");
        backend
            .media(MediaCommand::PlayPause)
            .expect("a media key must be accepted");
    }

    /// Injects and reads the events back off the device node the kernel created.
    ///
    /// **This is the assertion that matters.** A headless machine has no compositor consuming
    /// keystrokes, so "the call returned `Ok`" proves only that the kernel accepted the write —
    /// not that the right codes went out, in the right order, with the right press and release
    /// values. Reading the stream back is the only way to check the bytes rather than the
    /// return value.
    ///
    /// Needs more than the other two: as well as writing to `/dev/uinput` it has to *read* the
    /// `/dev/input/event*` node the kernel creates, which is `root:input 0640` and does not
    /// exist to be chmodded in advance. In practice that means running as root.
    #[test]
    #[ignore = "needs write access to /dev/uinput and read access to the node it creates"]
    fn a_combo_emits_the_right_codes_in_the_right_order() {
        use std::time::Instant;

        use evdev::Device;

        let backend = LinuxBackend::new();
        backend.preflight().expect("preflight");

        // The events a uinput device *produces* come out of the `/dev/input/event*` node the
        // kernel created for it. Reading the `/dev/uinput` handle instead returns the events
        // sent **to** the device — force-feedback uploads and LED changes — so it simply blocks
        // for ever on a device that has neither.
        let node = {
            let mut guard = backend.device.lock().expect("lock");
            let device = guard.as_mut().expect("device");
            device
                .enumerate_dev_nodes_blocking()
                .expect("enumerate the device nodes")
                .next()
                .expect("the kernel created no device node")
                .expect("device node path")
        };

        // Opened before anything is emitted: the kernel buffers per open file description, so
        // events sent before the open are not there to read afterwards.
        let mut reader = Device::open(&node).expect("open the device node");
        reader
            .set_nonblocking(true)
            .expect("make the reader non-blocking");

        backend
            .key_combo(&[Key::Control, Key::A], Duration::ZERO)
            .expect("a combo must be accepted");

        let mut events = Vec::new();
        let deadline = Instant::now() + Duration::from_secs(2);
        while events.len() < 4 && Instant::now() < deadline {
            match reader.fetch_events() {
                Ok(batch) => events.extend(
                    batch
                        .filter(|event| event.event_type() == EventType::KEY)
                        .map(|event| (event.code(), event.value())),
                ),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(10))
                }
                Err(error) => panic!("reading {}: {error}", node.display()),
            }
        }

        let ctrl = key_code(Key::Control);
        let a = key_code(Key::A);

        assert_eq!(
            events,
            vec![(ctrl, 1), (a, 1), (a, 0), (ctrl, 0)],
            "the modifier must be pressed first and released last, or the shortcut never fires"
        );
    }

    #[test]
    #[ignore = "needs write access to /dev/uinput"]
    fn text_is_refused_rather_than_half_working() {
        let backend = LinuxBackend::new();
        let error = backend.text("muxdeck").expect_err("text must be refused");
        assert!(matches!(error, InputError::Unsupported(_)));
        assert!(!backend.capabilities().text_unicode);
    }
}
