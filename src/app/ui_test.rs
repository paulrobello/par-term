//! In-app UI test harness: scripted key injection + overlay assertions.
//!
//! Agents cannot drive the real UI from outside on macOS: TCC refuses
//! synthetic keystrokes and screen capture for ungranted host processes
//! (osascript error 1002; `screencapture` "could not create image from
//! display"), and `--screenshot` captures only the offscreen pane composite,
//! which excludes egui overlays by design. This harness closes the gap from
//! inside the process, where no host permission applies:
//!
//! - `chord` steps reach the real keybinding layer through
//!   [`KeybindingRegistry::lookup_with_key_fields`] — winit's `KeyEvent` has
//!   private fields and cannot be constructed outside winit, so chords enter
//!   as public key fields — and dispatch via `execute_keybinding_action`,
//!   the same entry point real key events use.
//! - `type_text`/`press` steps push [`egui::Event`]s onto
//!   `EguiState::pending_events`, the channel macOS menu accelerators already
//!   use for synthetic input, so overlay widgets consume them on the next
//!   frame.
//! - `assert*` steps read live overlay/window state. Every step appends an
//!   observation record regardless of assertion, and the run finishes by
//!   writing a JSON report and exiting the app.
//!
//! Scripts are JSON. See `docs/guides/AGENT_UI_VERIFICATION.md` for the
//! format and a worked palette example.

use crate::app::window_manager::WindowManager;
use crate::app::window_state::WindowState;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use winit::event_loop::ActiveEventLoop;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, NativeKeyCode, PhysicalKey};

/// Default delay before each step, letting the event loop settle.
fn default_wait_ms() -> u64 {
    250
}

/// A parsed `--ui-test` script.
#[derive(Debug, Clone)]
pub(crate) struct UiTestScript {
    /// Steps in execution order.
    pub(crate) steps: Vec<UiTestStep>,
}

/// One scripted action plus its settle delay.
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct UiTestStep {
    /// Milliseconds to wait before executing this step.
    #[serde(default = "default_wait_ms")]
    pub(crate) wait_ms: u64,
    /// What to do.
    #[serde(flatten)]
    pub(crate) action: UiTestAction,
}

/// The step verb. Exactly one field per JSON object (untagged).
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub(crate) enum UiTestAction {
    /// Inject a chord (e.g. `"Ctrl+Alt+Cmd+P"`) through the keybinding layer.
    Chord { chord: String },
    /// Deliver text to the focused egui widget (palette/settings fields).
    TypeText { type_text: String },
    /// Press a named key (`Enter`, `Escape`, `ArrowDown`, `F1`..`F12`, ...) on
    /// the egui side.
    Press { press: String },
    /// Assert a named boolean condition is true.
    Assert { assert: String },
    /// Assert a named boolean condition is false.
    AssertNot { assert_not: String },
    /// Assert a named keyed value: `["top_action", "toggle_fullscreen"]`,
    /// `["file_empty", "/tmp/capture.txt"]`.
    AssertEq { assert_eq: (String, String) },
}

/// Load and parse a script file. Fails loudly on bad JSON so typos never
/// masquerade as a passing run.
pub(crate) fn load_script(path: &Path) -> anyhow::Result<UiTestScript> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("--ui-test: cannot read {}: {}", path.display(), e))?;
    #[derive(Deserialize)]
    struct RawScript {
        steps: Vec<UiTestStep>,
    }
    let raw: RawScript = serde_json::from_str(&raw).map_err(|e| {
        anyhow::anyhow!(
            "--ui-test: invalid script JSON in {}: {}",
            path.display(),
            e
        )
    })?;
    Ok(UiTestScript { steps: raw.steps })
}

/// Accumulated per-step records for the final report.
#[derive(Debug, Default)]
pub(crate) struct UiTestRun {
    /// Where the JSON report is written at finish.
    pub(crate) report_path: Option<PathBuf>,
    /// One record per executed step, in order.
    pub(crate) records: Vec<StepRecord>,
    /// Count of failed assertions (missing assert operands count too).
    pub(crate) failed: usize,
    /// Count of passed assertions.
    pub(crate) passed: usize,
}

