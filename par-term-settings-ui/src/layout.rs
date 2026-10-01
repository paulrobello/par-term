//! The Settings information architecture as data (UX.md 15.2, 15.3).
//!
//! Twelve top-level tabs, each split into sub-pages; every sub-page lists
//! the sections it draws, in order. The sidebar, the page selector, the
//! content area, the search harvest, and the SP3 mapping test all read this
//! one table, so moving a section is a one-line change here.

use std::collections::HashSet;

use crate::SettingsUI;
use crate::sidebar::SettingsTab;

/// Draws one section.
pub(crate) type DrawFn = fn(&mut egui::Ui, &mut SettingsUI, &mut bool, &mut HashSet<String>);

/// One section on a page: its registry id and the function that draws it.
pub(crate) struct SectionDef {
    /// The id the section passes to `collapsing_section` (the search
    /// registry key and persisted collapse key).
    pub id: &'static str,
    pub draw: DrawFn,
}

/// One sub-page of a tab.
pub(crate) struct PageDef {
    pub title: &'static str,
    pub sections: &'static [SectionDef],
}

const fn s(id: &'static str, draw: DrawFn) -> SectionDef {
    SectionDef { id, draw }
}

const fn page(title: &'static str, sections: &'static [SectionDef]) -> PageDef {
    PageDef { title, sections }
}

use crate::advanced_tab::{import_export, logging, system, tmux};
use crate::ai_inspector_tab::{agent_config_section, context_section, prompt_library};
use crate::appearance_tab::{cursor_section, fonts_section};
use crate::automation_tab::{coprocesses_section, plugins_section, triggers_section};
use crate::input_tab::{keybindings, keyboard, mouse, selection, word_selection};
use crate::notifications_tab::{activity, alert_sounds, anti_idle, bell};
use crate::profiles_tab::{dynamic_sources, management};
use crate::status_bar_tab as sb;
use crate::terminal_tab::{search, semantic_history, shell, startup, unicode};
use crate::window_tab::{display, panes, performance, scrollbar, tab_bar, transparency};

const GENERAL: &[PageDef] = &[
    page(
        "Common",
        &[s("general_common", crate::quick_settings::show)],
    ),
    page(
        "Startup & Restore",
        &[
            s("terminal_startup", startup::show_startup_section),
            s(
                "arrangements_auto_restore",
                crate::arrangements_tab::show_auto_restore_section,
            ),
            s("terminal_shell", shell::show_shell_section),
        ],
    ),
    page(
        "Closing & Quitting",
        &[s(
            "general_closing",
            crate::terminal_tab::behavior::show_closing_section,
        )],
    ),
    page(
        "Selection & Clipboard",
        &[
            s("input_selection", selection::show_selection_section),
            s(
                "input_clipboard_limits",
                selection::show_clipboard_limits_section,
            ),
        ],
    ),
    page(
        "Search & Links",
        &[
            s("terminal_search", search::show_search_section),
            s(
                "terminal_semantic_history",
                semantic_history::show_semantic_history_section,
            ),
            s(
                "terminal_command_history",
                search::show_command_history_section,
            ),
        ],
    ),
    page(
        "Integration & Files",
        &[
            s("integrations_shell", |ui, st, ch, co| {
                st.show_shell_integration_section(ui, ch, co)
            }),
            s("advanced_screenshots", system::show_screenshot_section),
            s(
                "advanced_file_transfers",
                system::show_file_transfers_section,
            ),
        ],
    ),
    page(
        "Updates",
        &[s("advanced_updates", system::show_updates_section)],
    ),
];

