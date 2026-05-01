//! NewTab action handler: tab creation with delayed command write.

use crate::app::window_state::WindowState;

/// Delay before writing the command to a newly created tab's terminal,
/// to allow the shell to initialize.
pub(crate) const NEW_TAB_COMMAND_DELAY_MS: u64 = 200;

impl WindowState {
    /// Execute a NewTab custom action.
    ///
    /// Creates a new tab and optionally writes a command to it after a short delay
    /// to allow the shell to initialize.
    pub(crate) fn execute_new_tab_action(
        &mut self,
        command: Option<String>,
        title: String,
    ) -> bool {
        let tab_count_before = self.tab_manager.tab_count();
        self.new_tab();

        let opened_new_tab = self.tab_manager.tab_count() > tab_count_before;
        if !opened_new_tab {
            log::warn!("NewTab action '{}' did not open a tab", title);
            return false;
        }

        if let Some(command) = command.filter(|cmd| !cmd.trim().is_empty())
            && let Some(tab) = self.tab_manager.active_tab()
        {
            let text_with_nl = format!("{}\n", command);
            let terminal = std::sync::Arc::clone(&tab.terminal);
            let title = title.clone();

            std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(NEW_TAB_COMMAND_DELAY_MS));

                // try_write: background thread; on contention skip the write.
                // Shell may not be ready yet -- user can re-run the action.
                if let Ok(term) = terminal.try_write()
                    && let Err(e) = term.write(text_with_nl.as_bytes())
                {
                    log::error!("NewTab action '{}' write failed: {}", title, e);
                }
            });
        }

        true
    }
}
