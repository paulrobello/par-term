//! SplitPane action handler: pane splitting with delayed command write.

use crate::app::window_state::WindowState;
use crate::config::snippets::ActionSplitDirection;

impl WindowState {
    /// Execute a SplitPane custom action.
    ///
    /// Splits the current pane in the specified direction and optionally writes a command
    /// to the new pane after a delay.
    pub(crate) fn execute_split_pane_action(
        &mut self,
        direction: ActionSplitDirection,
        command: Option<String>,
        command_is_direct: bool,
        focus_new_pane: bool,
        delay_ms: u64,
        split_percent: u8,
        title: String,
    ) -> bool {
        let pane_direction = match direction {
            ActionSplitDirection::Horizontal => crate::pane::SplitDirection::Horizontal,
            ActionSplitDirection::Vertical => crate::pane::SplitDirection::Vertical,
        };

        crate::debug_info!(
            "TAB_ACTION",
            "SplitPane action '{}' direction={:?} focus_new={} direct={}",
            title,
            pane_direction,
            focus_new_pane,
            command_is_direct
        );

        // For direct commands, parse argv and pass as the pane's initial process.
        let initial_command = if command_is_direct {
            command.as_deref().map(|cmd_str| {
                let mut parts = cmd_str.split_whitespace();
                let cmd = parts.next().unwrap_or("").to_string();
                let args: Vec<String> = parts.map(|s| s.to_string()).collect();
                (cmd, args)
            })
        } else {
            None
        };

        let new_pane_id =
            self.split_pane_direction(pane_direction, focus_new_pane, initial_command, split_percent);

        // For shell-mode commands, send text to the new pane after a delay.
        if !command_is_direct && let (Some(pane_id), Some(text)) = (new_pane_id, command) {
            let text_with_nl = format!("{}\n", text);
            if let Some(tab) = self.tab_manager.active_tab()
                && let Some(pm) = tab.pane_manager()
                && let Some(pane) = pm.get_pane(pane_id)
            {
                let terminal = std::sync::Arc::clone(&pane.terminal);
                std::thread::spawn(move || {
                    if delay_ms > 0 {
                        std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                    }
                    // try_write: background thread; on contention skip the write.
                    // Shell may not be ready -- user can retry the keybinding.
                    if let Ok(term) = terminal.try_write()
                        && let Err(e) = term.write(text_with_nl.as_bytes())
                    {
                        log::error!(
                            "SplitPane action '{}' write failed for pane {}: {}",
                            title,
                            pane_id,
                            e
                        );
                    }
                });
            }
        }

        new_pane_id.is_some()
    }
}
