//! Per-row palette metadata (UX.md OV6): the category a row sits under and
//! the one-line description drawn beneath its label.
//!
//! Both are derived from the row's action id rather than stored on
//! [`super::catalog::PaletteEntry`]: runtime rows are built in half a dozen
//! places (plugins, agents, crashes, roster, par-mux), and an id-keyed
//! lookup gives every one of them a category without touching those sites.
//! Built-in categories reuse the F1 help filing ([`crate::help_content::section_for`])
//! so the palette and help agree on where an action lives; only the names
//! are mapped onto the OV6 set.

/// The OV6 palette categories, in display order.
pub(crate) const CATEGORIES: &[&str] = &[
    "Window", "Tab", "Pane", "Session", "Profiles", "View", "Agents", "Edit", "Terminal", "Other",
];

/// The category a palette row sits under.
pub(crate) fn category(action_id: &str) -> &'static str {
    // Runtime rows first: their ids are prefixes, and none of them is an
    // F1 help row.
    if action_id.starts_with("agent-cmd:")
        || action_id.starts_with("launch-agent")
        || action_id.starts_with("triage-crash:")
        || action_id.starts_with("agent-roster-focus:")
        || matches!(
            action_id,
            "focus_next_attention_agent" | "toggle_agent_usage_panel" | "toggle_ai_inspector"
        )
    {
        return "Agents";
    }
    if action_id.starts_with("attach_mux_session:") || action_id == "mux-restart-pane" {
        return "Session";
    }
    if action_id.starts_with("move_tab_to_window:") {
        return "Tab";
    }
    if action_id.starts_with("plugin-action:") {
        return "Other";
    }
    match crate::help_content::section_for(action_id) {
        "Window" => "Window",
        "Tab" => "Tab",
        "Pane" => "Pane",
        "Sessions & Profiles" => {
            if action_id.contains("profile") || action_id == "ssh_quick_connect" {
                "Profiles"
            } else {
                "Session"
            }
        }
        "Edit & Search" => "Edit",
        "Scrolling" | "Terminal" => "Terminal",
        "View" => "View",
        _ => "Other",
    }
}

/// The one-line description drawn under a row's label, if one exists.
pub(crate) fn description(action_id: &str) -> Option<&'static str> {
    if let Some(d) = runtime_description(action_id) {
        return Some(d);
    }
    if let Some(n) = action_id.strip_prefix("switch_to_tab_") {
        return (n.len() == 1).then_some("Focus the tab at that position in the tab bar");
    }
    if let Some(n) = action_id.strip_prefix("switch_to_window_") {
        return (n.len() == 1).then_some("Focus the window holding that number");
    }
    DESCRIPTIONS
        .iter()
        .find(|(id, _)| *id == action_id)
        .map(|(_, d)| *d)
}

fn runtime_description(action_id: &str) -> Option<&'static str> {
    Some(if action_id.starts_with("launch-agent-autonomous:") {
        "Launch this agent with its autonomy flags"
    } else if action_id.starts_with("launch-agent:") {
        "Launch this configured agent where the focus is"
    } else if action_id == "launch-default-agent" {
        "Launch the agent marked as default"
    } else if action_id.starts_with("agent-cmd:") {
        "Run a command an agent or you saved"
    } else if action_id.starts_with("triage-crash:") {
        "Send the captured crash to your default agent"
    } else if action_id.starts_with("agent-roster-focus:") {
        "Focus this agent's pane"
    } else if action_id.starts_with("attach_mux_session:") {
        "Attach this window to the running par-mux session"
    } else if action_id == "mux-restart-pane" {
        "Restart the focused par-mux pane's process"
    } else if action_id.starts_with("move_tab_to_window:") {
        "Move the active tab into that window"
    } else if action_id.starts_with("plugin-action:") {
        "Provided by a plugin"
    } else {
        return None;
    })
}

