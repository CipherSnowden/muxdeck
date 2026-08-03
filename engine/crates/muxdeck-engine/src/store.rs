//! Deck layouts on disk. `docs/PROTOCOL.md` §4.5 and §6.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use muxdeck_core::{
    Button, ButtonAction, ErrorCode, Grid, KnownOp, Op, Page, Position, Profile, ProfileSummary,
    Role,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::config::{read_json_or, write_json};
use crate::dispatch::{access_for, Access};
use crate::error::{EngineError, Result};

/// The profile a fresh install starts on.
pub const DEFAULT_PROFILE_ID: &str = "p_default";

#[derive(Debug, Serialize, Deserialize)]
struct StoreFile {
    profiles: BTreeMap<String, Profile>,
    active: String,
}

impl Default for StoreFile {
    fn default() -> Self {
        let profile = default_profile();
        let mut profiles = BTreeMap::new();
        profiles.insert(profile.id.clone(), profile);
        Self {
            profiles,
            active: DEFAULT_PROFILE_ID.to_string(),
        }
    }
}

/// Profiles, persisted as JSON.
pub struct Store {
    path: PathBuf,
    file: StoreFile,
}

impl Store {
    /// Loads the store, writing the default profile on first run.
    ///
    /// A fresh install landing on an empty grid would look broken, so the default is written
    /// immediately rather than materialised in memory — the panel should find something to edit.
    pub fn load(path: &Path) -> Result<Self> {
        let existed = path.exists();
        let file = read_json_or(path, "reading profiles.json", StoreFile::default)?;

        let store = Self {
            path: path.to_path_buf(),
            file,
        };
        if !existed {
            store.persist()?;
        }
        Ok(store)
    }

    pub fn get(&self, profile_id: &str) -> Option<&Profile> {
        self.file.profiles.get(profile_id)
    }

    /// The active profile, falling back to any profile if the active ID has gone stale.
    ///
    /// Falling back rather than erroring: a deck with *a* layout is far more useful than a deck
    /// refusing to start because a profile was deleted without the active pointer being moved.
    pub fn active(&self) -> Option<&Profile> {
        self.file
            .profiles
            .get(&self.file.active)
            .or_else(|| self.file.profiles.values().next())
    }

    pub fn active_id(&self) -> String {
        self.active()
            .map(|p| p.id.clone())
            .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string())
    }

    pub fn list(&self) -> Vec<ProfileSummary> {
        let active = self.active_id();
        self.file
            .profiles
            .values()
            .map(|p| ProfileSummary {
                id: p.id.clone(),
                name: p.name.clone(),
                active: p.id == active,
            })
            .collect()
    }

    /// Validates and stores a profile.
    ///
    /// `role` is the caller's, so a button cannot be *stored* carrying an action its author
    /// could not invoke. The engine re-checks at press time regardless — this stops the profile
    /// existing in the first place, which is a better error than a button that fails when
    /// pressed.
    pub fn set(&mut self, profile: Profile, role: Role) -> Result<()> {
        validate(&profile, role)?;
        self.file.profiles.insert(profile.id.clone(), profile);
        self.persist()
    }

    pub fn delete(&mut self, profile_id: &str) -> Result<()> {
        if !self.file.profiles.contains_key(profile_id) {
            return Err(EngineError::wire(ErrorCode::NotFound, "no such profile"));
        }
        if self.file.profiles.len() == 1 {
            return Err(EngineError::wire(
                ErrorCode::BadRequest,
                "this is the only profile; a deck with no layout has nothing to show",
            ));
        }

        self.file.profiles.remove(profile_id);
        if self.file.active == profile_id {
            // Point somewhere real rather than leaving a dangling active ID.
            self.file.active = self
                .file
                .profiles
                .keys()
                .next()
                .cloned()
                .unwrap_or_else(|| DEFAULT_PROFILE_ID.to_string());
        }
        self.persist()
    }

    pub fn activate(&mut self, profile_id: &str) -> Result<()> {
        if !self.file.profiles.contains_key(profile_id) {
            return Err(EngineError::wire(ErrorCode::NotFound, "no such profile"));
        }
        self.file.active = profile_id.to_string();
        self.persist()
    }

    fn persist(&self) -> Result<()> {
        write_json(&self.path, &self.file)
    }
}

