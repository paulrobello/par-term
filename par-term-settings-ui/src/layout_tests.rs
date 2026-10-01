//! SP3 acceptance criterion 1 (UX.md 15.3): every section of the 13-tab
//! Settings appears in exactly one home in the 12-tab layout.
//!
//! `OLD_TO_NEW` freezes the 86 sections the pre-SP3 window registered (tab
//! and id, harvested from the old layout) and names each one's new tab and
//! page. The test reads the registry harvested from the new layout, so a
//! section that stops drawing, lands on another page, or is drawn twice
//! fails here.

use std::collections::HashMap;

use crate::search::test_registry;
use crate::sidebar::SettingsTab;

/// `(old tab, section id, new tab, new page)`.
const OLD_TO_NEW: &[(&str, &str, SettingsTab, &str)] = {
    use SettingsTab::*;
    &[
        ("Appearance", "appearance_theme", Appearance, "Theme"),
        (
            "Appearance",
            "appearance_auto_dark_mode",
            Appearance,
            "Theme",
        ),
        ("Appearance", "appearance_fonts", Appearance, "Text & Fonts"),
        (
            "Appearance",
            "appearance_font_variants",
            Appearance,
            "Text & Fonts",
        ),
        (
            "Appearance",
            "appearance_font_rendering",
            Appearance,
            "Text & Fonts",
        ),
        ("Appearance", "appearance_cursor", Appearance, "Cursor"),
        (
            "Appearance",
            "appearance_cursor_locks",
            Appearance,
            "Cursor",
        ),
        (
            "Appearance",
            "appearance_cursor_effects",
            Appearance,
            "Cursor",
        ),
        ("Appearance", "badge_general", Appearance, "Badge"),
        ("Appearance", "badge_appearance", Appearance, "Badge"),
        ("Appearance", "badge_position", Appearance, "Badge"),
        ("Appearance", "badge_variables", Appearance, "Badge"),
        (
            "Appearance",
            "progress_bar_general",
            Appearance,
            "Progress Bar",
        ),
        (
            "Appearance",
            "progress_bar_colors",
            Appearance,
            "Progress Bar",
        ),
        ("Window", "window_display", WindowsAndTabs, "Window"),
        ("Window", "window_transparency", WindowsAndTabs, "Window"),
        (
            "Window",
            "window_performance",
            Advanced,
            "Performance & Power",
        ),
        ("Window", "window_behavior", WindowsAndTabs, "Window"),
        ("Window", "window_tab_bar", WindowsAndTabs, "Tab Bar"),
        (
            "Window",
            "window_tab_bar_appearance",
            WindowsAndTabs,
            "Tab Bar Colors",
        ),
        ("Window", "window_panes", Panes, "Layout & Dividers"),
        ("Window", "window_pane_appearance", Panes, "Appearance"),
        ("Window", "window_scrollbar", WindowsAndTabs, "Scrollbar"),
        ("Window", "arrangements_save", Sessions, "Arrangements"),
        ("Window", "arrangements_list", Sessions, "Arrangements"),
        (
            "Window",
            "arrangements_auto_restore",
            General,
            "Startup & Restore",
        ),
        ("Input", "input_keyboard", Keys, "Option/Alt"),
        ("Input", "input_modifier_remapping", Keys, "Modifiers"),
        ("Input", "input_mouse", Pointer, "Mouse"),
        ("Input", "input_selection", General, "Selection & Clipboard"),
        (
            "Input",
            "input_clipboard_limits",
            General,
            "Selection & Clipboard",
        ),
        ("Input", "input_word_selection", Pointer, "Word Selection"),
        ("Input", "input_copy_mode", Pointer, "Copy Mode"),
        ("Input", "input_keybindings", Keys, "Key Bindings"),
        // Split: scrollback keeps this id; closing moved to general_closing.
        (
            "Terminal",
            "terminal_behavior",
            Advanced,
            "Terminal Emulation",
        ),
        (
            "Terminal",
            "terminal_unicode",
            Advanced,
            "Terminal Emulation",
        ),
        ("Terminal", "terminal_shell", General, "Startup & Restore"),
        ("Terminal", "terminal_startup", General, "Startup & Restore"),
        ("Terminal", "terminal_search", General, "Search & Links"),
        (
            "Terminal",
            "terminal_semantic_history",
            General,
            "Search & Links",
        ),
        (
            "Terminal",
            "terminal_command_history",
            General,
            "Search & Links",
        ),
        (
            "Terminal",
            "terminal_command_separator",
            Appearance,
            "Theme",
        ),
        (
            "Effects",
            "background_effects",
            Effects,
            "Background & Shader",
        ),
        ("Effects", "per_pane_background", Panes, "Pane Backgrounds"),
        ("Effects", "inline_images", Effects, "Inline Images"),
        ("Effects", "cursor_shader", Appearance, "Cursor"),
        ("StatusBar", "status_bar_general", Advanced, "Status Bar"),
        ("StatusBar", "status_bar_styling", Advanced, "Status Bar"),
        ("StatusBar", "status_bar_auto_hide", Advanced, "Status Bar"),
        (
            "StatusBar",
            "status_bar_widget_options",
            Advanced,
            "Status Bar",
        ),
        (
            "StatusBar",
            "status_bar_agent_usage",
            Assistant,
            "Agent Usage",
        ),
        (
            "StatusBar",
            "status_bar_poll_intervals",
            Advanced,
            "Status Bar",
        ),
        ("StatusBar", "status_bar_widgets", Advanced, "Status Bar"),
        ("Profiles", "profiles_management", Profiles, "Profiles"),
        ("Profiles", "profiles_display", Profiles, "Profiles"),
        (
            "Profiles",
            "profiles_dynamic_sources",
            Profiles,
            "Dynamic Sources",
        ),
        (
            "Notifications",
            "notifications_bell",
            Advanced,
            "Notifications & Bell",
        ),
        (
            "Notifications",
            "notifications_activity",
            Advanced,
            "Notifications & Bell",
        ),
        (
            "Notifications",
            "notifications_alert_sounds",
            Advanced,
            "Notifications & Bell",
        ),
        (
            "Notifications",
            "notifications_behavior",
            Advanced,
            "Notifications & Bell",
        ),
        (
            "Notifications",
            "notifications_anti_idle",
            Advanced,
            "Terminal Emulation",
        ),
        (
            "Integrations",
            "integrations_shell",
            General,
            "Integration & Files",
        ),
        (
            "Integrations",
            "integrations_shaders",
            Effects,
            "Background & Shader",
        ),
        ("Integrations", "integrations_ssh", Profiles, "SSH"),
        ("Automation", "automation_triggers", Automation, "Triggers"),
        (
            "Automation",
            "automation_coprocesses",
            Automation,
            "Coprocesses",
        ),
        ("Automation", "scripts_list", Automation, "Observer Scripts"),
        ("Automation", "automation_plugins", Automation, "Plugins"),
        ("Snippets", "snippets_list", Automation, "Snippets"),
        ("Snippets", "snippets_variables", Automation, "Snippets"),
        ("Snippets", "actions_list", Automation, "Custom Actions"),
        (
            "Snippets",
            "agent_commands_list",
            Assistant,
            "Agent Commands",
        ),
        ("Snippets", "agents_list", Assistant, "Agents"),
        ("AiInspector", "ai_inspector_panel", Assistant, "Panel"),
        ("AiInspector", "ai_inspector_agent", Assistant, "Agents"),
        (
            "AiInspector",
            "ai_inspector_prompt_library",
            Assistant,
            "Prompt Library",
        ),
        (
            "AiInspector",
            "ai_inspector_custom_agents",
            Assistant,
            "Agents",
        ),
        (
            "AiInspector",
            "ai_inspector_permissions",
            Assistant,
            "Permissions",
        ),
        (
            "Advanced",
            "advanced_import_export",
            Advanced,
            "Import/Export",
        ),
        ("Advanced", "advanced_tmux", Sessions, "tmux"),
        ("Advanced", "advanced_logging", Advanced, "Logging"),
        (
            "Advanced",
            "advanced_screenshots",
            General,
            "Integration & Files",
        ),
        ("Advanced", "advanced_updates", General, "Updates"),
        (
            "Advanced",
            "advanced_file_transfers",
            General,
            "Integration & Files",
        ),
        ("Advanced", "advanced_debug_logging", Advanced, "Logging"),
        ("Advanced", "advanced_security", Advanced, "Security"),
    ]
};

