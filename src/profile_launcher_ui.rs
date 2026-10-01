//! Open Profiles… (UX.md PR1, MD2): one keyboard-first profile launcher on
//! the shared list component (`app::overlay::picker`, OV5).
//!
//! - Search over name and tags; a tag chip row filters by tag.
//! - Enter opens the selected profile in a new tab, Shift+Enter in a new
//!   window, Cmd+D / Cmd+Shift+D split right / down, Cmd+Enter changes the
//!   active tab's profile.
//! - Ctrl+Space marks rows; an activation with marks opens every marked
//!   profile.
//! - Each row shows the profile's live registry shortcut (its
//!   `open_profile:<id>` chord), a `dynamic` marker for remotely fetched
//!   profiles, and a `default` marker for the profile a new tab already
//!   runs (none of the profiles is: par-term's default tab runs the
//!   configured shell, so the launcher lists a "Default" row first).
//! - Right-click a row: Open in New Tab / New Window / Split Right /
//!   Split Down / This Tab, Edit Profile…, Duplicate.
//!
//! It replaces the tab bar's chevron "New Tab" window and the
//! `new_tab_shortcut_shows_profiles` picker. The Profiles drawer (PR2) is
//! the same list pinned to the window's right edge
//! ([`ProfileLauncherUI::show_pinned`] inside `ProfileDrawerUI`).
//!
//! The launcher returns [`LauncherChoice`]s; `WindowState` runs them as the
//! profile registry actions, so a launcher pick and a bound shortcut take
//! one path.

use crate::app::overlay::picker::{self, Activation, ExtraKey, ListConfig, ListNav, ListOutcome};
use crate::profile::{Profile, ProfileId, ProfileManager};
use egui::{Context, Key, Modifiers, RichText};

/// Rows drawn before the list scrolls (popup).
const VISIBLE_ROWS: usize = 12;

/// The launcher's extra chords, in `Activation::Extra` index order.
const EXTRA_KEYS: [ExtraKey; 2] = [
    ExtraKey {
        modifiers: Modifiers::COMMAND,
        key: Key::D,
        label: "Cmd+D split right",
    },
    ExtraKey {
        modifiers: Modifiers {
            alt: false,
            ctrl: false,
            shift: true,
            mac_cmd: false,
            command: true,
        },
        key: Key::D,
        label: "Cmd+Shift+D split down",
    },
];

/// Where a chosen profile opens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchTarget {
    NewTab,
    NewWindow,
    SplitRight,
    SplitDown,
    /// Change the active tab's profile.
    ThisTab,
}

/// What the launcher asks the window to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LauncherChoice {
    /// Open each profile (`None` = the Default row: a plain new tab or
    /// window) at `target`.
    Open {
        profiles: Vec<Option<ProfileId>>,
        target: LaunchTarget,
    },
    /// Settings › Profiles, for this profile.
    Edit(ProfileId),
    /// Copy this profile (new id, " (copy)" name, no shortcut).
    Duplicate(ProfileId),
    /// Settings › Profiles.
    Manage,
}

/// One row: the Default row (`profile: None`) or a profile.
#[derive(Debug, Clone, PartialEq)]
pub struct LauncherRow {
    pub profile: Option<ProfileId>,
    pub label: String,
    pub tags: Vec<String>,
    /// Live registry chord for `open_profile:<id>`.
    pub chord: Option<String>,
    pub dynamic: bool,
}

/// The launcher's state.
#[derive(Default)]
pub struct ProfileLauncherUI {
    /// The popup is open.
    pub visible: bool,
    query: String,
    tag: Option<String>,
    nav: ListNav,
    request_focus: bool,
    rows: Vec<LauncherRow>,
    marked: Vec<Option<ProfileId>>,
    toggle_chord: Option<String>,
}