/// What one step did, with the observation taken after it.
#[derive(Debug, Serialize)]
pub(crate) struct StepRecord {
    /// 1-based step index.
    index: usize,
    /// The settle delay that preceded the step.
    wait_ms: u64,
    /// Human-readable description of the action.
    action: String,
    /// `Some(ok)` for verdict-bearing steps (assertions, and action steps
    /// that could not execute); `None` for performed non-verdict steps.
    ok: Option<bool>,
    /// Outcome detail (matched action, assert value, errors).
    detail: String,
    /// Live UI state sampled right after the step.
    observation: Observation,
}

/// Overlay/window visibility snapshot taken after every step — the script's
/// eyes. Fields are the surfaces `--ui-test` exists to expose.
#[derive(Debug, Serialize, Clone)]
pub(crate) struct Observation {
    /// Command palette overlay visible.
    palette_open: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    agent_usage_panel_open: bool,
    /// Search overlay visible.
    search_open: bool,
    /// Standalone settings window open.
    settings_window_open: bool,
    /// `any_modal_ui_visible()` — the guard that blocks keys from the PTY.
    modal_guard: bool,
    /// egui currently owns keyboard focus (text field focused).
    egui_keyboard: bool,
    /// Window is in fullscreen mode.
    fullscreen: bool,
    /// Top-ranked palette action id for the current query, if the palette
    /// is open.
    top_action: Option<String>,
    /// Names of every modal overlay currently visible — the modal_guard
    /// breakdown, so a surprise guard=true names its cause.
    modals: Vec<String>,
}

/// Names of the modal overlays `any_modal_ui_visible()` sums over, in its
/// declaration order — one string per visible overlay.
fn visible_modal_names(ws: &WindowState) -> Vec<String> {
    let o = &ws.overlay_ui;
    let mut names = Vec::new();
    let mut push = |visible: bool, name: &str| {
        if visible {
            names.push(name.to_string());
        }
    };
    push(o.help_ui.visible, "help_ui");
    push(o.clipboard_history_ui.visible, "clipboard_history_ui");
    push(o.command_history_ui.visible, "command_history_ui");
    push(o.search_ui.visible, "search_ui");
    push(o.command_palette.visible, "command_palette");
    push(o.tmux_session_picker_ui.visible, "tmux_session_picker_ui");
    push(o.shader_install_ui.visible, "shader_install_ui");
    push(o.integrations_ui.visible, "integrations_ui");
    push(o.ssh_connect_ui.is_visible(), "ssh_connect_ui");
    push(
        o.remote_shell_install_ui.is_visible(),
        "remote_shell_install_ui",
    );
    push(o.quit_confirmation_ui.is_visible(), "quit_confirmation_ui");
    names
}

/// Convert a chord string into the (logical key, physical key, modifiers)
/// triple the keybinding seam consumes.
fn chord_to_fields(chord: &str) -> Result<(Key, PhysicalKey, winit::event::Modifiers), String> {
    use par_term_keybindings::parser::{ParsedKey, parse_key_combo};

    let combo = parse_key_combo(chord).map_err(|e| format!("cannot parse chord '{chord}': {e}"))?;
    let m = &combo.modifiers;

    let mut state = ModifiersState::empty();
    if m.ctrl {
        state |= ModifiersState::CONTROL;
    }
    if m.alt {
        state |= ModifiersState::ALT;
    }
    if m.shift {
        state |= ModifiersState::SHIFT;
    }
    // CmdOrCtrl resolves platform-side at match time; injected chords must
    // carry the resolved modifier to match what a real event would report.
    if m.super_key || (m.cmd_or_ctrl && cfg!(target_os = "macos")) {
        state |= ModifiersState::SUPER;
    }
    if m.cmd_or_ctrl && !cfg!(target_os = "macos") {
        state |= ModifiersState::CONTROL;
    }
    let modifiers = winit::event::Modifiers::from(state);

    match combo.key {
        ParsedKey::Character(c) => {
            let upper = c.to_ascii_uppercase().to_string();
            Ok((
                Key::Character(upper.into()),
                PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
                modifiers,
            ))
        }
        ParsedKey::Named(named) => Ok((Key::Named(named), named_physical_code(&named), modifiers)),
        ParsedKey::Physical(code) => {
            // Physical bindings name a KeyCode, not a logical key; matching
            // goes by scan code, so the logical slot carries a placeholder.
            Ok((Key::Dead(None), PhysicalKey::Code(code), modifiers))
        }
    }
}

