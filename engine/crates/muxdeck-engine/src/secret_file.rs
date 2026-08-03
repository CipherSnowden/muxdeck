//! Writing files that only their owner may read.
//!
//! `admin.token`'s file permissions are the entire boundary between "the user who owns this
//! desktop session" and "any other local user" — `docs/ARCHITECTURE.md` §5.4. Loopback alone
//! would admit a second logged-in user, so this module is doing real security work, not
//! housekeeping.

use std::fs;
use std::path::Path;

use crate::error::{EngineError, Result};

/// Writes `bytes` to `path`, replacing any existing file, readable only by the current user.
///
/// The permission change happens **before** the content is in place at its final name, so
/// there is no window in which a secret exists at a predictable path with default permissions.
pub fn write_secret(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");

    fs::write(&tmp, bytes).map_err(|e| EngineError::io("writing secret file", &tmp, e))?;

    if let Err(e) = restrict_to_owner(&tmp) {
        // Never leave a readable copy of a secret lying around because the tightening failed.
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }

    fs::rename(&tmp, path).map_err(|e| EngineError::io("installing secret file", path, e))?;
    Ok(())
}

/// Reads a file previously written by [`write_secret`].
pub fn read_secret(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| EngineError::io("reading secret file", path, e))
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).map_err(|e| {
        EngineError::Permissions {
            path: path.to_path_buf(),
            detail: e.to_string(),
        }
    })
}

/// Windows has no mode bits, so this sets an explicit DACL: inheritance removed, one entry
/// granting the current user full control, nobody else named.
///
/// `%APPDATA%` is already user-scoped by its inherited ACL, so in the common case this changes
/// nothing. It exists for the uncommon case — a profile whose permissions somebody widened.
/// Relying on inheritance would mean relying on that never having happened.
///
/// ponytail: shells out to `icacls` rather than calling `SetNamedSecurityInfoW`. That keeps the
/// crate free of `unsafe` for one first-run operation, at the cost of a process spawn and a
/// dependency on a tool that has shipped in System32 since Vista. Move to the Win32 call if this
/// ever needs to run anywhere but first-run setup.
#[cfg(windows)]
fn restrict_to_owner(path: &Path) -> Result<()> {
    use std::process::Command;

    let fail = |detail: String| EngineError::Permissions {
        path: path.to_path_buf(),
        detail,
    };

    let user = std::env::var("USERNAME").map_err(|_| {
        fail("USERNAME is not set, so the account to grant access to is unknown".to_string())
    })?;

    let output = Command::new("icacls")
        .arg(path)
        .arg("/inheritance:r")
        .arg("/grant:r")
        .arg(format!("{user}:(F)"))
        .arg("/q")
        .output()
        .map_err(|e| fail(format!("could not run icacls: {e}")))?;

    if !output.status.success() {
        return Err(fail(format!(
            "icacls exited with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_path(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("muxdeck-secret-tests");
        fs::create_dir_all(&dir).expect("temp dir");
        dir.join(name)
    }

    #[test]
    fn round_trips_content() {
        let path = temp_path("round_trip.bin");
        write_secret(&path, b"hunter2").expect("write");
        assert_eq!(read_secret(&path).expect("read"), b"hunter2");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn overwrites_an_existing_file() {
        let path = temp_path("overwrite.bin");
        write_secret(&path, b"first").expect("first write");
        write_secret(&path, b"second").expect("second write");
        assert_eq!(read_secret(&path).expect("read"), b"second");
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn leaves_no_temp_file_behind() {
        let path = temp_path("no_temp.bin");
        write_secret(&path, b"x").expect("write");
        assert!(!path.with_extension("tmp").exists());
        let _ = fs::remove_file(&path);
    }

    #[cfg(unix)]
    #[test]
    fn is_owner_only_on_unix() {
        use std::os::unix::fs::PermissionsExt;

        let path = temp_path("perms.bin");
        write_secret(&path, b"x").expect("write");
        let mode = fs::metadata(&path).expect("metadata").permissions().mode();
        assert_eq!(
            mode & 0o777,
            0o600,
            "secret files must be readable only by their owner"
        );
        let _ = fs::remove_file(&path);
    }
}
