//! Profile registry actions (UX.md PR3) and the one-time migration of
//! per-profile `keyboard_shortcut` strings into registry bindings (MD3).
//!
//! A profile is opened, split with, or applied through four action ids:
//!
//! | Action id | Effect |
//! |---|---|
//! | `open_profile:<id>` | new tab from the profile |
//! | `open_profile_window:<id>` | new window whose first tab runs the profile |
//! | `split_profile:<id>:<dir>` | split the focused pane (`right` / `down`) running the profile |
//! | `set_tab_profile:<id>` | apply the profile to the active tab |
//!
//! They are ordinary registry actions: a chord bound to one fires through
//! `execute_keybinding_action`, is conflict-checked with every other
//! binding, and shows its live chord in the palette and the launcher.
//!
//! The per-profile shortcut layer that used to match `keyboard_shortcut`
//! strings against key events (B59) is gone. Existing strings migrate once:
//! [`migrate_profile_shortcuts`] turns each into an `open_profile:<id>`
//! binding (keeping the user's chord) and clears the profile field, so the
//! migration cannot run twice.

use crate::config::{Config, KeyBinding};
use crate::profile::{Profile, ProfileId};

/// Where a profile split puts the new pane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileSplit {
    /// Side by side, new pane on the right.
    Right,
    /// Stacked, new pane below.
    Down,
}

impl ProfileSplit {
    fn as_str(self) -> &'static str {
        match self {
            ProfileSplit::Right => "right",
            ProfileSplit::Down => "down",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        match s {
            "right" => Some(ProfileSplit::Right),
            "down" => Some(ProfileSplit::Down),
            _ => None,
        }
    }
}

/// One parsed profile action.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ProfileAction {
    OpenTab(ProfileId),
    OpenWindow(ProfileId),
    Split(ProfileId, ProfileSplit),
    SetTabProfile(ProfileId),
}

const OPEN_TAB: &str = "open_profile:";
const OPEN_WINDOW: &str = "open_profile_window:";
const SPLIT: &str = "split_profile:";
const SET_TAB: &str = "set_tab_profile:";

/// Every prefix a profile action id starts with.
pub(crate) const PROFILE_ACTION_PREFIXES: &[&str] = &[OPEN_TAB, OPEN_WINDOW, SPLIT, SET_TAB];

impl ProfileAction {
    /// The registry action id.
    pub(crate) fn id(self) -> String {
        match self {
            ProfileAction::OpenTab(id) => format!("{OPEN_TAB}{id}"),
            ProfileAction::OpenWindow(id) => format!("{OPEN_WINDOW}{id}"),
            ProfileAction::Split(id, dir) => format!("{SPLIT}{id}:{}", dir.as_str()),
            ProfileAction::SetTabProfile(id) => format!("{SET_TAB}{id}"),
        }
    }

    /// Parse a registry action id. `None` for anything that is not a
    /// well-formed profile action (unknown prefix, bad UUID, bad direction).
    pub(crate) fn parse(action: &str) -> Option<Self> {
        // `open_profile_window:` must be tried before `open_profile:` — the
        // former does not start with the latter (`_` vs `:`), but keep the
        // order explicit so a future prefix that does cannot shadow it.
        if let Some(rest) = action.strip_prefix(OPEN_WINDOW) {
            return rest.parse().ok().map(ProfileAction::OpenWindow);
        }
        if let Some(rest) = action.strip_prefix(OPEN_TAB) {
            return rest.parse().ok().map(ProfileAction::OpenTab);
        }
        if let Some(rest) = action.strip_prefix(SET_TAB) {
            return rest.parse().ok().map(ProfileAction::SetTabProfile);
        }
        if let Some(rest) = action.strip_prefix(SPLIT) {
            let (id, dir) = rest.rsplit_once(':')?;
            return Some(ProfileAction::Split(
                id.parse().ok()?,
                ProfileSplit::parse(dir)?,
            ));
        }
        None
    }