/// Best-effort physical code for named keys, so physical-preference matching
/// works for the common harness chords.
fn named_physical_code(key: &NamedKey) -> PhysicalKey {
    match key {
        NamedKey::F1 => PhysicalKey::Code(KeyCode::F1),
        NamedKey::F2 => PhysicalKey::Code(KeyCode::F2),
        NamedKey::F3 => PhysicalKey::Code(KeyCode::F3),
        NamedKey::F4 => PhysicalKey::Code(KeyCode::F4),
        NamedKey::F5 => PhysicalKey::Code(KeyCode::F5),
        NamedKey::F6 => PhysicalKey::Code(KeyCode::F6),
        NamedKey::F7 => PhysicalKey::Code(KeyCode::F7),
        NamedKey::F8 => PhysicalKey::Code(KeyCode::F8),
        NamedKey::F9 => PhysicalKey::Code(KeyCode::F9),
        NamedKey::F10 => PhysicalKey::Code(KeyCode::F10),
        NamedKey::F11 => PhysicalKey::Code(KeyCode::F11),
        NamedKey::F12 => PhysicalKey::Code(KeyCode::F12),
        NamedKey::Enter => PhysicalKey::Code(KeyCode::Enter),
        NamedKey::Escape => PhysicalKey::Code(KeyCode::Escape),
        NamedKey::Tab => PhysicalKey::Code(KeyCode::Tab),
        NamedKey::Backspace => PhysicalKey::Code(KeyCode::Backspace),
        NamedKey::Delete => PhysicalKey::Code(KeyCode::Delete),
        NamedKey::ArrowUp => PhysicalKey::Code(KeyCode::ArrowUp),
        NamedKey::ArrowDown => PhysicalKey::Code(KeyCode::ArrowDown),
        NamedKey::ArrowLeft => PhysicalKey::Code(KeyCode::ArrowLeft),
        NamedKey::ArrowRight => PhysicalKey::Code(KeyCode::ArrowRight),
        NamedKey::Home => PhysicalKey::Code(KeyCode::Home),
        NamedKey::End => PhysicalKey::Code(KeyCode::End),
        NamedKey::PageUp => PhysicalKey::Code(KeyCode::PageUp),
        NamedKey::PageDown => PhysicalKey::Code(KeyCode::PageDown),
        _ => PhysicalKey::Unidentified(NativeKeyCode::Unidentified),
    }
}

