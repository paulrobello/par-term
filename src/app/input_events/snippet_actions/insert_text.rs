//! InsertText action handler: variable substitution and terminal write.

use crate::app::window_state::WindowState;
use std::collections::HashMap;

impl WindowState {
    /// Execute an InsertText custom action.
    ///
    /// Substitutes variables in the text, then writes the result to the active terminal.
    pub(crate) fn execute_insert_text_action(
        &mut self,
        text: String,
        variables: HashMap<String, String>,
    ) -> bool {
        // Substitute variables
        let substituted_text =
            match crate::snippets::VariableSubstitutor::new().substitute(&text, &variables) {
                Ok(content) => content,
                Err(e) => {
                    log::error!("Failed to substitute variables in action: {}", e);
                    self.show_toast(format!("Action Error: {}", e));
                    return false;
                }
            };

        // Write to the active terminal
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // try_lock: intentional -- execute_custom_action runs from keybinding
            // handler in sync event loop. On miss: the action text is not written.
            // Logged as an error so the user is aware; they can retry the keybinding.
            if let Ok(terminal) = tab.terminal.try_write() {
                if let Err(e) = terminal.write(substituted_text.as_bytes()) {
                    log::error!("Failed to write action text to terminal: {}", e);
                    return false;
                }

                log::info!("Executed insert text action");
                return true;
            } else {
                log::error!("Failed to lock terminal for action execution");
                return false;
            }
        }

        false
    }
}