/// Every rule from `docs/PROTOCOL.md` §4.5.
///
/// Each failure names what is wrong and where. **Never last-write-wins and never silent
/// coercion** — a layout that was quietly altered on the way in is worse than one refused, since
/// the user cannot see what happened.
fn validate(profile: &Profile, role: Role) -> Result<()> {
    if profile.pages.is_empty() {
        return Err(bad("a profile must have at least one page"));
    }
    if profile.grid.cols == 0 || profile.grid.rows == 0 {
        return Err(bad(
            "a profile's grid must have at least one column and one row",
        ));
    }

    let mut page_ids = std::collections::HashSet::new();
    let mut button_ids = std::collections::HashSet::new();

    for page in &profile.pages {
        if !page_ids.insert(&page.id) {
            return Err(bad(format!("duplicate page id '{}'", page.id)));
        }

        let mut occupied = std::collections::HashSet::new();
        for button in &page.buttons {
            // Button IDs are unique across the whole profile, not just within a page: an action
            // referring to a button by ID must not be ambiguous.
            if !button_ids.insert(&button.id) {
                return Err(bad(format!("duplicate button id '{}'", button.id)));
            }
            if button.pos.col >= profile.grid.cols || button.pos.row >= profile.grid.rows {
                return Err(bad(format!(
                    "button '{}' is at ({}, {}), outside the {}x{} grid",
                    button.id, button.pos.col, button.pos.row, profile.grid.cols, profile.grid.rows
                )));
            }
            if !occupied.insert((button.pos.col, button.pos.row)) {
                return Err(bad(format!(
                    "two buttons share position ({}, {}) on page '{}'",
                    button.pos.col, button.pos.row, page.id
                )));
            }

            check_action(button.on_tap.as_ref(), &button.id, "on_tap", role)?;
            check_action(
                button.on_long_press.as_ref(),
                &button.id,
                "on_long_press",
                role,
            )?;
        }
    }
    Ok(())
}

/// An embedded button action must be an op the caller could invoke itself.
fn check_action(
    action: Option<&ButtonAction>,
    button_id: &str,
    field: &str,
    role: Role,
) -> Result<()> {
    let Some(action) = action else { return Ok(()) };

    let Some(op) = action.op.known() else {
        return Err(bad(format!(
            "button '{button_id}' has an unknown op '{}' in {field}",
            action.op.as_str()
        )));
    };

    // Reuses the capability matrix rather than a second list, so the stored-profile check and
    // the press-time check can never disagree.
    let permitted = match access_for(op) {
        Access::Authenticated => true,
        Access::AdminOnly => role == Role::Admin,
        Access::PreAuth | Access::EventOnly => false,
    };

    if !permitted {
        return Err(bad(format!(
            "button '{button_id}' has op '{}' in {field}, which role '{}' may not invoke",
            op.as_str(),
            match role {
                Role::Deck => "deck",
                Role::Admin => "admin",
            }
        )));
    }
    Ok(())
}

fn bad(message: impl Into<String>) -> EngineError {
    EngineError::wire(ErrorCode::BadRequest, message)
}