/// Map a step's `press` name to the egui key it should deliver.
fn press_to_egui_key(name: &str) -> Option<egui::Key> {
    Some(match name.to_ascii_lowercase().as_str() {
        "enter" => egui::Key::Enter,
        "escape" => egui::Key::Escape,
        "tab" => egui::Key::Tab,
        "backspace" => egui::Key::Backspace,
        "delete" => egui::Key::Delete,
        "arrowup" | "up" => egui::Key::ArrowUp,
        "arrowdown" | "down" => egui::Key::ArrowDown,
        "arrowleft" | "left" => egui::Key::ArrowLeft,
        "arrowright" | "right" => egui::Key::ArrowRight,
        "home" => egui::Key::Home,
        "end" => egui::Key::End,
        "pageup" => egui::Key::PageUp,
        "pagedown" => egui::Key::PageDown,
        "f1" => egui::Key::F1,
        "f2" => egui::Key::F2,
        "f3" => egui::Key::F3,
        "f4" => egui::Key::F4,
        "f5" => egui::Key::F5,
        "f6" => egui::Key::F6,
        "f7" => egui::Key::F7,
        "f8" => egui::Key::F8,
        "f9" => egui::Key::F9,
        "f10" => egui::Key::F10,
        "f11" => egui::Key::F11,
        "f12" => egui::Key::F12,
        // Single letters a–z are documented press names
        // (AGENT_UI_VERIFICATION.md) for letter-keyed overlays such as the
        // agent-usage panel's `r` refresh; anything else is unknown.
        s if s.len() == 1 && s.as_bytes()[0].is_ascii_alphabetic() => egui::Key::from_name(s)?,
        _ => return None,
    })
}

/// Fold a step outcome into (action description, verdict, detail).
///
/// `Failed` must carry `Some(false)` — a step that could not execute
/// (unknown press/chord name, no terminal window yet) has to fail the run,
/// or a typo'd script reports `all_passed: true`. `Performed` carries no
/// verdict, including a chord deliberately blocked by the modal guard.
fn step_verdict(outcome: StepOutcome) -> (String, Option<bool>, String) {
    match outcome {
        StepOutcome::Performed(desc) => (desc, None, String::new()),
        StepOutcome::Asserted {
            desc,
            passed,
            detail,
        } => (desc, Some(passed), detail),
        StepOutcome::Failed(desc) => (desc.clone(), Some(false), desc),
    }
}

impl WindowManager {
    /// Execute one scripted step against the first terminal window.
    pub(crate) fn run_ui_test_step(&mut self, step: &UiTestStep) {
        let index = self.ui_test.records.len() + 1;
        let (action_desc, ok, detail) = step_verdict(self.ui_test_step_inner(step));
        let observation = self.ui_test_observe();
        if ok == Some(false) {
            self.ui_test.failed += 1;
            log::warn!("UI TEST step {index} FAILED: {action_desc} — {detail}");
        } else if ok == Some(true) {
            self.ui_test.passed += 1;
        }
        self.ui_test.records.push(StepRecord {
            index,
            wait_ms: step.wait_ms,
            action: action_desc,
            ok,
            detail,
            observation,
        });
    }