impl ProfileLauncherUI {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open the popup with fresh rows (closes the pinned view: one
    /// launcher on screen at a time).
    pub fn open(&mut self, rows: Vec<LauncherRow>, toggle_chord: Option<String>) {
        self.rows = rows;
        self.toggle_chord = toggle_chord;
        self.visible = true;
        self.query.clear();
        self.tag = None;
        self.marked.clear();
        self.nav.reset();
        self.request_focus = true;
    }

    pub fn close(&mut self) {
        self.visible = false;
        self.marked.clear();
    }

    /// Replace the rows (the pinned view refreshes them each frame).
    pub fn set_rows(&mut self, rows: Vec<LauncherRow>) {
        self.rows = rows;
    }

    /// Reset the list for the pinned view as it opens: the popup closes
    /// (one launcher on screen) and the filter starts empty and focused.
    pub fn prepare_pinned(&mut self) {
        self.visible = false;
        self.query.clear();
        self.tag = None;
        self.marked.clear();
        self.nav.reset();
        self.request_focus = true;
    }

    /// Test seam: type into the filter.
    #[cfg(test)]
    pub(crate) fn set_query_for_test(&mut self, query: &str) {
        self.query = query.to_string();
        self.nav.reset();
    }

    /// Test seam: pick a tag chip.
    #[cfg(test)]
    pub(crate) fn set_tag_for_test(&mut self, tag: Option<&str>) {
        self.tag = tag.map(str::to_string);
        self.nav.reset();
    }

    /// Rows matching the query and the tag chip, Default first. The query
    /// matches name or tags as a case-insensitive substring, then ranks by
    /// the shared fuzzy score.
    pub fn filtered(&self) -> Vec<&LauncherRow> {
        let tag = self.tag.as_deref().map(str::to_lowercase);
        let rows = self.rows.iter().filter(|row| match &tag {
            Some(tag) => row.tags.iter().any(|t| t.to_lowercase() == *tag),
            None => true,
        });
        let query = self.query.trim();
        if query.is_empty() {
            return rows.collect();
        }
        let mut scored: Vec<(u32, usize, &LauncherRow)> = rows
            .enumerate()
            .filter_map(|(i, row)| {
                let haystack = format!("{} {}", row.label, row.tags.join(" "));
                crate::command_palette::fuzzy::score(query, &haystack).map(|s| (s, i, row))
            })
            .collect();
        scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
        scored.into_iter().map(|(_, _, row)| row).collect()
    }

    /// Every tag across the rows, sorted, for the chip row.
    fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self.rows.iter().flat_map(|r| r.tags.clone()).collect();
        tags.sort_by_key(|t| t.to_lowercase());
        tags.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
        tags
    }

    /// The profiles an activation of `index` opens: the marked rows when
    /// any are marked, otherwise the row itself.
    fn activation_profiles(&self, index_profile: Option<ProfileId>) -> Vec<Option<ProfileId>> {
        if self.marked.is_empty() {
            vec![index_profile]
        } else {
            self.marked.clone()
        }
    }

    /// Draw the popup (when open); returns what was chosen.
    pub fn show(&mut self, ctx: &Context) -> Option<LauncherChoice> {
        if !self.visible {
            return None;
        }
        let mut choice = None;
        let mut close = false;
        egui::Window::new("Open Profiles")
            .title_bar(false)
            .resizable(false)
            .collapsible(false)
            .anchor(egui::Align2::CENTER_TOP, [0.0, 80.0])
            .frame(egui::Frame::popup(&ctx.global_style()).shadow(egui::epaint::Shadow::default()))
            .show(ctx, |ui| {
                ui.set_min_width(crate::app::overlay::theme::WIDTH_MEDIUM);
                (choice, close) = self.body(ui, true);
            });
        if close || choice.is_some() {
            self.close();
        }
        choice
    }