const APPEARANCE: &[PageDef] = &[
    page(
        "Theme",
        &[
            s("appearance_theme", fonts_section::show_theme_section),
            s(
                "appearance_auto_dark_mode",
                fonts_section::show_auto_dark_mode_section,
            ),
            s(
                "terminal_command_separator",
                search::show_command_separator_section,
            ),
        ],
    ),
    page(
        "Text & Fonts",
        &[
            s("appearance_fonts", fonts_section::show_fonts_section),
            s(
                "appearance_font_variants",
                fonts_section::show_font_variants_section,
            ),
            s(
                "appearance_font_rendering",
                fonts_section::show_font_rendering_section,
            ),
        ],
    ),
    page(
        "Cursor",
        &[
            s("appearance_cursor", cursor_section::show_cursor_section),
            s(
                "appearance_cursor_locks",
                cursor_section::show_cursor_locks_section,
            ),
            s(
                "appearance_cursor_effects",
                cursor_section::show_cursor_effects_section,
            ),
            s("cursor_shader", crate::background_tab::show_cursor_shader),
        ],
    ),
    page(
        "Badge",
        &[
            s("badge_general", crate::badge_tab::show_general_section),
            s(
                "badge_appearance",
                crate::badge_tab::show_appearance_section,
            ),
            s("badge_position", crate::badge_tab::show_position_section),
            s("badge_variables", crate::badge_tab::show_variables_section),
        ],
    ),
    page(
        "Progress Bar",
        &[
            s(
                "progress_bar_general",
                crate::progress_bar_tab::show_general_section,
            ),
            s(
                "progress_bar_colors",
                crate::progress_bar_tab::show_colors_section,
            ),
        ],
    ),
];

const WINDOWS_AND_TABS: &[PageDef] = &[
    page(
        "Window",
        &[
            s("window_display", display::show_display_section),
            s(
                "window_transparency",
                transparency::show_transparency_section,
            ),
            s(
                "window_behavior",
                crate::window_tab::behavior::show_behavior_section,
            ),
        ],
    ),
    page(
        "Tab Bar",
        &[s("window_tab_bar", tab_bar::show_tab_bar_section)],
    ),
    page(
        "Tab Bar Colors",
        &[s(
            "window_tab_bar_appearance",
            tab_bar::show_tab_bar_appearance_section,
        )],
    ),
    page(
        "Scrollbar",
        &[s("window_scrollbar", scrollbar::show_scrollbar_section)],
    ),
];

const PANES: &[PageDef] = &[
    page(
        "Layout & Dividers",
        &[s("window_panes", panes::show_panes_section)],
    ),
    page(
        "Appearance",
        &[s(
            "window_pane_appearance",
            panes::show_pane_appearance_section,
        )],
    ),
    page(
        "Pane Backgrounds",
        &[s(
            "per_pane_background",
            crate::background_tab::show_pane_backgrounds,
        )],
    ),
];

const SESSIONS: &[PageDef] = &[
    page(
        "par-mux",
        &[s(
            "sessions_par_mux",
            crate::sessions_tab::show_par_mux_section,
        )],
    ),
    page("tmux", &[s("advanced_tmux", tmux::show_tmux_section)]),
    page(
        "Arrangements",
        &[
            s("arrangements_save", |ui, st, _, co| {
                crate::arrangements_tab::show_save_section(ui, st, co)
            }),
            s("arrangements_list", |ui, st, _, co| {
                crate::arrangements_tab::show_arrangements_list(ui, st, co)
            }),
        ],
    ),
];

const PROFILES: &[PageDef] = &[
    page(
        "Profiles",
        &[
            s("profiles_management", |ui, st, _, co| {
                management::show_management_section(ui, st, co)
            }),
            s("profiles_display", |ui, st, _, co| {
                management::show_display_options_section(ui, st, co)
            }),
        ],
    ),
    page(
        "Dynamic Sources",
        &[s(
            "profiles_dynamic_sources",
            dynamic_sources::show_dynamic_sources_section,
        )],
    ),
    page(
        "SSH",
        &[s("integrations_ssh", |ui, st, ch, co| {
            st.show_ssh_tab_as_section(ui, ch, co)
        })],
    ),
];