    /// The step body, before observation recording.
    fn ui_test_step_inner(&mut self, step: &UiTestStep) -> StepOutcome {
        // Steps act on the first non-settings terminal window; scripts are
        // single-window by design.
        let window_ids: Vec<_> = self.windows.keys().copied().collect();
        let terminal_id = window_ids
            .into_iter()
            .find(|id| !self.is_settings_window(*id));

        match &step.action {
            UiTestAction::Chord { chord } => {
                let Some(id) = terminal_id else {
                    return StepOutcome::Failed("chord: no terminal window yet".into());
                };
                let ws = self.windows.get_mut(&id).expect("id from keys()");
                Self::inject_chord(ws, chord)
            }
            UiTestAction::TypeText { type_text } => {
                let Some(id) = terminal_id else {
                    return StepOutcome::Failed("type_text: no terminal window yet".into());
                };
                let ws = self.windows.get_mut(&id).expect("id from keys()");
                ws.egui
                    .pending_events
                    .push(egui::Event::Text(type_text.clone()));
                ws.request_redraw();
                StepOutcome::Performed(format!("type_text \"{type_text}\" (egui)"))
            }
            UiTestAction::Press { press } => {
                let Some(egui_key) = press_to_egui_key(press) else {
                    return StepOutcome::Failed(format!(
                        "press: unknown key name '{press}' (see AGENT_UI_VERIFICATION.md)"
                    ));
                };
                let Some(id) = terminal_id else {
                    return StepOutcome::Failed("press: no terminal window yet".into());
                };
                let ws = self.windows.get_mut(&id).expect("id from keys()");
                ws.egui.pending_events.push(egui::Event::Key {
                    key: egui_key,
                    physical_key: None,
                    pressed: true,
                    repeat: false,
                    modifiers: egui::Modifiers::default(),
                });
                ws.request_redraw();
                StepOutcome::Performed(format!("press {press} (egui)"))
            }
            UiTestAction::Assert { assert } => {
                let value = self.ui_test_bool(assert);
                match value {
                    Some(v) => StepOutcome::Asserted {
                        desc: format!("assert {assert}"),
                        passed: v,
                        detail: format!("{assert} = {v}"),
                    },
                    None => StepOutcome::Asserted {
                        desc: format!("assert {assert}"),
                        passed: false,
                        detail: format!("unknown assert operand '{assert}'"),
                    },
                }
            }
            UiTestAction::AssertNot { assert_not } => {
                let value = self.ui_test_bool(assert_not);
                match value {
                    Some(v) => StepOutcome::Asserted {
                        desc: format!("assert_not {assert_not}"),
                        passed: !v,
                        detail: format!("{assert_not} = {v}"),
                    },
                    None => StepOutcome::Asserted {
                        desc: format!("assert_not {assert_not}"),
                        passed: false,
                        detail: format!("unknown assert operand '{assert_not}'"),
                    },
                }
            }
            UiTestAction::AssertEq { assert_eq } => {
                let (what, expected) = assert_eq;
                match self.ui_test_value(what, expected) {
                    Ok((actual, passed)) => StepOutcome::Asserted {
                        desc: format!("assert_eq {what}"),
                        passed,
                        detail: format!("{what} = {actual:?} (expected {expected:?})"),
                    },
                    Err(err) => StepOutcome::Asserted {
                        desc: format!("assert_eq {what}"),
                        passed: false,
                        detail: err,
                    },
                }
            }
        }
    }

    /// Inject a chord through the real keybinding registry + action dispatch.
    fn inject_chord(ws: &mut WindowState, chord: &str) -> StepOutcome {
        let fields = match chord_to_fields(chord) {
            Ok(f) => f,
            Err(err) => return StepOutcome::Failed(format!("chord: {err}")),
        };
        let (logical, physical, modifiers) = fields;

        // Mirror the real event path: while a modal overlay is visible, the
        // modal guard in handle_window_event blocks every key except
        // F1/F2/F3/Escape before the keybinding layer ever runs.
        if ws.any_modal_ui_visible()
            && !matches!(
                logical,
                Key::Named(NamedKey::F1 | NamedKey::F2 | NamedKey::F3 | NamedKey::Escape)
            )
        {
            return StepOutcome::Performed(format!(
                "chord {chord} -> blocked by modal guard (overlay open)"
            ));
        }

        // Mirror the real event path: modifier state is updated before the
        // key press is looked up.
        ws.input_handler.update_modifiers(modifiers);

        let config = ws.config.load();
        let action = ws.keybinding_registry.lookup_with_key_fields(
            &logical,
            physical,
            &modifiers,
            &config.input.modifier_remapping,
            config.input.use_physical_keys,
        );
        drop(config);

        match action {
            Some(action) => {
                let action = action.to_string();
                let handled = ws.execute_keybinding_action(&action);
                ws.request_redraw();
                StepOutcome::Performed(format!(
                    "chord {chord} -> keybinding '{action}' (handled={handled})"
                ))
            }
            None => StepOutcome::Performed(format!(
                "chord {chord} -> no keybinding matched (not dispatched)"
            )),
        }
    }