    /// Draw the pinned view into a docked panel's `ui`. Returns what was
    /// chosen and whether the panel should close (its ✕, or Escape while
    /// its filter is focused).
    pub fn show_pinned(&mut self, ui: &mut egui::Ui) -> (Option<LauncherChoice>, bool) {
        let mut close = false;
        let mut manage = false;
        ui.horizontal(|ui| {
            ui.heading("Profiles");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                close = ui.small_button("✕").on_hover_text("Close").clicked();
                manage = ui.small_button("Manage").clicked();
            });
        });
        let (choice, escaped) = self.body(ui, false);
        if choice.is_some() {
            self.marked.clear();
        }
        let choice = if manage {
            Some(LauncherChoice::Manage)
        } else {
            choice
        };
        (choice, close || escaped)
    }

    /// The shared body: tag chips, the list, the context menu.
    fn body(&mut self, ui: &mut egui::Ui, owns_keys: bool) -> (Option<LauncherChoice>, bool) {
        let tags = self.all_tags();
        if !tags.is_empty() {
            ui.horizontal_wrapped(|ui| {
                if ui.selectable_label(self.tag.is_none(), "all").clicked() {
                    self.tag = None;
                    self.nav.reset();
                }
                for tag in &tags {
                    let on = self
                        .tag
                        .as_deref()
                        .is_some_and(|t| t.eq_ignore_ascii_case(tag));
                    if ui.selectable_label(on, format!("#{tag}")).clicked() {
                        self.tag = (!on).then(|| tag.clone());
                        self.nav.reset();
                    }
                }
            });
        }

        let matches: Vec<LauncherRow> = self.filtered().into_iter().cloned().collect();
        let marked = self.marked.clone();
        let mut context: Option<LauncherChoice> = None;
        // The config borrows the toggle chord; clone it so `self`'s query
        // and nav can be borrowed mutably by the body below.
        let toggle = self.toggle_chord.clone();
        let config = ListConfig {
            toggle_chord: toggle.as_deref(),
            ..launcher_config()
        };
        let (outcome, query_changed) = picker::list_body(
            ui,
            &config,
            picker::ListBodyState {
                query: &mut self.query,
                request_focus: &mut self.request_focus,
                nav: &mut self.nav,
            },
            matches.len(),
            owns_keys,
            |ui, index, selected| {
                let row = &matches[index];
                let (clicked, menu) = draw_row(ui, row, selected, marked.contains(&row.profile));
                if menu.is_some() {
                    context = menu;
                }
                clicked
            },
        );
        if query_changed {
            self.nav.reset();
        }
        if context.is_some() {
            return (context, true);
        }
        match outcome {
            ListOutcome::Open => (None, false),
            ListOutcome::Closed => (None, true),
            ListOutcome::ToggleMark(index) => {
                if let Some(row) = matches.get(index) {
                    match self.marked.iter().position(|m| *m == row.profile) {
                        Some(at) => {
                            self.marked.remove(at);
                        }
                        None => self.marked.push(row.profile),
                    }
                }
                (None, false)
            }
            ListOutcome::Chosen { index, how } => {
                let Some(row) = matches.get(index) else {
                    return (None, false);
                };
                let target = match how {
                    Activation::Primary => LaunchTarget::NewTab,
                    Activation::Shift => LaunchTarget::NewWindow,
                    Activation::Command => LaunchTarget::ThisTab,
                    Activation::Extra(0) => LaunchTarget::SplitRight,
                    Activation::Extra(_) => LaunchTarget::SplitDown,
                };
                let profiles = self.activation_profiles(row.profile);
                (Some(LauncherChoice::Open { profiles, target }), true)
            }
        }
    }

    /// Choose the selected row as if by keyboard (the native-menu path:
    /// on macOS the menu's Cmd+D accelerator fires before the launcher's
    /// egui frame sees the key, so the window routes it here).
    pub fn choose_selected(&mut self, target: LaunchTarget) -> Option<LauncherChoice> {
        let profile = self.filtered().get(self.nav.selected)?.profile;
        let profiles = self.activation_profiles(profile);
        self.close();
        Some(LauncherChoice::Open { profiles, target })
    }
}

/// The launcher's list configuration, toggle chord unset.
fn launcher_config() -> ListConfig<'static> {
    ListConfig {
        id: "Open Profiles",
        hint: "Profile name or tag",
        visible_rows: VISIBLE_ROWS,
        width: crate::app::overlay::theme::WIDTH_MEDIUM,
        empty_text: "No matching profiles",
        enter_verb: "new tab",
        toggle_chord: None,
        alternates: true,
        alternate_labels: Some(("new window", "this tab")),
        extra_keys: &EXTRA_KEYS,
        multi_select: true,
    }
}