/// One line per built-in action, in the vocabulary of UX.md T1-T8 (a
/// "session" is only a par-mux or tmux session).
const DESCRIPTIONS: &[(&str, &str)] = &[
    // Window
    ("new_window", "Open a new window"),
    ("close_window", "Close this window and all its tabs"),
    (
        "close_tab_or_window",
        "Close the active tab, or the window when it is the last tab",
    ),
    ("next_window", "Focus the next window"),
    ("prev_window", "Focus the previous window"),
    ("toggle_fullscreen", "Enter or leave full screen"),
    (
        "maximize_vertically",
        "Stretch the window to the full screen height",
    ),
    (
        "toggle_always_on_top",
        "Keep this window above other windows",
    ),
    (
        "toggle_tree_picker",
        "Jump to any window, tab, or pane by name",
    ),
    (
        "save_arrangement",
        "Save the current windows as a named arrangement",
    ),
    ("quit", "Quit par-term"),
    // Tab
    ("new_tab", "Open a new tab"),
    ("close_tab", "Close the active tab"),
    (
        "duplicate_tab",
        "Open a copy of this tab: same directory and pane layout",
    ),
    ("next_tab", "Focus the tab to the right"),
    ("prev_tab", "Focus the tab to the left"),
    ("last_tab", "Toggle back to the previously active tab"),
    ("go_to_last_tab", "Focus the rightmost tab"),
    ("move_tab_left", "Move the active tab one place left"),
    ("move_tab_right", "Move the active tab one place right"),
    ("rename_tab", "Rename the active tab"),
    ("close_other_tabs", "Close every tab except the active one"),
    (
        "close_tabs_to_right",
        "Close every tab right of the active one",
    ),
    (
        "reopen_closed_tab",
        "Bring back the most recently closed tab",
    ),
    (
        "move_tab_to_new_window",
        "Move the active tab, with its shell, into a new window",
    ),
    (
        "move_tab_to_window_picker",
        "Choose another window to move the active tab into",
    ),
    // Pane
    (
        "split_right",
        "Split the focused pane, new pane on the right",
    ),
    ("split_down", "Split the focused pane, new pane below"),
    ("split_left", "Split the focused pane, new pane on the left"),
    ("split_up", "Split the focused pane, new pane above"),
    ("close_pane", "Close the focused pane"),
    ("navigate_pane_left", "Focus the pane to the left"),
    ("navigate_pane_right", "Focus the pane to the right"),
    ("navigate_pane_up", "Focus the pane above"),
    ("navigate_pane_down", "Focus the pane below"),
    ("select_pane_hint", "Pick a pane by typing its letter"),
    ("resize_pane_left", "Move the focused pane's divider left"),
    ("resize_pane_right", "Move the focused pane's divider right"),
    ("resize_pane_up", "Move the focused pane's divider up"),
    ("resize_pane_down", "Move the focused pane's divider down"),
    (
        "enter_resize_mode",
        "Resize panes with the arrow keys until Esc",
    ),
    (
        "swap_pane_left",
        "Swap the focused pane with its left neighbor",
    ),
    (
        "swap_pane_right",
        "Swap the focused pane with its right neighbor",
    ),
    ("swap_pane_up", "Swap the focused pane with the pane above"),
    (
        "swap_pane_down",
        "Swap the focused pane with the pane below",
    ),
    (
        "toggle_pane_zoom",
        "Fill the tab with the focused pane, or restore",
    ),
    ("next_pane", "Focus the next pane"),
    ("prev_pane", "Focus the previous pane"),
    ("last_pane", "Return to the previously focused pane"),
    (
        "restart_pane",
        "Restart the focused pane's program in place",
    ),
    (
        "equalize_panes",
        "Give every pane in the tab an equal share",
    ),
    ("cycle_layout", "Step through the pane layout presets"),
    ("rename_pane", "Rename the focused pane"),
    (
        "promote_pane_to_tab",
        "Move the focused pane into its own tab",
    ),
    (
        "demote_tab_to_pane",
        "Merge the active tab into another tab as a pane",
    ),
    (
        "toggle_broadcast_input",
        "Send typing to every pane in the tab at once",
    ),
    (
        "toggle_pane_broadcast",
        "Include or exclude the focused pane from broadcast",
    ),
    // Session
    (
        "toggle_session_picker",
        "Attach, switch, create, or end par-mux and tmux sessions",
    ),
    (
        "new_mux_session",
        "Create a par-mux session and attach this window",
    ),
    (
        "detach",
        "Detach this window from its par-mux session; it keeps running",
    ),
    (
        "focus_next_attention_agent",
        "Jump to the next agent that is blocked or finished",
    ),
    // Profiles
    ("toggle_profile_drawer", "Show or hide the profile drawer"),
    (
        "reload_dynamic_profiles",
        "Fetch remote profile sources again",
    ),
    ("ssh_quick_connect", "Connect to an SSH host"),
    // View
    ("toggle_help", "Show every shortcut"),
    ("toggle_command_palette", "Search every action"),
    ("open_settings", "Open the settings window"),
    ("reload_config", "Reload the configuration file"),
    ("toggle_menu", "Open the application menu"),
    ("toggle_fps_overlay", "Show or hide the frame-rate overlay"),
    ("increase_font_size", "Make the text bigger"),
    ("decrease_font_size", "Make the text smaller"),
    ("reset_font_size", "Return to the configured font size"),
    (
        "cycle_cursor_style",
        "Step through block, beam, and underline",
    ),
    (
        "toggle_background_shader",
        "Turn the background shader on or off",
    ),
    ("toggle_cursor_shader", "Turn the cursor shader on or off"),
    (
        "cycle_background_shader",
        "Switch to the next background shader",
    ),
    (
        "toggle_shader_animation",
        "Pause or resume shader animation",
    ),
    (
        "toggle_shader_readability_mode",
        "Dim the background shader so text reads clearly",
    ),
    (
        "toggle_throughput_mode",
        "Render less often during heavy output to save CPU",
    ),
    // Agents
    ("toggle_ai_inspector", "Show or hide the assistant panel"),
    (
        "toggle_agent_usage_panel",
        "Show coding-agent usage and limits",
    ),
    // Edit
    ("toggle_search", "Find text in the terminal"),
    ("select_all", "Select the whole buffer"),
    ("paste_special", "Transform the clipboard before pasting"),
    (
        "toggle_clipboard_history",
        "Paste from recent clipboard entries",
    ),
    (
        "toggle_command_history",
        "Search and rerun previous commands",
    ),
    ("toggle_copy_mode", "Select and copy text with the keyboard"),
    ("enter_copy_mode", "Select and copy text with the keyboard"),
    // Terminal
    ("clear_scrollback", "Discard the focused pane's scrollback"),
    ("clear_screen", "Clear the focused pane's screen"),
    ("scroll_up_page", "Scroll up one page"),
    ("scroll_down_page", "Scroll down one page"),
    ("scroll_to_top", "Scroll to the start of the scrollback"),
    ("scroll_to_bottom", "Scroll back to the live output"),
    ("scroll_to_previous_mark", "Jump to the previous command"),
    ("scroll_to_next_mark", "Jump to the next command"),
    (
        "toggle_session_logging",
        "Start or stop recording this pane's output",
    ),
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command_palette::catalog::build_catalog;

    #[test]
    fn every_catalog_action_has_a_description() {
        let missing: Vec<String> = build_catalog()
            .into_iter()
            .filter(|e| description(&e.action_id).is_none())
            .map(|e| e.action_id)
            .collect();
        assert!(
            missing.is_empty(),
            "palette rows with no description line (add them to DESCRIPTIONS): {missing:?}"
        );
    }

    #[test]
    fn every_category_is_in_the_ov6_set() {
        for entry in build_catalog() {
            let category = category(&entry.action_id);
            assert!(
                CATEGORIES.contains(&category),
                "{} files under unknown category {category}",
                entry.action_id
            );
        }
    }

    #[test]
    fn runtime_rows_file_under_their_owner() {
        assert_eq!(category("launch-agent:claude"), "Agents");
        assert_eq!(category("agent-cmd:x"), "Agents");
        assert_eq!(category("triage-crash:3"), "Agents");
        assert_eq!(category("agent-roster-focus:7"), "Agents");
        assert_eq!(category("attach_mux_session:d/s"), "Session");
        assert_eq!(category("mux-restart-pane"), "Session");
        assert_eq!(category("move_tab_to_window:2"), "Tab");
        assert_eq!(category("split_right"), "Pane");
        assert_eq!(category("toggle_profile_drawer"), "Profiles");
        assert_eq!(category("toggle_session_picker"), "Session");
        // Built-ins follow the F1 help filing: fullscreen is View there.
        assert_eq!(category("toggle_fullscreen"), "View");
        assert_eq!(category("new_window"), "Window");
    }

    #[test]
    fn descriptions_are_one_short_line() {
        for (id, d) in DESCRIPTIONS {
            assert!(!d.contains('\n'), "{id}: one line");
            assert!(
                d.len() <= 70,
                "{id}: {} chars is too long for a row",
                d.len()
            );
        }
    }
}