    /// The profile this action names.
    pub(crate) fn profile_id(self) -> ProfileId {
        match self {
            ProfileAction::OpenTab(id)
            | ProfileAction::OpenWindow(id)
            | ProfileAction::Split(id, _)
            | ProfileAction::SetTabProfile(id) => id,
        }
    }
}

/// Why one stored shortcut was not migrated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SkipReason {
    /// The string does not parse as a chord.
    Unparseable,
    /// Another binding already holds the chord.
    ChordTaken { by: String },
}

/// What [`migrate_profile_shortcuts`] did.
#[derive(Debug, Default, PartialEq, Eq)]
pub(crate) struct ShortcutMigration {
    /// `(profile name, chord)` pairs that became `open_profile:` bindings.
    pub(crate) migrated: Vec<(String, String)>,
    /// `(profile name, chord, why)` left on the profile, untouched.
    pub(crate) skipped: Vec<(String, String, SkipReason)>,
}

impl ShortcutMigration {
    /// Whether the config or the profiles changed and must be saved.
    pub(crate) fn changed(&self) -> bool {
        !self.migrated.is_empty()
    }
}

/// Move every profile's `keyboard_shortcut` into an `open_profile:<id>`
/// registry binding (UX.md MD3), keeping the user's chord.
///
/// - Reads each profile's own field, never the inherited one, so a child
///   that inherited its parent's shortcut adds no duplicate binding (one of
///   the B59 defects).
/// - A chord that does not parse, or that another binding (a default, a
///   user binding, or an earlier profile) already holds, is skipped and left
///   on the profile so nothing the user wrote is lost silently.
/// - A migrated profile's field is cleared: a second run finds nothing.
pub(crate) fn migrate_profile_shortcuts(
    config: &mut Config,
    profiles: &mut [Profile],
) -> ShortcutMigration {
    let mut report = ShortcutMigration::default();
    for profile in profiles.iter_mut() {
        let Some(raw) = profile.keyboard_shortcut.clone() else {
            continue;
        };
        let chord = raw.trim();
        if chord.is_empty() {
            profile.keyboard_shortcut = None;
            continue;
        }
        let Ok(combo) = par_term_keybindings::parser::parse_key_combo(chord) else {
            report
                .skipped
                .push((profile.name.clone(), raw.clone(), SkipReason::Unparseable));
            continue;
        };
        let combo = combo.platform_normalized();
        let own_action = ProfileAction::OpenTab(profile.id).id();
        let taken_by = config.keybindings.iter().find_map(|kb| {
            par_term_keybindings::parser::parse_key_combo(&kb.key)
                .ok()
                .map(par_term_keybindings::parser::KeyCombo::platform_normalized)
                .filter(|other| *other == combo)
                .map(|_| kb.action.clone())
        });
        // Already bound to this profile (a save that wrote the field back
        // after an earlier migration): only the field is left to clear.
        if taken_by.as_deref() == Some(own_action.as_str()) {
            profile.keyboard_shortcut = None;
            report
                .migrated
                .push((profile.name.clone(), chord.to_string()));
            continue;
        }
        if let Some(by) = taken_by {
            report.skipped.push((
                profile.name.clone(),
                raw.clone(),
                SkipReason::ChordTaken { by },
            ));
            continue;
        }
        config.keybindings.push(KeyBinding {
            key: chord.to_string(),
            action: own_action,
        });
        profile.keyboard_shortcut = None;
        report
            .migrated
            .push((profile.name.clone(), chord.to_string()));
    }
    report
}

