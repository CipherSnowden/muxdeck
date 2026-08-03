//! mDNS service advertisement. `docs/ARCHITECTURE.md` §6.
//!
//! Clients browse for `_muxdeck._tcp.local.` to populate their "hosts found" list. Because the
//! fingerprint travels in the TXT record, a previously-paired client can confirm it is talking
//! to the same host even if the IP changed — which is the whole reason an IP change does not
//! require re-pairing.

use mdns_sd::{ServiceDaemon, ServiceInfo};
use tracing::{info, warn};

use crate::error::{EngineError, Result};

/// The service type clients browse for. Must match `NSBonjourServices` in the iOS
/// `Info.plist` exactly, or discovery on iOS silently returns nothing.
pub const SERVICE_TYPE: &str = "_muxdeck._tcp.local.";

/// A live advertisement. Dropping it de-advertises.
pub struct Advertisement {
    daemon: ServiceDaemon,
    fullname: String,
}

impl Advertisement {
    /// Starts advertising this host.
    ///
    /// `v` is a comma-separated list of supported protocol majors, not a single value, because
    /// one value cannot express "this host speaks both" during a transition release.
    pub fn start(instance_name: &str, host_id: &str, fingerprint: &str, port: u16) -> Result<Self> {
        let daemon = ServiceDaemon::new()
            .map_err(|e| EngineError::Certificate(format!("could not start mDNS: {e}")))?;

        // The instance name is what a human sees in a browser; the hostname must end in
        // `.local.` for mDNS to resolve it.
        let hostname = format!("{}.local.", sanitise(instance_name));

        let properties = [
            ("v", muxdeck_core::PROTOCOL_VERSION.to_string()),
            ("id", host_id.to_string()),
            ("name", instance_name.to_string()),
            ("fp", fingerprint.to_string()),
        ];

        let service = ServiceInfo::new(
            SERVICE_TYPE,
            &sanitise(instance_name),
            &hostname,
            (),
            port,
            &properties[..],
        )
        .map_err(|e| EngineError::Certificate(format!("could not build the mDNS record: {e}")))?
        .enable_addr_auto();

        let fullname = service.get_fullname().to_string();
        daemon
            .register(service)
            .map_err(|e| EngineError::Certificate(format!("could not advertise over mDNS: {e}")))?;

        info!(service = %fullname, port, "advertising over mDNS");
        Ok(Self { daemon, fullname })
    }

    /// De-advertises and stops the daemon.
    ///
    /// Called explicitly on shutdown so clients see the host disappear promptly rather than
    /// waiting for the record to age out.
    pub fn stop(self) {
        match self.daemon.unregister(&self.fullname) {
            Ok(_) => info!(service = %self.fullname, "withdrew the mDNS advertisement"),
            Err(e) => warn!(error = %e, "could not withdraw the mDNS advertisement"),
        }
        let _ = self.daemon.shutdown();
    }
}

/// mDNS instance names may not contain dots — they are label separators.
fn sanitise(name: &str) -> String {
    let cleaned: String = name
        .chars()
        .map(|c| if c == '.' { '-' } else { c })
        .filter(|c| !c.is_control())
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "MuxDeck".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dots_become_dashes_because_they_are_label_separators() {
        assert_eq!(sanitise("my.host.name"), "my-host-name");
    }

    #[test]
    fn an_empty_name_falls_back_rather_than_producing_an_invalid_record() {
        assert_eq!(sanitise("   "), "MuxDeck");
        assert_eq!(sanitise(""), "MuxDeck");
    }

    #[test]
    fn an_ordinary_name_is_left_alone() {
        assert_eq!(sanitise("ENIGMA-ENTROPY"), "ENIGMA-ENTROPY");
    }
}