/// Sections SP3 added: `(id, tab, page)`.
const NEW_SECTIONS: &[(&str, SettingsTab, &str)] = &[
    // SD3: the quick-settings strip as a page.
    ("general_common", SettingsTab::General, "Common"),
    // SX2: split out of terminal_behavior.
    (
        "general_closing",
        SettingsTab::General,
        "Closing & Quitting",
    ),
    // SX1.
    ("sessions_par_mux", SettingsTab::Sessions, "par-mux"),
    // Leader key controls (card 01a0f612a670).
    ("input_leader", SettingsTab::Keys, "Leader Key"),
];

fn page_title(tab: SettingsTab, page: usize) -> &'static str {
    crate::layout::page_at(tab, page).title
}

#[test]
fn the_old_layout_had_86_sections_on_13_tabs() {
    assert_eq!(OLD_TO_NEW.len(), 86);
    let tabs: std::collections::HashSet<&str> = OLD_TO_NEW.iter().map(|(t, ..)| *t).collect();
    assert_eq!(tabs.len(), 13, "old tabs: {tabs:?}");
}

/// Criterion 1: each old section is drawn exactly once, on the page the
/// mapping names; every drawn section is either mapped or new.
#[test]
fn every_old_section_has_exactly_one_new_home() {
    let registry = test_registry();
    let mut drawn: HashMap<&str, Vec<(SettingsTab, usize)>> = HashMap::new();
    for section in &registry.sections {
        drawn
            .entry(section.id.as_str())
            .or_default()
            .push((section.tab, section.page));
    }

    let mut failures = Vec::new();
    let expected = OLD_TO_NEW
        .iter()
        .map(|(_, id, tab, page)| (*id, *tab, *page))
        .chain(NEW_SECTIONS.iter().copied());
    for (id, tab, page) in expected {
        match drawn.get(id).map(Vec::as_slice) {
            None | Some([]) => failures.push(format!("{id}: not drawn anywhere")),
            Some([(t, p)]) if *t == tab && page_title(*t, *p) == page => {}
            Some([(t, p)]) => failures.push(format!(
                "{id}: on {t:?} › {}, expected {tab:?} › {page}",
                page_title(*t, *p)
            )),
            Some(many) => failures.push(format!("{id}: drawn {} times", many.len())),
        }
    }

    for id in drawn.keys() {
        let known = OLD_TO_NEW.iter().any(|(_, i, ..)| i == id)
            || NEW_SECTIONS.iter().any(|(i, ..)| i == id);
        if !known {
            failures.push(format!("{id}: drawn but neither mapped nor listed as new"));
        }
    }
    failures.sort();
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// The layout table and the registry agree: every section the table lists
/// is a section the page actually registers.
#[test]
fn every_layout_entry_registers_its_id() {
    let registry = test_registry();
    let mut missing = Vec::new();
    for tab in SettingsTab::all() {
        for (pi, page) in crate::layout::pages(*tab).iter().enumerate() {
            for section in page.sections {
                let found = registry
                    .sections
                    .iter()
                    .any(|s| s.id == section.id && s.tab == *tab && s.page == pi);
                if !found {
                    missing.push(format!("{tab:?} › {} › {}", page.title, section.id));
                }
            }
        }
    }
    assert!(missing.is_empty(), "layout ids not registered: {missing:?}");
}

/// UX.md 15.2: twelve tabs, each with at least one page, no page empty.
#[test]
fn twelve_tabs_each_with_pages() {
    assert_eq!(SettingsTab::all().len(), 12);
    for tab in SettingsTab::all() {
        let pages = crate::layout::pages(*tab);
        assert!(!pages.is_empty(), "{tab:?} has no pages");
        for page in pages {
            assert!(
                !page.sections.is_empty(),
                "{tab:?} › {} is empty",
                page.title
            );
        }
    }
}

/// Criterion 3: Sessions has a global par-mux section, and General has a
/// Closing & Quitting page holding the D6 settings.
#[test]
fn sessions_has_par_mux_and_general_has_closing_and_quitting() {
    let registry = test_registry();
    let section = |id: &str| {
        registry
            .sections
            .iter()
            .find(|s| s.id == id)
            .unwrap_or_else(|| panic!("no section {id}"))
    };
    let mux = section("sessions_par_mux");
    assert_eq!(mux.tab, SettingsTab::Sessions);
    assert!(
        mux.controls
            .iter()
            .any(|c| c.label == "Attach on launch:"
                && c.extra.iter().any(|e| e == "mux_auto_attach")),
        "par-mux section has no mux_auto_attach control: {:?}",
        mux.controls
    );

    let closing = section("general_closing");
    assert_eq!(closing.tab, SettingsTab::General);
    assert_eq!(page_title(closing.tab, closing.page), "Closing & Quitting");
    for key in [
        "prompt_on_quit",
        "confirm_close_multiple_tabs",
        "confirm_close_running_jobs",
        "jobs_to_ignore",
        "shell_exit_action",
        "session_undo_timeout_secs",
        "session_undo_preserve_shell",
    ] {
        assert!(
            closing
                .controls
                .iter()
                .any(|c| c.extra.iter().any(|e| e == key)),
            "Closing & Quitting has no control tagged {key}"
        );
    }
}
