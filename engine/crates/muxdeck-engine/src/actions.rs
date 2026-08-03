//! Named shell actions. `docs/PROTOCOL.md` §4.4, `docs/ARCHITECTURE.md` §5.5.
//!
//! # Why this is the most locked-down module in the engine
//!
//! Every other op does something the protocol fully describes: press this key, store this
//! layout. This one runs arbitrary programs on the user's desktop at the request of a device on
//! the network, which makes it the single largest footgun in a project like this.
//!
//! Three things contain it, and none of them is optional:
//!
//! 1. **Off by default.** `shell_actions_enabled` starts `false` and only the control panel can
//!    turn it on, with a warning. Both running *and defining* an action require it.
//! 2. **The client sends a name, never a command.** A device can only invoke something a human
//!    already defined on the desktop; there is no path from a socket to an arbitrary string.
//! 3. **No shell, ever.** `command` and `args` are separate fields and go straight to the OS
//!    process API. Nothing is passed to `sh -c` or `cmd /c`, so an argument containing
//!    `; rm -rf ~` is an argument, not a second command.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;

use muxdeck_core::{Action, ErrorCode};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::config::{read_json_or, write_json};
use crate::error::{EngineError, Result};

#[derive(Debug, Default, Serialize, Deserialize)]
struct ActionsFile {
    actions: BTreeMap<String, Action>,
}

/// Named actions, persisted as JSON.
///
/// Unlike [`crate::store::Store`] this ships **no defaults**. A fresh install with a
/// pre-defined command that runs on someone's machine is exactly the surprise this feature must
/// not have.
pub struct ActionStore {
    path: PathBuf,
    file: ActionsFile,
}

impl ActionStore {
    pub fn load(path: &Path) -> Result<Self> {
        let file = read_json_or(path, "reading actions.json", ActionsFile::default)?;
        Ok(Self {
            path: path.to_path_buf(),
            file,
        })
    }

    pub fn get(&self, action_id: &str) -> Option<&Action> {
        self.file.actions.get(action_id)
    }

    pub fn list(&self) -> Vec<Action> {
        self.file.actions.values().cloned().collect()
    }

    pub fn set(&mut self, action: Action) -> Result<()> {
        validate(&action)?;
        self.file.actions.insert(action.id.clone(), action);
        self.persist()
    }

    pub fn delete(&mut self, action_id: &str) -> Result<()> {
        if self.file.actions.remove(action_id).is_none() {
            return Err(EngineError::wire(ErrorCode::NotFound, "no such action"));
        }
        // Profiles referencing this action are deliberately left alone (`docs/PROTOCOL.md`
        // §4.4): the engine re-checks at press time, so a stale button fails with NOT_FOUND
        // rather than the delete having to rewrite every layout that might mention it.
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        write_json(&self.path, &self.file)
    }
}

fn validate(action: &Action) -> Result<()> {
    if action.id.trim().is_empty() {
        return Err(EngineError::wire(
            ErrorCode::BadRequest,
            "an action needs an id",
        ));
    }
    if action.command.trim().is_empty() {
        return Err(EngineError::wire(
            ErrorCode::BadRequest,
            "an action needs a command to run",
        ));
    }
    Ok(())
}

/// Spawns an action and returns as soon as the process exists.
///
/// **Deliberately not awaited.** A deck button that switches an OBS scene should feel instant,
/// and the exit code arrives long after the user has moved on; blocking the socket until the
/// program finished would make every slow action look like a dropped press. The child is still
/// reaped — the spawned task waits on it, which both avoids zombies on Unix and puts a failing
/// command in the log where the panel's log tail will show it.
///
/// The caller is responsible for the `shell_actions_enabled` check; this function does not know
/// about settings.
pub fn spawn(action: &Action) -> Result<()> {
    // `tokio::process` rather than `std::process` purely for the reaping: a std child that is
    // never waited on becomes a zombie, and a deck can press a button hundreds of times a day.
    let mut command = tokio::process::Command::new(&action.command);
    command.args(&action.args);
    if let Some(cwd) = &action.cwd {
        command.current_dir(cwd);
    }

    // A daemon has no terminal to inherit. Without this the child would share the engine's
    // stdio and could write over the log output, or block for ever on a read from a stdin
    // nobody is typing into.
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());

    let mut child = command.spawn().map_err(|error| {
        EngineError::wire(
            ErrorCode::InjectionFailed,
            format!(
                "could not run '{}': {error}. Check the command exists and is on PATH.",
                action.command
            ),
        )
    })?;

    let id = action.id.clone();
    let name = action.command.clone();
    tokio::spawn(async move {
        match child.wait().await {
            Ok(status) if status.success() => info!(action_id = %id, "action finished"),
            Ok(status) => warn!(action_id = %id, command = %name, %status, "action failed"),
            Err(error) => warn!(action_id = %id, command = %name, %error, "action was lost"),
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("muxdeck-action-tests");
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(format!("{name}.json"));
        let _ = fs::remove_file(&path);
        path
    }

    fn action(id: &str) -> Action {
        Action {
            id: id.to_string(),
            name: "Test".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            cwd: None,
        }
    }

    #[test]
    fn a_fresh_install_defines_no_actions() {
        // Shipping a default action would mean a command nobody asked for is runnable by any
        // paired device the moment the feature is switched on.
        let store = ActionStore::load(&temp_path("fresh")).expect("load");
        assert!(store.list().is_empty());
    }

    #[test]
    fn setting_then_getting_round_trips_through_the_file() {
        let path = temp_path("round-trip");
        let mut store = ActionStore::load(&path).expect("load");
        store.set(action("a_one")).expect("set");

        let reloaded = ActionStore::load(&path).expect("reload");
        assert_eq!(
            reloaded.get("a_one").map(|a| a.command.as_str()),
            Some("echo")
        );
    }

    #[test]
    fn set_replaces_by_id_rather_than_adding_a_duplicate() {
        let mut store = ActionStore::load(&temp_path("replace")).expect("load");
        store.set(action("a_one")).expect("set");

        let mut replacement = action("a_one");
        replacement.name = "Renamed".to_string();
        store.set(replacement).expect("replace");

        assert_eq!(store.list().len(), 1);
        assert_eq!(store.get("a_one").map(|a| a.name.as_str()), Some("Renamed"));
    }

    #[test]
    fn an_action_with_no_command_is_refused() {
        let mut store = ActionStore::load(&temp_path("no-command")).expect("load");
        let mut empty = action("a_one");
        empty.command = "   ".to_string();

        let error = store.set(empty).expect_err("must be refused");
        assert_eq!(error.to_payload().code, ErrorCode::BadRequest);
    }

    #[test]
    fn deleting_something_that_is_not_there_is_not_found() {
        let mut store = ActionStore::load(&temp_path("missing")).expect("load");
        let error = store.delete("a_missing").expect_err("must be refused");
        assert_eq!(error.to_payload().code, ErrorCode::NotFound);
    }

    #[test]
    fn arguments_stay_arguments() {
        // The whole point of separate `command` and `args`: this string is one argument, not a
        // command separator, because nothing goes near a shell interpreter.
        let mut store = ActionStore::load(&temp_path("quoting")).expect("load");
        let mut dangerous = action("a_one");
        dangerous.args = vec!["; rm -rf ~".to_string()];
        store.set(dangerous).expect("set");

        assert_eq!(
            store.get("a_one").map(|a| a.args.as_slice()),
            Some(["; rm -rf ~".to_string()].as_slice())
        );
    }
}
