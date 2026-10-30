//! par-mux tests that daemon pane output reaches the per-pane hooks a local tab
//! feeds: triggers, session logging, plugin and tab-script bell events.
//!
//! Split from `mux.rs`'s test module (AUDIT.md ARC-003); the shared
//! daemon helpers stay in `mux::tests`.

use super::mux::tests::{attach_test, connect, socket_path, spawn_daemon};
use super::mux_test_seams::{DAEMON_DEADLINE, quiesce};
use std::time::{Duration, Instant};

/// Triggers are scanned by the PTY reader thread — which a mux mirror
/// pane does not have. Output routed into a mirror via
/// `handle_tmux_output` used to reach the grid but never the trigger
/// registry, and `check_trigger_actions` polled only `tab.terminal`
/// (the hidden shell), so a pattern printed in a mux pane fired
/// nothing (card 01a0d9b559487e42a651a18e93dfce93, criterion 1).
///
/// Proves the whole chain through the real routing and dispatch paths:
/// attach installs mirror panes carrying the config's trigger registry,
/// a `%output` chunk seeded through `handle_tmux_output` is scanned on
/// the mirror terminal, and `check_trigger_actions` polls every pane of
/// the active tab — the RunCommand lands in the confirmation queue
/// (prompt_before_run keeps it side-effect-free for the test).
#[test]
fn triggers_fire_on_mux_pane_output() {
    let path = socket_path("ws-triggers");
    spawn_daemon(&path);

    // A config carrying one trigger whose pattern only this test
    // prints. RunCommand + prompt_before_run queues a dialog action
    // instead of spawning a process.
    let mut config = crate::config::Config::default();
    config.automation.triggers = vec![par_term_config::TriggerConfig {
        name: "mux-wiring-trigger".to_string(),
        pattern: "TRIGGER-PATTERN-FIRED".to_string(),
        enabled: true,
        actions: vec![par_term_config::TriggerActionConfig::RunCommand {
            command: "echo".to_string(),
            args: vec!["mux-trigger-ran".to_string()],
        }],
        prompt_before_run: true,
        i_accept_the_risk: false,
        allowed_commands: Vec::new(),
    }];

    // Created-session attach (the paste-test pattern): the daemon
    // pushes the window on the first pump, the layout consumer
    // creates the native mirror pane carrying the trigger registry.
    let transport = connect(&path);
    let attach = attach_test(&transport, "wstrig", Some((80, 24)), &Default::default())
        .expect("attach_sequence");
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws = crate::app::window_state::WindowState::new(config, runtime);
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("wstrig".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer created the native mirror pane.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let native = loop {
        ws.check_mux_notifications();
        if let Some(&(tab_id, native)) = ws.tmux_state.tmux_pane_owners.get(&0) {
            ws.tab_manager.switch_to(tab_id);
            break native;
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the native pane: {:?}",
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    };

    // The mirror terminal must carry the trigger registry — mirrors are
    // created after Tab::new_internal's one-shot sync, so creation and
    // config propagation both have to install triggers into panes.
    {
        let tab = ws.tab_manager.active_tab().expect("mux tab active");
        let pane = tab
            .pane_manager()
            .and_then(|pm| pm.get_pane(native))
            .expect("mirror pane exists");
        let term = pane.terminal.try_read().expect("pane terminal free");
        assert!(
            term.trigger_names()
                .values()
                .any(|n| n == "mux-wiring-trigger"),
            "mirror pane must have the trigger installed: {:?}",
            term.trigger_names()
        );
    }

    // Seed pattern text through the production routing path — the
    // daemon pane's %output — and run the per-frame trigger dispatch.
    ws.handle_tmux_output(0, b"TRIGGER-PATTERN-FIRED\r\n");
    ws.check_trigger_actions();

    let queued: Vec<&par_term_emu_core_rust::terminal::ActionResult> = ws
        .trigger_state
        .pending_trigger_actions
        .iter()
        .map(|p| &p.action)
        .collect();
    assert!(
        queued
            .iter()
            .any(|a| matches!(a, par_term_emu_core_rust::terminal::ActionResult::RunCommand { command, args, .. }
                if command == "echo" && args.first().is_some_and(|a| a == "mux-trigger-ran"))),
        "the mux pane's trigger must fire through poll+dispatch: {queued:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// Session logging hooks the PTY output callback on `tab.terminal` —
/// in a mux tab that terminal is the hidden shell, and the mirror
/// panes that render daemon output have no PTY reader thread to fire
/// the callback. Output seeded through `handle_tmux_output` must
/// reach the tab's session log, including for a pane created by a
/// later layout change while logging is active (card
/// 01a0d9b559487e42a651a18e93dfce93, criterion 2).
#[test]
fn session_logging_captures_mux_pane_output() {
    let path = socket_path("ws-sessionlog");
    spawn_daemon(&path);

    // Plain-format session log in an isolated directory.
    let logs = tempfile::tempdir().expect("tempdir");
    let mut config = crate::config::Config::default();
    config.session_log.session_log_directory = logs.path().display().to_string();
    config.session_log.session_log_format = crate::config::SessionLogFormat::Plain;

    // Created-session attach (the triggers-test pattern): the daemon
    // pushes the window on the first pump, the layout consumer
    // creates the native mirror pane.
    let transport = connect(&path);
    let attach = attach_test(&transport, "wssesslog", Some((80, 24)), &Default::default())
        .expect("attach_sequence");
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws = crate::app::window_state::WindowState::new(config, runtime);
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("wssesslog".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer created the native mirror pane.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let tab_id = loop {
        ws.check_mux_notifications();
        if let Some(&(tab_id, _)) = ws.tmux_state.tmux_pane_owners.get(&0) {
            break tab_id;
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the pane: {:?}",
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    ws.tab_manager.switch_to(tab_id);

    // Toggle logging on through the production entry (the hotkey path).
    {
        let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
        let started = tab.toggle_session_logging(&ws.config.load());
        assert!(
            matches!(started, Ok(true)),
            "session logging must start: {started:?}"
        );
    }

    // Seed output through the production routing path — the daemon
    // pane's %output.
    ws.handle_tmux_output(0, b"SESSION-LOG-MUX-MARKER-1\r\n");

    // Split while logging is active — a pane born from a later layout
    // change must log too (the re-attachment path).
    quiesce(&mut ws);
    ws.split_pane_vertical();
    let mut split_landed = false;
    while Instant::now() < deadline && !split_landed {
        ws.check_mux_notifications();
        split_landed = ws.tmux_state.tmux_pane_owners.contains_key(&1);
        if !split_landed {
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    assert!(
        split_landed,
        "the daemon-side split must create the second pane: {:?}",
        ws.tmux_state.tmux_pane_owners
    );
    ws.handle_tmux_output(1, b"SESSION-LOG-MUX-MARKER-2\r\n");

    // Stop (which flushes) and read the log back.
    {
        let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
        let stopped = tab.toggle_session_logging(&ws.config.load());
        assert!(
            matches!(stopped, Ok(false)),
            "session logging must stop: {stopped:?}"
        );
    }
    let mut logged = String::new();
    for entry in std::fs::read_dir(logs.path()).expect("log dir readable") {
        let entry = entry.expect("dir entry");
        if entry.path().is_file() {
            logged.push_str(&std::fs::read_to_string(entry.path()).unwrap_or_default());
        }
    }
    assert!(
        logged.contains("SESSION-LOG-MUX-MARKER-1"),
        "mux pane output must reach the session log: {logged:?}"
    );
    assert!(
        logged.contains("SESSION-LOG-MUX-MARKER-2"),
        "output of a pane created while logging is active must reach the log: {logged:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// Plugin event subscriptions are wired by `pump_plugin_events`,
/// which registered each subscribed plugin's forwarder on
/// `tab.terminal` only — in a mux tab that is the hidden shell, so a
/// bell rung in a mux pane (daemon `%output` through
/// `handle_tmux_output`) never reached the plugin (card
/// 01a0d9b559487e42a651a18e93dfce93, criterion 3).
#[test]
fn plugin_receives_bell_from_mux_pane() {
    if par_term_scripting::manager::python_interpreter().is_none() {
        eprintln!("skipping: no Python interpreter on PATH");
        return;
    }
    let path = socket_path("ws-pluginbell");
    spawn_daemon(&path);

    // A subscribed plugin fixture: status-bar-widget kind subscribed
    // to bell_rang, whose entry stays alive without speaking the
    // protocol (the forwarder exists from spawn; the process never
    // needs to answer).
    let plugins = tempfile::tempdir().expect("tempdir");
    let dir = plugins.path().join("com.example.belltest");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("manifest.json"),
        concat!(
            r#"{"schemaVersion":1,"id":"com.example.belltest","name":"Bell test","version":"0.1.0","#,
            r#""kinds":["status-bar-widget"],"activation":"manual","#,
            r#""entryPoints":{"statusBarWidget":{"command":"widget.py","args":[]}},"#,
            r#""statusBarWidget":{"displayName":"Bell test","section":"right","defaults":{},"#,
            r#""schema":[{"key":"on","type":"boolean","label":"On","defaultValue":true}]},"#,
            r#""subscriptions":["bell_rang"]}"#
        ),
    )
    .unwrap();
    std::fs::write(dir.join("widget.py"), "import time\ntime.sleep(60)\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(
            dir.join("widget.py"),
            std::fs::Permissions::from_mode(0o755),
        )
        .unwrap();
    }

    // Created-session attach (the triggers-test pattern): the daemon
    // pushes the window on the first pump, the layout consumer
    // creates the native mirror pane.
    let transport = connect(&path);
    let attach = attach_test(
        &transport,
        "wsplugbell",
        Some((80, 24)),
        &Default::default(),
    )
    .expect("attach_sequence");
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws =
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("wsplugbell".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer created the native mirror pane.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    loop {
        ws.check_mux_notifications();
        if let Some(&(tab_id, _)) = ws.tmux_state.tmux_pane_owners.get(&0) {
            ws.tab_manager.switch_to(tab_id);
            break;
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the pane: {:?}",
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    }

    // Enable the plugin through the host API (the Settings toggle's
    // path) and run the per-frame registration sweep — the event
    // loop's entry for plugin event wiring.
    {
        let host = ws.status_bar_ui.plugin_host_mut();
        let (found, warnings) = par_term_scripting::manifest::discover_plugins(plugins.path());
        assert!(
            found.len() == 1,
            "fixture plugin must pass discovery: {warnings:?}"
        );
        host.refresh_discovery(plugins.path());
        host.apply_enabled(&[par_term_scripting::plugin_manager::EnabledPlugin {
            id: "com.example.belltest".to_string(),
            settings_json: "{}".to_string(),
        }]);
        host.poll();
    }
    let forwarder = ws
        .status_bar_ui
        .plugin_host()
        .subscription_forwarders()
        .get("com.example.belltest")
        .expect("subscribed plugin has a forwarder")
        .clone();
    ws.status_bar_ui.pump_plugin_events(&ws.tab_manager);

    // Ring the bell in the daemon pane — the production routing path.
    ws.handle_tmux_output(0, b"\x07");

    let events = forwarder.drain_events();
    assert!(
        events.iter().any(|e| e.kind == "bell_rang"),
        "the mux pane's bell must reach the subscribed plugin: {events:?}"
    );

    let _ = std::fs::remove_file(&path);
}

/// Tab-script event subscriptions attach their forwarder on
/// `tab.terminal` only (`start_script_at`) — in a mux tab that is the
/// hidden shell, so a bell rung in a mux pane never reached a
/// subscribed script (card 01a0d9b559487e42a651a18e93dfce93,
/// criterion 3).
#[test]
fn tab_script_receives_bell_from_mux_pane() {
    if par_term_scripting::manager::python_interpreter().is_none() {
        eprintln!("skipping: no Python interpreter on PATH");
        return;
    }
    let path = socket_path("ws-scriptbell");
    spawn_daemon(&path);

    // A stay-alive script entry; the forwarder is created at spawn and
    // the process itself never needs to speak the protocol.
    let scripts = tempfile::tempdir().expect("tempdir");
    let entry = scripts.path().join("bell_script.py");
    std::fs::write(&entry, "import time\ntime.sleep(60)\n").unwrap();

    // Created-session attach (the triggers-test pattern).
    let transport = connect(&path);
    let attach = attach_test(
        &transport,
        "wsscriptbell",
        Some((80, 24)),
        &Default::default(),
    )
    .expect("attach_sequence");
    let runtime = std::sync::Arc::new(
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build test runtime"),
    );
    let mut ws =
        crate::app::window_state::WindowState::new(crate::config::Config::default(), runtime);
    for window_id in &attach.existing_windows {
        ws.handle_tmux_window_add(*window_id);
    }
    ws.tmux_state.mux_screen_seeds = attach.screens.into_iter().collect();
    ws.tmux_state.transport = Some(Box::new(transport));
    ws.tmux_state.tmux_session_name = Some("wsscriptbell".to_string());
    ws.tmux_state.tmux_sync.enable();

    // Pump until the layout consumer created the native mirror pane.
    let deadline = Instant::now() + DAEMON_DEADLINE;
    let tab_id = loop {
        ws.check_mux_notifications();
        if let Some(&(tab_id, _)) = ws.tmux_state.tmux_pane_owners.get(&0) {
            ws.tab_manager.switch_to(tab_id);
            break tab_id;
        }
        assert!(
            Instant::now() < deadline,
            "the layout consumer never created the pane: {:?}",
            ws.tmux_state.tmux_pane_owners
        );
        std::thread::sleep(Duration::from_millis(25));
    };

    // Start a subscribed tab script on the mux tab (the Settings
    // start button shares `start_script_at`).
    let script_config = par_term_config::ScriptConfig {
        name: "mux-bell-script".to_string(),
        enabled: true,
        script_path: entry.display().to_string(),
        args: Vec::new(),
        auto_start: false,
        restart_policy: par_term_config::automation::RestartPolicy::Never,
        restart_delay_ms: 0,
        subscriptions: vec!["bell_rang".to_string()],
        env_vars: std::collections::HashMap::new(),
        allow_write_text: false,
        prompt_before_write_text: true,
        allow_run_command: false,
        allow_change_config: false,
        write_text_rate_limit: 0,
        run_command_rate_limit: 0,
    };
    {
        let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
        let terminal = tab.terminal.clone();
        let term = terminal.try_read().expect("tab terminal free");
        tab.scripting
            .start_script_at(&term, 0, &script_config)
            .expect("script starts");
    }

    // The per-frame pane-attachment reconcile — the same call the
    // event loop's script sweep runs for every tab with scripts.
    let pane_terminals: Vec<(
        crate::pane::PaneId,
        std::sync::Arc<tokio::sync::RwLock<par_term_terminal::TerminalManager>>,
    )> = {
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
        tab.pane_manager()
            .map(|pm| {
                pm.all_panes()
                    .into_iter()
                    .map(|p| (p.id, std::sync::Arc::clone(&p.terminal)))
                    .collect()
            })
            .unwrap_or_default()
    };
    {
        let tab = ws.tab_manager.get_tab_mut(tab_id).expect("mux tab");
        tab.scripting.reconcile_pane_observers(&pane_terminals);
    }

    // Ring the bell in the daemon pane — the production routing path.
    ws.handle_tmux_output(0, b"\x07");

    let forwarder = {
        let tab = ws.tab_manager.get_tab(tab_id).expect("mux tab");
        tab.scripting.script_forwarders[0]
            .clone()
            .expect("forwarder stored at start")
    };
    let events = forwarder.drain_events();
    assert!(
        events.iter().any(|e| e.kind == "bell_rang"),
        "the mux pane's bell must reach the subscribed tab script: {events:?}"
    );

    let _ = std::fs::remove_file(&path);
}
