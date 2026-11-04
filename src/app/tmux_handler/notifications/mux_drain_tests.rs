//! Synthetic-injection tests for the par-mux drain (`mux_drain.rs`),
//! split out to keep the drain under the 800-line production gate.

#[cfg(test)]
mod tests {
    use crate::app::tmux_handler::tmux_state::TmuxTransport;
    use par_term_emu_core_rust::tmux_control::{TmuxControlParser, TmuxNotification as Core};
    use std::cell::RefCell;

    struct NotificationTransport(RefCell<Vec<Core>>);

    impl TmuxTransport for NotificationTransport {
        fn drain(&self) -> (Vec<Core>, bool) {
            (self.0.take(), false)
        }

        fn send_command(&self, _command: &str) -> std::io::Result<Vec<String>> {
            panic!("an untargeted notification must not send a pane command")
        }
    }

    /// A pin-core drain: the telemetry push arrives as `Unknown { line }`
    /// (the newer core's named variant emits the same wire line), the
    /// reaction is the `list-agents` re-query, and the reply's rows land
    /// in the roster cache.
    struct TelemetryTransport {
        parser: RefCell<TmuxControlParser>,
        commands: std::rc::Rc<std::cell::RefCell<Vec<String>>>,
    }

    impl TmuxTransport for TelemetryTransport {
        fn drain(&self) -> (Vec<Core>, bool) {
            (
                self.parser
                    .borrow_mut()
                    .parse(b"%agent-telemetry-changed %5 claude\n"),
                false,
            )
        }

        fn send_command(&self, command: &str) -> std::io::Result<Vec<String>> {
            self.commands.borrow_mut().push(command.to_string());
            Ok(vec!["%5 claude question hook".to_string()])
        }
    }

    #[test]
    fn telemetry_push_triggers_a_roster_refetch() {
        let commands = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
        let mut ws = crate::app::tmux_handler::notifications::mux::tests::manners_state();
        ws.tmux_state.transport = Some(Box::new(TelemetryTransport {
            parser: RefCell::new(TmuxControlParser::new(true)),
            commands: std::rc::Rc::clone(&commands),
        }));
        assert!(
            ws.check_mux_notifications_with(|_, _, _| ()),
            "a telemetry push is a roster change"
        );
        assert_eq!(
            commands.borrow().as_slice(),
            ["list-agents"],
            "the push carries no values, so the reaction is the re-fetch"
        );
        let snapshot: Vec<_> = ws
            .tmux_state
            .agent_roster
            .iter()
            .map(|e| (e.pane, e.state.clone()))
            .collect();
        assert_eq!(
            snapshot,
            vec![(5, "question".to_string())],
            "the fresh reply replaced the roster wholesale"
        );
    }

    #[test]
    fn mux_notification_drain_delivers_decoded_text_once_without_a_pane() {
        let mut parser = TmuxControlParser::new(true);
        let mut ws = crate::app::tmux_handler::notifications::mux::tests::manners_state();
        ws.tmux_state.transport = Some(Box::new(NotificationTransport(RefCell::new(
            parser.parse(b"%notification YnVpbGQgZmFpbGVk body=c2VlIGxvZw==\n%notification ZG9uZQ==\n%notification %%%\n"),
        ))));
        let mut delivered = Vec::new();
        assert!(ws.check_mux_notifications_with(|_, title, body| {
            delivered.push((title.to_owned(), body.to_owned()));
        }));
        assert_eq!(
            delivered,
            [
                ("build failed".into(), "see log".into()),
                ("done".into(), String::new())
            ]
        );
        assert!(!ws.check_mux_notifications_with(|_, _, _| panic!("delivered twice")));
    }

    /// Criterion 2 at the handler level (card "Mux reconnect: replay
    /// daemon event ring via `events --since`"): lifecycle lines replayed
    /// from the ring are queued as core notifications by the attach tail
    /// (`mux_replay_pending`) and the drain runs them through the SAME
    /// partition/handler path as live pushes — a `%pane-exited` that
    /// arrived during a disconnect sets `mux_exited_panes`, the state the
    /// exited-pane overlay and the restart key read.
    #[test]
    fn replayed_pane_exit_rides_the_drain_into_mux_exited_panes() {
        let mut parser = TmuxControlParser::new(true);
        let mut ws = crate::app::tmux_handler::notifications::mux::tests::manners_state();
        // The transport drains nothing: every notification below is the
        // replay, not a live push.
        ws.tmux_state.transport = Some(Box::new(NotificationTransport(RefCell::new(Vec::new()))));
        ws.tmux_state.mux_replay_pending = parser
            .parse(b"%pane-exited %3 7\n%agent-state-changed %3 claude working source=hook\n");

        assert!(
            ws.check_mux_notifications_with(|_, _, _| ()),
            "a replayed exit changes roster/overlay state"
        );
        assert!(
            ws.tmux_state.mux_exited_panes.contains_key(&3),
            "the replayed exit is reflected in the UI state: {:?}",
            ws.tmux_state.mux_exited_panes
        );
        assert_eq!(
            ws.tmux_state
                .agent_roster
                .iter()
                .find(|e| e.pane == 3)
                .map(|e| e.state.as_str()),
            Some("working"),
            "the replayed agent push feeds the roster cache"
        );
        // Consumed once: a second drain re-runs nothing.
        assert!(
            ws.tmux_state.mux_replay_pending.is_empty(),
            "the replay batch is consumed by its drain"
        );
    }

    /// Card 01a1a8b8: a liveness recovery ("responding again") must not
    /// clear the attach's stale-version verdict. An old daemon responds
    /// fine while still missing every daemon-side fix; `Connected` on the
    /// chip erased the only persistent stale-daemon signal while the
    /// mismatched daemon's tree was already grafted into tabs.
    #[test]
    fn responding_again_does_not_clear_version_stale() {
        let mut ws = crate::app::tmux_handler::notifications::mux::tests::manners_state();
        ws.tmux_state.mux_version_stale = true;
        ws.tmux_state.mux_health = crate::session_chip::MuxHealth::Stale;
        ws.apply_mux_health_event("par-mux daemon responding again".to_string());
        assert_eq!(
            ws.tmux_state.mux_health,
            crate::session_chip::MuxHealth::Stale,
            "a stale-version daemon that recovered liveness stays Stale"
        );
    }

    /// The control for the stale-guard: without a version mismatch the
    /// recovery transition still returns the chip to Connected.
    #[test]
    fn responding_again_recovers_to_connected_when_not_stale() {
        let mut ws = crate::app::tmux_handler::notifications::mux::tests::manners_state();
        ws.tmux_state.mux_version_stale = false;
        ws.tmux_state.mux_health = crate::session_chip::MuxHealth::Unresponsive;
        ws.apply_mux_health_event("par-mux daemon responding again".to_string());
        assert_eq!(
            ws.tmux_state.mux_health,
            crate::session_chip::MuxHealth::Connected
        );
    }
}