    /// Evaluate a named boolean assert operand against live state.
    fn ui_test_bool(&self, name: &str) -> Option<bool> {
        Some(match name {
            "palette_open" => self.ui_test_palette_open(),
            "agent_usage_panel_open" => self.ui_test_agent_usage_panel_open(),
            "settings_window_open" => self.settings_window.is_some(),
            // Manager-level fallbacks when no terminal window exists yet.
            _ => {
                let ws = self.ui_test_window_state()?;
                match name {
                    "search_open" => ws.overlay_ui.search_ui.visible,
                    "agent_usage_ready" => !ws.status_bar_ui.usage_snapshot().records.is_empty(),
                    "plugins_loaded" => ws.status_bar_ui.plugins_discovered_count() >= 1,
                    "plugin_widget_set" => ws.status_bar_ui.any_plugin_widget_text(),
                    "plugin_panel_set" => ws.status_bar_ui.any_plugin_panel_pushed(),
                    "plugin_overlay_set" => !ws.status_bar_ui.plugin_host().overlays().is_empty(),
                    "plugin_action_dispatched" => ws.status_bar_ui.any_plugin_action_dispatched(),
                    "modal_guard" => ws.any_modal_ui_visible(),
                    "pane_hint_mode_active" => ws.pane_hint_select.is_active(),
                    "egui_keyboard" => ws.is_egui_using_keyboard(),
                    "fullscreen" => ws.window.as_ref().is_some_and(|w| w.fullscreen().is_some()),
                    _ => return None,
                }
            }
        })
    }

    /// Evaluate a keyed assert operand, returning (actual, passed).
    fn ui_test_value(&self, what: &str, expected: &str) -> Result<(String, bool), String> {
        match what {
            "top_action" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("top_action: no terminal window".into());
                };
                let actual = ws
                    .overlay_ui
                    .command_palette
                    .top_action()
                    .unwrap_or("<none>")
                    .to_string();
                Ok((actual.clone(), actual == expected))
            }
            "file_empty" => {
                let meta = std::fs::metadata(expected);
                let actual = match &meta {
                    Ok(m) => format!("{} bytes", m.len()),
                    Err(_) => "missing".to_string(),
                };
                let empty = matches!(&meta, Ok(m) if m.len() == 0)
                    || matches!(&meta, Err(e) if e.kind() == std::io::ErrorKind::NotFound);
                Ok((actual, empty))
            }
            _ => Err(format!("unknown assert_eq operand '{what}'")),
        }
    }

    /// Palette visibility is window-level but harmless to probe with no
    /// window (reports false).
    fn ui_test_palette_open(&self) -> bool {
        self.ui_test_window_state()
            .is_some_and(|ws| ws.overlay_ui.command_palette.visible)
    }

    /// Agent-usage panel visibility, same window-level probing contract.
    fn ui_test_agent_usage_panel_open(&self) -> bool {
        self.ui_test_window_state()
            .is_some_and(|ws| ws.overlay_ui.agent_usage_panel.visible)
    }

    /// The first non-settings terminal window's state, if one exists.
    fn ui_test_window_state(&self) -> Option<&WindowState> {
        self.windows
            .iter()
            .find(|(id, _)| !self.is_settings_window(**id))
            .map(|(_, ws)| ws)
    }

    /// Snapshot live overlay/window state — recorded after every step.
    fn ui_test_observe(&self) -> Observation {
        let ws = self.ui_test_window_state();
        Observation {
            palette_open: ws.is_some_and(|w| w.overlay_ui.command_palette.visible),
            agent_usage_panel_open: ws.is_some_and(|w| w.overlay_ui.agent_usage_panel.visible),
            search_open: ws.is_some_and(|w| w.overlay_ui.search_ui.visible),
            settings_window_open: self.settings_window.is_some(),
            modal_guard: ws.is_some_and(|w| w.any_modal_ui_visible()),
            egui_keyboard: ws.is_some_and(|w| w.is_egui_using_keyboard()),
            fullscreen: ws.is_some_and(|w| {
                w.window
                    .as_ref()
                    .is_some_and(|win| win.fullscreen().is_some())
            }),
            top_action: ws
                .and_then(|w| w.overlay_ui.command_palette.top_action())
                .map(str::to_string),
            modals: ws.map(visible_modal_names).unwrap_or_default(),
        }
    }

    /// Write the JSON report, log the summary, and exit the app. Called on
    /// `AppEvent::UiTestFinish`.
    pub(crate) fn ui_test_finish(&mut self, event_loop: &ActiveEventLoop) {
        let all_passed = self.ui_test.failed == 0;
        #[derive(Serialize)]
        struct Report<'a> {
            passed: usize,
            failed: usize,
            all_passed: bool,
            steps: &'a [StepRecord],
        }
        let report = Report {
            passed: self.ui_test.passed,
            failed: self.ui_test.failed,
            all_passed,
            steps: &self.ui_test.records,
        };
        match &self.ui_test.report_path {
            Some(path) => match serde_json::to_string_pretty(&report) {
                Ok(json) => {
                    if let Err(e) = std::fs::write(path, json) {
                        log::error!("UI TEST: cannot write report {}: {e}", path.display());
                    }
                }
                Err(e) => log::error!("UI TEST: cannot serialize report: {e}"),
            },
            None => log::warn!("UI TEST: no report path configured"),
        }
        log::info!(
            "UI TEST finished: {} passed, {} failed — {}",
            self.ui_test.passed,
            self.ui_test.failed,
            if all_passed {
                "ALL PASSED"
            } else {
                "FAILURES PRESENT"
            }
        );
        event_loop.exit();
    }
}