/// One row: mark, label, markers, tags, live chord; right-click menu.
fn draw_row(
    ui: &mut egui::Ui,
    row: &LauncherRow,
    selected: bool,
    marked: bool,
) -> (bool, Option<LauncherChoice>) {
    let mut clicked = false;
    let mut menu = None;
    ui.horizontal(|ui| {
        let check = if marked { "☑ " } else { "" };
        let label = format!("{check}{}", row.label);
        let label = if selected {
            RichText::new(label).strong()
        } else {
            RichText::new(label)
        };
        let response = ui.selectable_label(selected, label);
        clicked = response.clicked();
        response.context_menu(|ui| {
            menu = row_menu(ui, row.profile);
        });
        if row.profile.is_none() {
            ui.weak("default");
        }
        if row.dynamic {
            ui.weak("dynamic");
        }
        for tag in &row.tags {
            ui.weak(format!("#{tag}"));
        }
        if let Some(chord) = &row.chord {
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.weak(chord.as_str())
            });
        }
    });
    (clicked, menu)
}

/// The row context menu (UX.md PR1: Open in…, Edit Profile…, Duplicate).
fn row_menu(ui: &mut egui::Ui, profile: Option<ProfileId>) -> Option<LauncherChoice> {
    let mut choice = None;
    let open = |target| LauncherChoice::Open {
        profiles: vec![profile],
        target,
    };
    ui.menu_button("Open in", |ui| {
        for (label, target) in [
            ("New Tab", LaunchTarget::NewTab),
            ("New Window", LaunchTarget::NewWindow),
            ("Split Right", LaunchTarget::SplitRight),
            ("Split Down", LaunchTarget::SplitDown),
            ("This Tab", LaunchTarget::ThisTab),
        ] {
            if ui.button(label).clicked() {
                choice = Some(open(target));
                ui.close();
            }
        }
    });
    match profile {
        Some(id) => {
            if ui.button("Edit Profile…").clicked() {
                choice = Some(LauncherChoice::Edit(id));
                ui.close();
            }
            if ui.button("Duplicate").clicked() {
                choice = Some(LauncherChoice::Duplicate(id));
                ui.close();
            }
        }
        None => {
            if ui.button("Manage Profiles…").clicked() {
                choice = Some(LauncherChoice::Manage);
                ui.close();
            }
        }
    }
    choice
}

