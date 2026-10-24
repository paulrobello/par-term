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
}