/// Result shape for step bodies.
enum StepOutcome {
    /// Non-asserting step; carried description.
    Performed(String),
    /// Assertion with its verdict.
    Asserted {
        desc: String,
        passed: bool,
        detail: String,
    },
    /// Step could not run (bad operand, no window); recorded as a failure.
    Failed(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chord_to_fields_maps_modifiers() {
        let (logical, physical, modifiers) = chord_to_fields("Ctrl+Alt+Cmd+P").expect("parses");
        assert!(matches!(logical, Key::Character(ref c) if c.as_str() == "P"));
        // Character chords match logically; the physical slot is a documented
        // placeholder (physical-preference configs would need real codes).
        assert!(matches!(physical, PhysicalKey::Unidentified(_)));
        let state = modifiers.state();
        assert!(state.control_key() && state.alt_key() && state.super_key());
        assert!(!state.shift_key());
    }

    #[test]
    fn chord_to_fields_rejects_modifier_only_chord() {
        // "Ctrl" ends with a modifier and has no key: a parse error.
        assert!(chord_to_fields("Ctrl").is_err());
    }

    #[test]
    fn named_keys_map_to_egui() {
        assert_eq!(press_to_egui_key("Enter"), Some(egui::Key::Enter));
        assert_eq!(press_to_egui_key("escape"), Some(egui::Key::Escape));
        assert_eq!(press_to_egui_key("Nope"), None);
    }

    #[test]
    fn single_letters_map_to_egui() {
        // The agent-usage panel's `r` refresh drives through this mapping.
        assert_eq!(press_to_egui_key("r"), Some(egui::Key::R));
        assert_eq!(press_to_egui_key("H"), Some(egui::Key::H));
        assert_eq!(press_to_egui_key("rr"), None);
        assert_eq!(press_to_egui_key("1"), None);
    }

    #[test]
    fn failed_steps_carry_a_verdict() {
        // An undrivable step (unknown press name, no terminal window yet)
        // must fail the run: ok=Some(false) feeds the failed counter and
        // all_passed, so a typo'd script cannot report a pass.
        let (_, ok, _) = step_verdict(StepOutcome::Failed("press: unknown key name 'x'".into()));
        assert_eq!(ok, Some(false));
    }

    #[test]
    fn performed_steps_stay_verdict_neutral() {
        // Includes chords deliberately blocked by the modal guard: the
        // injection happened as scripted, the block is the documented
        // behavior, so it must not fail the run.
        let (_, ok, _) = step_verdict(StepOutcome::Performed(
            "chord Ctrl+P -> blocked by modal guard (overlay open)".into(),
        ));
        assert_eq!(ok, None);
    }
}
