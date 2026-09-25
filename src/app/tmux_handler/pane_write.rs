//! Pane-write routing for par-mux tabs.
//!
//! Every feature that writes bytes to a terminal — snippets, custom
//! actions, trigger SendText, script WriteText, SSH quick-connect, agent
//! commands, keep-alives — routes through this seam. In a mux tab the
//! on-screen pane is a daemon mirror with no PTY, and `tab.terminal` is a
//! live hidden login shell (spawned by `Tab::new` before the layout swaps
//! in the mirrors): a local write is either silently dropped or runs a
//! command invisibly in the wrong shell and directory.
//!
//! The helpers return `true` when the bytes went to the daemon; callers
//! keep their existing local write on `false`, so non-mux tabs behave
//! exactly as before. This mirrors the read-side seam
//! (`Tab::try_with_read_terminal`) and the input seam
//! (`send_input_via_tmux`).

use crate::app::window_state::WindowState;
use crate::pane::PaneId;
use crate::tab::{Tab, TabId};

impl WindowState {
    /// Route a pane-targeted write to the daemon when `pane_id` inside
    /// `tab_id` is a par-mux mirror pane. Returns `false` when the pane is
    /// local (or no transport is attached) so the caller falls back to its
    /// own terminal write.
    #[cfg_attr(not(feature = "mux"), allow(unused_variables))]
    pub(crate) fn route_mux_pane_write(&self, tab_id: TabId, pane_id: PaneId, data: &[u8]) -> bool {
        #[cfg(feature = "mux")]
        if let Some(transport) = &self.tmux_state.transport
            && let Some(pane) = self.tmux_state.tmux_pane_in_tab(tab_id, pane_id)
        {
            super::notifications::mux::route_literal_bytes(&**transport, Some(pane), data);
            return true;
        }
        false
    }

    /// Tab-level variant: routes to the daemon pane behind the tab's
    /// focused pane. Write sites that hold only a `&Tab` start here.
    pub(crate) fn route_mux_tab_write(&self, tab: &Tab, data: &[u8]) -> bool {
        let Some(pane) = tab.pane_manager().and_then(|pm| pm.focused_pane()) else {
            return false;
        };
        self.route_mux_pane_write(tab.id, pane.id, data)
    }
}

#[cfg(all(test, feature = "mux"))]
pub(crate) mod tests {
    use super::*;
    use crate::app::tmux_handler::notifications::mux::{MuxAttachPending, tests as mux_tests};
    use crate::app::tmux_handler::tmux_state::TmuxTransport;
    use crate::app::window_state::WindowState;
    use crate::config::Config;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    /// Records every control-mode command sent through it. The test keeps
    /// the `Arc` handle after the transport itself is boxed into
    /// `tmux_state.transport`, so assertions read exactly what the app
    /// would have sent to the daemon — no shell-echo timing involved.
    pub(crate) struct RecordingTransport {
        sent: Arc<Mutex<Vec<String>>>,
    }

