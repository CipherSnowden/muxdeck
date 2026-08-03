//! Engine error type.
//!
//! `thiserror` at the library boundary, `anyhow` only in `muxdeckd` — `CLAUDE.md` §Conventions.

use std::path::{Path, PathBuf};

use muxdeck_core::{ErrorCode, ErrorPayload};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, EngineError>;

#[derive(Debug, Error)]
pub enum EngineError {
    #[error("{context} ({path})")]
    Io {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("{context} ({path})")]
    Json {
        context: &'static str,
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },

    #[error("certificate generation failed: {0}")]
    Certificate(String),

    #[error("could not read from the operating system's entropy source: {0}")]
    Entropy(String),

    #[error("stored {what} is malformed; delete it or run `muxdeckd --reset-identity --yes`")]
    CorruptIdentity { what: &'static str },

    #[error("could not set owner-only permissions on {path}: {detail}")]
    Permissions { path: PathBuf, detail: String },

    /// A platform auto-start tool — `schtasks`, `launchctl`, `systemctl` — was absent or
    /// refused. Never reaches a socket: the `service` subcommands run from a terminal or from
    /// the desktop panel's installer step, so the detail is safe to show in full.
    #[error("{context}: {detail}")]
    Service {
        context: &'static str,
        detail: String,
    },

    /// A failure that maps directly onto a wire error and should be sent to the client.
    #[error("{}: {}", .0.code.as_str(), .0.message)]
    Wire(ErrorPayload),
}

impl EngineError {
    pub fn io(context: &'static str, path: impl AsRef<Path>, source: std::io::Error) -> Self {
        EngineError::Io {
            context,
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub fn json(context: &'static str, path: impl AsRef<Path>, source: serde_json::Error) -> Self {
        EngineError::Json {
            context,
            path: path.as_ref().to_path_buf(),
            source,
        }
    }

    pub fn service(context: &'static str, detail: impl Into<String>) -> Self {
        EngineError::Service {
            context,
            detail: detail.into(),
        }
    }

    pub fn wire(code: ErrorCode, message: impl Into<String>) -> Self {
        EngineError::Wire(ErrorPayload::new(code, message))
    }

    /// The payload to send back over the socket.
    ///
    /// Only [`EngineError::Wire`] carries a message meant for a client. Everything else is an
    /// internal fault: it becomes [`ErrorCode::Internal`] with a fixed string, because the
    /// underlying error can name config paths and other host details a paired phone has no
    /// business seeing. The real error is logged, not transmitted.
    pub fn to_payload(&self) -> ErrorPayload {
        match self {
            EngineError::Wire(payload) => payload.clone(),
            _ => ErrorPayload::new(ErrorCode::Internal, "internal engine error"),
        }
    }
}