/// Build the launcher rows: Default first, then every profile in display
/// order with its live `open_profile:<id>` chord.
pub fn launcher_rows(
    profiles: &ProfileManager,
    chord_for: impl Fn(&str) -> Option<String>,
) -> Vec<LauncherRow> {
    let mut rows = vec![LauncherRow {
        profile: None,
        label: "Default".to_string(),
        tags: Vec::new(),
        chord: None,
        dynamic: false,
    }];
    rows.extend(
        profiles
            .profiles_ordered()
            .into_iter()
            .map(|p: &Profile| LauncherRow {
                profile: Some(p.id),
                label: p.display_label(),
                tags: p.tags.clone(),
                chord: chord_for(&crate::profile::actions::ProfileAction::OpenTab(p.id).id()),
                dynamic: p.source.is_dynamic(),
            }),
    );
    rows
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rows() -> Vec<LauncherRow> {
        let mut manager = ProfileManager::new();
        let mut work = Profile::new("Work");
        work.tags = vec!["dev".to_string()];
        let mut prod = Profile::new("Prod SSH");
        prod.tags = vec!["ssh".to_string(), "prod".to_string()];
        manager.add(work);
        manager.add(prod);
        launcher_rows(&manager, |action| {
            action
                .starts_with("open_profile:")
                .then(|| "Cmd+Alt+1".to_string())
        })
    }

    fn launcher() -> ProfileLauncherUI {
        let mut l = ProfileLauncherUI::new();
        l.open(rows(), Some("Cmd+O".to_string()));
        l
    }

    #[test]
    fn rows_lead_with_default_and_carry_the_live_shortcut() {
        let rows = rows();
        assert_eq!(rows[0].profile, None);
        assert_eq!(rows[0].label, "Default");
        assert_eq!(rows[1].label, "Work");
        assert_eq!(rows[1].chord.as_deref(), Some("Cmd+Alt+1"));
    }

    #[test]
    fn the_query_searches_names_and_tags() {
        let mut l = launcher();
        l.set_query_for_test("prod");
        let labels: Vec<&str> = l.filtered().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Prod SSH"]);
        l.set_query_for_test("dev");
        let labels: Vec<&str> = l.filtered().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Work"], "tags match too");
    }

    #[test]
    fn a_tag_chip_filters_by_tag() {
        let mut l = launcher();
        l.set_tag_for_test(Some("ssh"));
        let labels: Vec<&str> = l.filtered().iter().map(|r| r.label.as_str()).collect();
        assert_eq!(labels, ["Prod SSH"]);
        assert_eq!(l.all_tags(), ["dev", "prod", "ssh"]);
    }

    fn frame(
        l: &mut ProfileLauncherUI,
        ctx: &Context,
        key: Key,
        modifiers: Modifiers,
    ) -> Option<LauncherChoice> {
        let mut choice = None;
        for events in [
            vec![],
            vec![egui::Event::Key {
                key,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers,
            }],
        ] {
            let mut out = ctx.run_ui(
                egui::RawInput {
                    events,
                    ..Default::default()
                },
                |ui| {
                    if let Some(c) = l.show(ui.ctx()) {
                        choice = Some(c);
                    }
                },
            );
            out.textures_delta.clear();
        }
        choice
    }

    #[test]
    fn enter_shift_enter_cmd_d_and_cmd_enter_pick_their_targets() {
        // UX.md MP3 acceptance: a profile opens in a new tab, a new window,
        // and a split from the keyboard.
        let ctx = Context::default();
        for (key, modifiers, target) in [
            (Key::Enter, Modifiers::NONE, LaunchTarget::NewTab),
            (Key::Enter, Modifiers::SHIFT, LaunchTarget::NewWindow),
            (Key::Enter, Modifiers::COMMAND, LaunchTarget::ThisTab),
            (Key::D, Modifiers::COMMAND, LaunchTarget::SplitRight),
            (
                Key::D,
                Modifiers::COMMAND | Modifiers::SHIFT,
                LaunchTarget::SplitDown,
            ),
        ] {
            let mut l = launcher();
            l.set_query_for_test("work");
            let work = l.filtered()[0].profile;
            assert_eq!(
                frame(&mut l, &ctx, key, modifiers),
                Some(LauncherChoice::Open {
                    profiles: vec![work],
                    target
                }),
                "{key:?} {modifiers:?}"
            );
            assert!(!l.visible, "a pick closes the popup");
        }
    }

    #[test]
    fn marked_rows_open_together() {
        let ctx = Context::default();
        let mut l = launcher();
        let ids: Vec<Option<ProfileId>> = l.filtered().iter().map(|r| r.profile).collect();
        // Mark Work (row 1) and Prod (row 2).
        frame(&mut l, &ctx, Key::ArrowDown, Modifiers::NONE);
        frame(&mut l, &ctx, Key::Space, Modifiers::CTRL);
        frame(&mut l, &ctx, Key::ArrowDown, Modifiers::NONE);
        frame(&mut l, &ctx, Key::Space, Modifiers::CTRL);
        assert_eq!(
            frame(&mut l, &ctx, Key::Enter, Modifiers::NONE),
            Some(LauncherChoice::Open {
                profiles: vec![ids[1], ids[2]],
                target: LaunchTarget::NewTab
            })
        );
    }

    #[test]
    fn escape_closes_without_a_choice() {
        let ctx = Context::default();
        let mut l = launcher();
        assert_eq!(frame(&mut l, &ctx, Key::Escape, Modifiers::NONE), None);
        assert!(!l.visible);
    }

    #[test]
    fn the_native_menu_split_path_chooses_the_selected_row() {
        let mut l = launcher();
        l.set_query_for_test("prod");
        let prod = l.filtered()[0].profile;
        assert_eq!(
            l.choose_selected(LaunchTarget::SplitRight),
            Some(LauncherChoice::Open {
                profiles: vec![prod],
                target: LaunchTarget::SplitRight
            })
        );
        assert!(!l.visible);
    }
}