    impl RecordingTransport {
        pub(crate) fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let sent = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    sent: Arc::clone(&sent),
                },
                sent,
            )
        }
    }

    impl TmuxTransport for RecordingTransport {
        fn drain(
            &self,
        ) -> (
            Vec<par_term_emu_core_rust::tmux_control::TmuxNotification>,
            bool,
        ) {
            (Vec::new(), false)
        }

        fn send_command(&self, command: &str) -> std::io::Result<Vec<String>> {
            self.sent.lock().unwrap().push(command.to_string());
            Ok(Vec::new())
        }
    }

    fn test_runtime() -> Arc<tokio::runtime::Runtime> {
        Arc::new(
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("build test runtime"),
        )
    }

    fn hex_of(text: &str) -> String {
        text.bytes()
            .map(|b| format!("{b:02x}"))
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Attach to a fresh daemon, create window @0, pump until its pane is
    /// mapped, then swap in the recording transport. Returns the state
    /// ready for a write, the recorder handle, and the owning (tab, pane).
    pub(crate) fn mux_state_with_recorder(
        tag: &str,
        config: Config,
    ) -> (WindowState, Arc<Mutex<Vec<String>>>, TabId, PaneId) {
        let path = mux_tests::socket_path(tag);
        mux_tests::spawn_daemon(&path);

        let core_client = par_term_emu_core_rust::mux::MuxClient::connect(&path).expect("connect");
        let (tx, rx) = std::sync::mpsc::channel();
        tx.send(Ok(core_client)).unwrap();
        drop(tx);
        let mut ws = WindowState::new(config, test_runtime());
        ws.tmux_state.mux_attach_pending = Some(MuxAttachPending {
            name: "writetest".to_string(),
            rx,
        });
        ws.poll_mux_attach();
        assert!(ws.tmux_state.transport.is_some(), "attach must install");

        ws.handle_tmux_window_add(0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !ws.tmux_state.tmux_pane_owners.contains_key(&0) {
            assert!(
                Instant::now() < deadline,
                "window @0 never got a mapped pane"
            );
            ws.tmux_state
                .transport
                .as_ref()
                .expect("transport")
                .send_command("refresh-client -t %0 -C 80x24")
                .expect("size push broadcasts %layout-change");
            ws.check_mux_notifications();
            std::thread::sleep(Duration::from_millis(50));
        }
        let (tab_id, pane) = ws.tmux_state.tmux_pane_owners[&0];
        ws.tab_manager.switch_to(tab_id);

        let (transport, sent) = RecordingTransport::new();
        ws.tmux_state.transport = Some(Box::new(transport));
        (ws, sent, tab_id, pane)
    }

    #[test]
    fn tab_write_routes_focused_mirror_pane() {
        let (ws, sent, tab_id, pane) =
            mux_state_with_recorder("pane-write-helper", Config::default());
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");

        assert!(
            ws.route_mux_tab_write(tab, b"helper-needle"),
            "the focused mirror pane must route to the daemon"
        );
        assert_eq!(
            *sent.lock().unwrap(),
            vec![format!("send-keys -t %0 -H {}", hex_of("helper-needle"))],
            "the bytes must go to the daemon as one literal send-keys"
        );
        let _ = pane;
    }

    #[test]
    fn ctrl_l_clear_routes_to_the_daemon_pane() {
        let (ws, sent, tab_id, pane) =
            mux_state_with_recorder("pane-write-ctrl-l", Config::default());
        let _ = (tab_id, pane);

        ws.send_clear_screen_sequence();
        assert_eq!(
            *sent.lock().unwrap(),
            vec!["send-keys -t %0 -H 0c".to_string()],
            "Ctrl+L must clear the daemon pane, not the hidden shell"
        );
    }

    #[test]
    fn snippet_write_routes_to_the_daemon_pane() {
        let mut config = Config::default();
        config.snippets.push(
            serde_yaml_ng::from_str::<crate::config::snippets::SnippetConfig>(
                "id: snip\ntitle: Snip\ncontent: mux-write-needle",
            )
            .expect("snippet config"),
        );
        let (mut ws, sent, tab_id, pane) = mux_state_with_recorder("pane-write-snippet", config);
        let _ = (tab_id, pane);

        assert!(ws.execute_snippet("snip"), "snippet must execute");
        assert_eq!(
            *sent.lock().unwrap(),
            vec![format!("send-keys -t %0 -H {}", hex_of("mux-write-needle"))],
            "the snippet must reach the daemon pane, not the hidden shell"
        );
    }

    #[test]
    fn insert_text_action_routes_to_the_daemon_pane() {
        use crate::config::snippets::CustomActionConfig;
        let mut config = Config::default();
        config.actions.push(CustomActionConfig::InsertText {
            id: "act".to_string(),
            title: "Act".to_string(),
            text: "insert-needle".to_string(),
            variables: Default::default(),
            keybinding: None,
            prefix_char: None,
            keybinding_enabled: true,
            description: None,
        });
        let (mut ws, sent, tab_id, pane) = mux_state_with_recorder("pane-write-insert", config);
        let _ = (tab_id, pane);

        assert!(ws.execute_custom_action("act"), "action must execute");
        assert_eq!(
            *sent.lock().unwrap(),
            vec![format!("send-keys -t %0 -H {}", hex_of("insert-needle"))],
            "InsertText must reach the daemon pane, not the hidden shell"
        );
    }

    #[test]
    fn local_tab_writes_do_not_route() {
        let mut ws = WindowState::new(Config::default(), test_runtime());
        ws.tab_manager
            .new_tab(&Config::default(), test_runtime(), false, None)
            .expect("local tab");
        let tab = ws.tab_manager.active_tab().expect("tab");
        assert!(
            !ws.route_mux_tab_write(tab, b"hello"),
            "no transport attached: the caller must keep its local write"
        );
    }
}