/// The layout a fresh install starts with.
///
/// Everyday editing shortcuts plus a few window-management ones — chosen so a new user can press
/// something useful immediately rather than facing an empty grid and a manual.
fn default_profile() -> Profile {
    fn key(
        id: &str,
        col: u16,
        row: u16,
        label: &str,
        icon: &str,
        colour: &str,
        keys: &[&str],
    ) -> Button {
        Button {
            id: id.to_string(),
            pos: Position { col, row },
            label: label.to_string(),
            icon: icon.to_string(),
            color: colour.to_string(),
            haptic: muxdeck_core::Haptic::Light,
            on_tap: Some(ButtonAction {
                op: Op::of(KnownOp::InputKeyCombo),
                d: json!({ "keys": keys }),
            }),
            on_long_press: None,
        }
    }

    const BLUE: &str = "#2D6CDF";
    const PURPLE: &str = "#6B4FBB";
    const GREEN: &str = "#1F8A70";
    const RED: &str = "#B3422F";
    const GREY: &str = "#4A5568";

    Profile {
        id: DEFAULT_PROFILE_ID.to_string(),
        name: "Default".to_string(),
        grid: Grid { cols: 5, rows: 3 },
        pages: vec![Page {
            id: "pg_1".to_string(),
            name: "Main".to_string(),
            buttons: vec![
                key(
                    "b_copy",
                    0,
                    0,
                    "Copy",
                    "content_copy",
                    BLUE,
                    &["CONTROL", "C"],
                ),
                key(
                    "b_paste",
                    1,
                    0,
                    "Paste",
                    "content_paste",
                    BLUE,
                    &["CONTROL", "V"],
                ),
                key("b_cut", 2, 0, "Cut", "content_cut", BLUE, &["CONTROL", "X"]),
                key("b_undo", 3, 0, "Undo", "undo", BLUE, &["CONTROL", "Z"]),
                key("b_redo", 4, 0, "Redo", "redo", BLUE, &["CONTROL", "Y"]),
                key(
                    "b_all",
                    0,
                    1,
                    "Select all",
                    "select_all",
                    PURPLE,
                    &["CONTROL", "A"],
                ),
                key("b_save", 1, 1, "Save", "save", PURPLE, &["CONTROL", "S"]),
                key("b_find", 2, 1, "Find", "search", PURPLE, &["CONTROL", "F"]),
                key(
                    "b_switch",
                    3,
                    1,
                    "Switch app",
                    "swap_horiz",
                    PURPLE,
                    &["ALT", "TAB"],
                ),
                key(
                    "b_desktop",
                    4,
                    1,
                    "Desktop",
                    "desktop_windows",
                    PURPLE,
                    &["META", "D"],
                ),
                key(
                    "b_shot",
                    0,
                    2,
                    "Screenshot",
                    "photo_camera",
                    GREEN,
                    &["META", "SHIFT", "S"],
                ),
                key("b_lock", 1, 2, "Lock", "lock", GREEN, &["META", "L"]),
                key(
                    "b_tasks",
                    2,
                    2,
                    "Task view",
                    "grid_view",
                    GREEN,
                    &["META", "TAB"],
                ),
                key("b_close", 3, 2, "Close", "close", RED, &["ALT", "F4"]),
                key(
                    "b_escape",
                    4,
                    2,
                    "Escape",
                    "keyboard_return",
                    GREY,
                    &["ESCAPE"],
                ),
            ],
        }],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("muxdeck-store-tests");
        fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join(format!("{name}.json"));
        let _ = fs::remove_file(&path);
        path
    }

    fn simple(id: &str) -> Profile {
        Profile {
            id: id.to_string(),
            name: "Test".into(),
            grid: Grid { cols: 2, rows: 1 },
            pages: vec![Page {
                id: "pg_1".into(),
                name: "Main".into(),
                buttons: vec![],
            }],
        }
    }

    #[test]
    fn a_fresh_store_ships_a_usable_default() {
        // An empty grid on first run looks broken, so the default is written immediately.
        let path = temp_path("fresh");
        let store = Store::load(&path).expect("load");

        let active = store.active().expect("a default profile");
        assert_eq!(active.id, DEFAULT_PROFILE_ID);
        assert_eq!(active.grid.cols * active.grid.rows, 15);
        assert!(!active.pages[0].buttons.is_empty());
        assert!(
            path.exists(),
            "the default must be persisted, not just held in memory"
        );

        let _ = fs::remove_file(&path);
    }

    #[test]
    fn the_default_profile_passes_its_own_validation() {
        // Shipping a default the engine would reject on `profile.set` would be absurd, and is
        // exactly the kind of thing that survives until a user tries to edit it.
        validate(&default_profile(), Role::Admin).expect("the default must be valid");
    }

    #[test]
    fn profiles_survive_a_reload() {
        let path = temp_path("reload");
        let mut store = Store::load(&path).expect("load");
        store.set(simple("p_test"), Role::Admin).expect("set");

        let reloaded = Store::load(&path).expect("reload");
        assert!(reloaded.get("p_test").is_some());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn activation_moves_the_active_pointer() {
        let path = temp_path("activate");
        let mut store = Store::load(&path).expect("load");
        store.set(simple("p_other"), Role::Admin).expect("set");

        store.activate("p_other").expect("activate");
        assert_eq!(store.active_id(), "p_other");
        assert!(store.list().iter().any(|p| p.id == "p_other" && p.active));

        assert!(store.activate("p_nope").is_err());
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn deleting_the_active_profile_moves_the_pointer_somewhere_real() {
        let path = temp_path("delete_active");
        let mut store = Store::load(&path).expect("load");
        store.set(simple("p_other"), Role::Admin).expect("set");
        store.activate("p_other").expect("activate");

        store.delete("p_other").expect("delete");
        assert_ne!(store.active_id(), "p_other");
        assert!(
            store.active().is_some(),
            "a dangling active id would leave no layout"
        );
        let _ = fs::remove_file(&path);
    }

    #[test]
    fn the_last_profile_cannot_be_deleted() {
        // A deck with no layout has nothing to show, and no way back through the panel.
        let path = temp_path("delete_last");
        let mut store = Store::load(&path).expect("load");

        let err = store.delete(DEFAULT_PROFILE_ID).expect_err("must refuse");
        assert_eq!(err.to_payload().code, ErrorCode::BadRequest);
        let _ = fs::remove_file(&path);
    }

    // --- validation, one test per rule in docs/PROTOCOL.md §4.5 ------------

    #[test]
    fn an_empty_pages_array_is_rejected() {
        let mut profile = simple("p_x");
        profile.pages.clear();
        assert_eq!(
            validate(&profile, Role::Admin)
                .expect_err("no pages")
                .to_payload()
                .code,
            ErrorCode::BadRequest
        );
    }

    #[test]
    fn a_position_outside_the_grid_is_rejected() {
        let mut profile = simple("p_x");
        profile.pages[0].buttons.push(Button {
            id: "b_1".into(),
            pos: Position { col: 9, row: 0 },
            label: "Far".into(),
            icon: "circle".into(),
            color: "#000000".into(),
            haptic: muxdeck_core::Haptic::None,
            on_tap: None,
            on_long_press: None,
        });

        let err = validate(&profile, Role::Admin).expect_err("out of bounds");
        assert!(err.to_payload().message.contains("outside"));
    }

    #[test]
    fn two_buttons_in_the_same_cell_are_rejected() {
        let mut profile = simple("p_x");
        for id in ["b_1", "b_2"] {
            profile.pages[0].buttons.push(Button {
                id: id.into(),
                pos: Position { col: 0, row: 0 },
                label: id.into(),
                icon: "circle".into(),
                color: "#000000".into(),
                haptic: muxdeck_core::Haptic::None,
                on_tap: None,
                on_long_press: None,
            });
        }

        let err = validate(&profile, Role::Admin).expect_err("overlap");
        assert!(err.to_payload().message.contains("share position"));
    }

    #[test]
    fn a_duplicate_button_id_is_rejected_across_pages() {
        let mut profile = simple("p_x");
        profile.pages[0].buttons.push(Button {
            id: "b_dup".into(),
            pos: Position { col: 0, row: 0 },
            label: "One".into(),
            icon: "circle".into(),
            color: "#000000".into(),
            haptic: muxdeck_core::Haptic::None,
            on_tap: None,
            on_long_press: None,
        });
        profile.pages.push(Page {
            id: "pg_2".into(),
            name: "Second".into(),
            buttons: vec![Button {
                id: "b_dup".into(),
                pos: Position { col: 1, row: 0 },
                label: "Two".into(),
                icon: "circle".into(),
                color: "#000000".into(),
                haptic: muxdeck_core::Haptic::None,
                on_tap: None,
                on_long_press: None,
            }],
        });

        let err = validate(&profile, Role::Admin).expect_err("duplicate id");
        assert!(err.to_payload().message.contains("duplicate button id"));
    }

    #[test]
    fn a_duplicate_page_id_is_rejected() {
        let mut profile = simple("p_x");
        profile.pages.push(Page {
            id: "pg_1".into(),
            name: "Clash".into(),
            buttons: vec![],
        });

        let err = validate(&profile, Role::Admin).expect_err("duplicate page");
        assert!(err.to_payload().message.contains("duplicate page id"));
    }

    #[test]
    fn an_unknown_op_in_a_button_action_is_rejected() {
        let mut profile = simple("p_x");
        profile.pages[0].buttons.push(Button {
            id: "b_1".into(),
            pos: Position { col: 0, row: 0 },
            label: "Odd".into(),
            icon: "circle".into(),
            color: "#000000".into(),
            haptic: muxdeck_core::Haptic::None,
            on_tap: Some(ButtonAction {
                op: Op::parse("input.telepathy"),
                d: json!({}),
            }),
            on_long_press: None,
        });

        let err = validate(&profile, Role::Admin).expect_err("unknown op");
        assert!(err.to_payload().message.contains("unknown op"));
    }

    #[test]
    fn a_deck_may_not_store_an_admin_only_action() {
        // The escalation this blocks: a deck writing itself a button that opens a pairing
        // window, which would let it add devices on its own.
        let mut profile = simple("p_x");
        profile.pages[0].buttons.push(Button {
            id: "b_1".into(),
            pos: Position { col: 0, row: 0 },
            label: "Sneaky".into(),
            icon: "circle".into(),
            color: "#000000".into(),
            haptic: muxdeck_core::Haptic::None,
            on_tap: Some(ButtonAction {
                op: Op::of(KnownOp::PairBegin),
                d: json!({}),
            }),
            on_long_press: None,
        });

        assert!(
            validate(&profile, Role::Deck).is_err(),
            "a deck must not store pair.begin"
        );
        assert!(
            validate(&profile, Role::Admin).is_ok(),
            "the panel legitimately may, since it can invoke it"
        );
    }

    #[test]
    fn an_ordinary_input_action_is_accepted_for_either_role() {
        let mut profile = simple("p_x");
        profile.pages[0].buttons.push(Button {
            id: "b_1".into(),
            pos: Position { col: 0, row: 0 },
            label: "Copy".into(),
            icon: "content_copy".into(),
            color: "#2D6CDF".into(),
            haptic: muxdeck_core::Haptic::Light,
            on_tap: Some(ButtonAction {
                op: Op::of(KnownOp::InputKeyCombo),
                d: json!({ "keys": ["CONTROL", "C"] }),
            }),
            on_long_press: None,
        });

        validate(&profile, Role::Deck).expect("a deck may store an input action");
        validate(&profile, Role::Admin).expect("so may the panel");
    }
}
