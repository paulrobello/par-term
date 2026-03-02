//! Search state methods for the copy mode state machine.

use super::types::{CopyModeState, SearchDirection};

impl CopyModeState {
    // ========================================================================
    // Search
    // ========================================================================

    /// Start search input mode
    pub fn start_search(&mut self, direction: SearchDirection) {
        self.is_searching = true;
        self.search_direction = direction;
        self.search_query.clear();
    }

    /// Add a character to the search query
    pub fn search_input(&mut self, ch: char) {
        self.search_query.push(ch);
    }

    /// Remove the last character from the search query
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
    }

    /// Cancel search mode without executing
    pub fn cancel_search(&mut self) {
        self.is_searching = false;
        self.search_query.clear();
    }
}