const KEYS: &[PageDef] = &[
    page(
        "Key Bindings",
        &[s(
            "input_keybindings",
            keybindings::show_keybindings_section,
        )],
    ),
    page(
        "Option/Alt",
        &[s("input_keyboard", keyboard::show_keyboard_section)],
    ),
    page(
        "Modifiers",
        &[s(
            "input_modifier_remapping",
            keyboard::show_modifier_remapping_section,
        )],
    ),
];

const POINTER: &[PageDef] = &[
    page("Mouse", &[s("input_mouse", mouse::show_mouse_section)]),
    page(
        "Word Selection",
        &[s(
            "input_word_selection",
            word_selection::show_word_selection_section,
        )],
    ),
    page(
        "Copy Mode",
        &[s("input_copy_mode", word_selection::show_copy_mode_section)],
    ),
];

const EFFECTS: &[PageDef] = &[
    page(
        "Background & Shader",
        &[
            s("background_effects", crate::background_tab::show_background),
            s("integrations_shaders", |ui, st, ch, co| {
                st.show_shaders_section(ui, ch, co)
            }),
        ],
    ),
    page(
        "Inline Images",
        &[s("inline_images", crate::effects_tab::show_inline_images)],
    ),
];

const AUTOMATION: &[PageDef] = &[
    page(
        "Triggers",
        &[s(
            "automation_triggers",
            triggers_section::show_triggers_section,
        )],
    ),
    page(
        "Snippets",
        &[
            s("snippets_list", crate::snippets_tab::show_snippets_section),
            s("snippets_variables", |ui, st, _, co| {
                crate::snippets_tab::variables_reference::show_variables_reference_section(
                    ui, st, co,
                )
            }),
        ],
    ),
    page(
        "Custom Actions",
        &[s(
            "actions_list",
            crate::actions_tab::action_list::show_actions_section,
        )],
    ),
    page(
        "Coprocesses",
        &[s(
            "automation_coprocesses",
            coprocesses_section::show_coprocesses_section,
        )],
    ),
    page(
        "Observer Scripts",
        &[s(
            "scripts_list",
            crate::scripts_tab::list::show_scripts_section,
        )],
    ),
    page(
        "Plugins",
        &[s(
            "automation_plugins",
            plugins_section::show_plugins_section,
        )],
    ),
];

const ASSISTANT: &[PageDef] = &[
    page(
        "Panel",
        &[s("ai_inspector_panel", context_section::show_panel_section)],
    ),
    page(
        "Agents",
        &[
            s("ai_inspector_agent", context_section::show_agent_section),
            s(
                "ai_inspector_custom_agents",
                agent_config_section::show_custom_agents_section,
            ),
            s(
                "agents_list",
                crate::actions_tab::agents_section::show_agents_section,
            ),
        ],
    ),
    page(
        "Agent Commands",
        &[s("agent_commands_list", |ui, st, _, co| {
            crate::actions_tab::agent_commands_section::show_agent_commands_section(ui, st, co)
        })],
    ),
    page(
        "Prompt Library",
        &[s("ai_inspector_prompt_library", |ui, st, _, co| {
            prompt_library::show_prompt_library_section(ui, st, co)
        })],
    ),
    page(
        "Permissions",
        &[s(
            "ai_inspector_permissions",
            agent_config_section::show_permissions_section,
        )],
    ),
    page(
        "Agent Usage",
        &[s(
            "status_bar_agent_usage",
            sb::agent_usage::show_agent_usage_section,
        )],
    ),
];

