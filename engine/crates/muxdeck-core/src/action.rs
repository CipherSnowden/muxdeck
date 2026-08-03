//! Named shell actions. `docs/PROTOCOL.md` §4.4 and §6.

use serde::{Deserialize, Serialize};

/// A named, pre-defined action.
///
/// `command` and `args` are separate fields, never a single string, so the engine can
/// never pass anything to a shell interpreter. Shell execution is disabled by default —
/// see `docs/ARCHITECTURE.md` §5.5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    pub id: String,
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    /// Working directory, or `null` to inherit the engine's.
    pub cwd: Option<String>,
}

/// `action.run`. The client sends an action *name*, never a command string.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRunRequest {
    pub action_id: String,
}

/// `action.list` response.
///
/// Full [`Action`] objects for both roles: a deck can already execute every defined
/// action, so withholding the command string it will run buys nothing and would mean two
/// response shapes for one op. When shell actions are disabled this is empty rather than
/// an error, so a client can call it unconditionally at startup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionListResponse {
    pub actions: Vec<Action>,
}

/// `action.set` — admin only. Creates or replaces by `id`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionSetRequest {
    pub action: Action,
}

/// `action.delete` — admin only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionDeleteRequest {
    pub action_id: String,
}
