//! Agent-launcher actions: `launch-agent:<id>` palette dispatch.
//!
//! A configured agent (the `agents:` config list) launches where the user's
//! focus is — a daemon-side split when a par-mux pane is focused (so the
//! agent roster's hooks see it), otherwise a local tab with the command
//! typed after the shell initializes (the snippet NewTab path).

use crate::app::window_state::WindowState;
use par_term_config::agent_launcher::AgentLaunchConfig;

impl WindowState {
    /// Dispatch a `launch-agent:<id>` / `launch-agent-autonomous:<id>`
    /// action. Unknown ids warn and no-op — a stale palette row must not
    /// launch the wrong agent.
    pub(crate) fn launch_agent_by_id(&mut self, id: &str, autonomous: bool) -> bool {
        let Some(agent) = self
            .config
            .load()
            .agents
            .iter()
            .find(|agent| agent.id == id)
            .cloned()
        else {
            log::warn!("launch-agent: no configured agent with id '{id}'");
            self.show_toast(format!("No configured agent '{id}'"));
            return false;
        };
        self.launch_agent(&agent, autonomous)
    }

    /// Dispatch the `launch-default-agent` action: the first entry marked
    /// `default`, launched plain.
    pub(crate) fn launch_default_agent(&mut self) -> bool {
        let Some(agent) =
            par_term_config::agent_launcher::default_agent(&self.config.load().agents).cloned()
        else {
            log::warn!("launch-default-agent: no default agent configured");
            self.show_toast("No default agent configured".to_string());
            return false;
        };
        self.launch_agent(&agent, false)
    }

    /// Launch one configured agent. The mux arm consumes the attempt when a
    /// daemon pane is focused — success or failure — so a local tab never
    /// appears as a surprise beside a daemon window.
    fn launch_agent(&mut self, agent: &AgentLaunchConfig, autonomous: bool) -> bool {
        let command_line = agent.command_line(autonomous);
        #[cfg(feature = "mux")]
        {
            crate::debug_info!("TAB_ACTION", "launch agent '{}' via palette", agent.name);
            use crate::app::tmux_handler::MuxLaunchOutcome;
            match self.launch_agent_via_mux(&command_line) {
                MuxLaunchOutcome::NotMux => {}
                MuxLaunchOutcome::Launched => return true,
                MuxLaunchOutcome::Failed => return false,
            }
        }
        #[cfg(not(feature = "mux"))]
        crate::debug_info!("TAB_ACTION", "launch agent '{}' via palette", agent.name);
        self.execute_new_tab_action(Some(command_line), agent.name.clone())
    }
}