/// Run the migration against the on-disk profiles and persist both files
/// when anything moved. Called once at startup, before the first window
/// builds its keybinding registry; a later window's `Config::load()` and
/// `load_profiles()` then read the migrated state from disk.
pub(crate) fn migrate_profile_shortcuts_on_disk(config: &mut Config) {
    let mut manager = match crate::profile::storage::load_profiles() {
        Ok(manager) => manager,
        Err(e) => {
            log::warn!("Profile shortcut migration skipped: profiles did not load: {e:#}");
            return;
        }
    };
    let mut profiles = manager.to_vec();
    let report = migrate_profile_shortcuts(config, &mut profiles);
    for (name, chord, why) in &report.skipped {
        log::warn!(
            "Profile '{name}' keeps its keyboard_shortcut {chord:?}: not migrated to a \
             keybinding ({why:?}). Bind open_profile:<id> in Settings › Keys instead."
        );
    }
    if !report.changed() {
        return;
    }
    for (name, chord) in &report.migrated {
        log::info!("Migrated profile '{name}' shortcut {chord:?} to an open_profile binding");
    }
    manager = crate::profile::ProfileManager::from_profiles(profiles);
    // Config first: a crash between the writes leaves the binding in place
    // and the profile field set, which the next run reports as ChordTaken
    // instead of binding the chord twice.
    if let Err(e) = config.save() {
        log::error!("Profile shortcut migration: saving config failed: {e:#}");
        return;
    }
    if let Err(e) = crate::profile::storage::save_profiles(&manager) {
        log::error!("Profile shortcut migration: saving profiles failed: {e:#}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(name: &str, shortcut: Option<&str>) -> Profile {
        let mut p = Profile::new(name);
        p.keyboard_shortcut = shortcut.map(str::to_string);
        p
    }

    #[test]
    fn action_ids_round_trip() {
        let id = uuid::Uuid::new_v4();
        for action in [
            ProfileAction::OpenTab(id),
            ProfileAction::OpenWindow(id),
            ProfileAction::Split(id, ProfileSplit::Right),
            ProfileAction::Split(id, ProfileSplit::Down),
            ProfileAction::SetTabProfile(id),
        ] {
            assert_eq!(ProfileAction::parse(&action.id()), Some(action));
            assert_eq!(action.profile_id(), id);
        }
        assert!(
            PROFILE_ACTION_PREFIXES
                .iter()
                .any(|p| ProfileAction::OpenWindow(id).id().starts_with(p))
        );
    }

    #[test]
    fn malformed_profile_actions_do_not_parse() {
        let id = uuid::Uuid::new_v4();
        for bad in [
            "open_profile:".to_string(),
            "open_profile:not-a-uuid".to_string(),
            format!("split_profile:{id}"),
            format!("split_profile:{id}:left"),
            format!("set_tab_profile:{id}x"),
            "new_tab".to_string(),
        ] {
            assert_eq!(ProfileAction::parse(&bad), None, "{bad}");
        }
    }

    #[test]
    fn a_stored_shortcut_becomes_an_open_profile_binding_with_the_users_chord() {
        let mut config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        let mut profiles = vec![profile("Work", Some("Ctrl+Alt+W"))];
        let id = profiles[0].id;
        let report = migrate_profile_shortcuts(&mut config, &mut profiles);
        assert_eq!(
            report.migrated,
            vec![("Work".to_string(), "Ctrl+Alt+W".to_string())]
        );
        assert!(report.skipped.is_empty());
        let binding = config
            .keybindings
            .iter()
            .find(|kb| kb.action == format!("open_profile:{id}"))
            .expect("binding added");
        assert_eq!(binding.key, "Ctrl+Alt+W");
        assert_eq!(profiles[0].keyboard_shortcut, None, "field cleared");
        // The migrated chord fires through the registry.
        let registry = par_term_keybindings::KeybindingRegistry::from_config(&config.keybindings);
        assert_eq!(
            registry.find_by_chord("Ctrl+Alt+W"),
            Some(format!("open_profile:{id}").as_str())
        );
    }

    #[test]
    fn the_migration_runs_once() {
        let mut config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        let mut profiles = vec![profile("Work", Some("Ctrl+Alt+W"))];
        migrate_profile_shortcuts(&mut config, &mut profiles);
        let second = migrate_profile_shortcuts(&mut config, &mut profiles);
        assert!(!second.changed());
        assert_eq!(config.keybindings.len(), 1);
    }

    #[test]
    fn a_chord_already_bound_to_the_same_profile_only_clears_the_field() {
        let mut config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        let mut profiles = vec![profile("Work", Some("Ctrl+Alt+W"))];
        migrate_profile_shortcuts(&mut config, &mut profiles);
        // A Settings save writes the old field back.
        profiles[0].keyboard_shortcut = Some("Ctrl+Alt+W".to_string());
        let report = migrate_profile_shortcuts(&mut config, &mut profiles);
        assert!(report.skipped.is_empty());
        assert_eq!(config.keybindings.len(), 1, "no second binding");
        assert_eq!(profiles[0].keyboard_shortcut, None);
    }

    #[test]
    fn a_taken_or_unparseable_chord_is_skipped_and_kept_on_the_profile() {
        let mut config = Config::default();
        // A shipped default owns this chord on every platform.
        let taken = config.keybindings[0].key.clone();
        let taken_action = config.keybindings[0].action.clone();
        let before = config.keybindings.len();
        let mut profiles = vec![
            profile("Clash", Some(&taken)),
            profile("Junk", Some("Cmd+Nonsense+Q")),
        ];
        let report = migrate_profile_shortcuts(&mut config, &mut profiles);
        assert!(report.migrated.is_empty());
        assert_eq!(config.keybindings.len(), before, "nothing added");
        assert_eq!(
            report.skipped[0].2,
            SkipReason::ChordTaken { by: taken_action }
        );
        assert_eq!(report.skipped[1].2, SkipReason::Unparseable);
        assert_eq!(
            profiles[0].keyboard_shortcut.as_deref(),
            Some(taken.as_str())
        );
        assert_eq!(
            profiles[1].keyboard_shortcut.as_deref(),
            Some("Cmd+Nonsense+Q")
        );
    }

    #[test]
    fn two_profiles_with_one_chord_bind_it_once() {
        // B59: inherited and copied shortcuts created silent duplicates.
        let mut config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        let mut profiles = vec![
            profile("First", Some("Ctrl+Alt+1")),
            profile("Second", Some("ctrl+alt+1")),
        ];
        let report = migrate_profile_shortcuts(&mut config, &mut profiles);
        assert_eq!(report.migrated.len(), 1);
        assert_eq!(report.skipped.len(), 1);
        assert_eq!(config.keybindings.len(), 1);
    }

    #[test]
    fn an_inherited_shortcut_is_not_bound_for_the_child() {
        // The child has no shortcut of its own; resolve_profile would hand
        // it the parent's. The migration reads the raw field only.
        let mut config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        let parent = profile("Parent", Some("Ctrl+Alt+P"));
        let mut child = profile("Child", None);
        child.parent_id = Some(parent.id);
        let mut profiles = vec![parent, child];
        let report = migrate_profile_shortcuts(&mut config, &mut profiles);
        assert_eq!(report.migrated.len(), 1);
        assert_eq!(config.keybindings.len(), 1);
    }

    #[test]
    fn a_migrated_chord_can_be_a_menu_accelerator() {
        // UX.md acceptance: a migrated shortcut appears as the menu
        // accelerator. The menu derives accelerators from registry chords;
        // the migration keeps the chord string, so it must still convert.
        let mut config = Config {
            keybindings: Vec::new(),
            ..Config::default()
        };
        let mut profiles = vec![profile("Work", Some("CmdOrCtrl+Alt+W"))];
        migrate_profile_shortcuts(&mut config, &mut profiles);
        let combo = par_term_keybindings::parser::parse_key_combo(&config.keybindings[0].key)
            .expect("migrated chord parses");
        assert!(combo.modifiers.alt);
    }
}