const ADVANCED: &[PageDef] = &[
    page(
        "Terminal Emulation",
        &[
            // Scrollback keeps the old "Behavior" section's id.
            s(
                "terminal_behavior",
                crate::terminal_tab::behavior::show_scrollback_section,
            ),
            s("terminal_unicode", unicode::show_unicode_section),
            s("notifications_anti_idle", anti_idle::show_anti_idle_section),
        ],
    ),
    page(
        "Notifications & Bell",
        &[
            s("notifications_bell", bell::show_bell_section),
            s("notifications_activity", activity::show_activity_section),
            s(
                "notifications_alert_sounds",
                alert_sounds::show_alert_sounds_section,
            ),
            s(
                "notifications_behavior",
                crate::notifications_tab::behavior::show_behavior_section,
            ),
        ],
    ),
    page(
        "Status Bar",
        &[
            s("status_bar_general", sb::general::show_general_section),
            s("status_bar_styling", sb::styling::show_styling_section),
            s(
                "status_bar_auto_hide",
                sb::auto_hide::show_auto_hide_section,
            ),
            s(
                "status_bar_widget_options",
                sb::widget_options::show_widget_options_section,
            ),
            s(
                "status_bar_poll_intervals",
                sb::poll_intervals::show_poll_intervals_section,
            ),
            s("status_bar_widgets", sb::widgets::show_widgets_section),
        ],
    ),
    page(
        "Performance & Power",
        &[s(
            "window_performance",
            performance::show_performance_section,
        )],
    ),
    page(
        "Logging",
        &[
            s("advanced_logging", logging::show_logging_section),
            s("advanced_debug_logging", system::show_debug_logging_section),
        ],
    ),
    page(
        "Import/Export",
        &[s(
            "advanced_import_export",
            import_export::show_import_export_section,
        )],
    ),
    page(
        "Security",
        &[s("advanced_security", system::show_security_section)],
    ),
];

/// The pages of `tab`, in display order.
pub(crate) fn pages(tab: SettingsTab) -> &'static [PageDef] {
    match tab {
        SettingsTab::General => GENERAL,
        SettingsTab::Appearance => APPEARANCE,
        SettingsTab::WindowsAndTabs => WINDOWS_AND_TABS,
        SettingsTab::Panes => PANES,
        SettingsTab::Sessions => SESSIONS,
        SettingsTab::Profiles => PROFILES,
        SettingsTab::Keys => KEYS,
        SettingsTab::Pointer => POINTER,
        SettingsTab::Effects => EFFECTS,
        SettingsTab::Automation => AUTOMATION,
        SettingsTab::Assistant => ASSISTANT,
        SettingsTab::Advanced => ADVANCED,
    }
}

/// The page of `tab` at `index`, clamped to the last page.
pub(crate) fn page_at(tab: SettingsTab, index: usize) -> &'static PageDef {
    let pages = pages(tab);
    &pages[index.min(pages.len() - 1)]
}

/// Section ids the host opens Settings at (UX.md B51). The root crate's
/// menu, keybinding, and drawer paths pass these to
/// `SettingsUI::open_section`; `layout_tests` checks each resolves.
pub mod deep_link {
    /// Menu "Manage Profiles", the profile drawer's Manage button.
    pub const PROFILES: &str = "profiles_management";
    /// Menu "Save Arrangement", the `save_arrangement` keybinding.
    pub const SAVE_ARRANGEMENT: &str = "arrangements_save";
}

/// Where a section id lives: `(tab, page index)`.
pub(crate) fn locate(section_id: &str) -> Option<(SettingsTab, usize)> {
    SettingsTab::all().iter().find_map(|tab| {
        pages(*tab)
            .iter()
            .position(|p| p.sections.iter().any(|s| s.id == section_id))
            .map(|page| (*tab, page))
    })
}

/// Draw one page of a tab: every section it lists, in order.
pub(crate) fn show_page(
    ui: &mut egui::Ui,
    settings: &mut SettingsUI,
    tab: SettingsTab,
    page: usize,
    changes: &mut bool,
    collapsed: &mut HashSet<String>,
) {
    for section in page_at(tab, page).sections {
        (section.draw)(ui, settings, changes, collapsed);
    }
}
