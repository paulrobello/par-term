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
//!   the same entry point real key events use. A chord bound to
//!   `pass_to_terminal`, or one no binding matches, is encoded to the
//!   focused terminal's PTY through the same `KeyInput` path the keyboard
//!   handler's tail uses.
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
    /// `["file_empty", "/tmp/capture.txt"]`,
    /// `["mux_pane_grid", "97x29"]`.
    AssertEq { assert_eq: (String, String) },
    /// Stash a keyed operand's current value for a later
    /// `assert_eq_captured` step — for values a script cannot know up
    /// front, like a spawned shell's PID.
    Capture { capture: String },
    /// Assert a keyed operand's current value equals the one an earlier
    /// `capture` step stashed for it.
    AssertEqCaptured { assert_eq_captured: String },
    /// Seed one of the B61 modal dialogs open (`close_running_job`,
    /// `mux_last_tab`, `trigger_confirm`, `agent_command_confirm`,
    /// `update_dialog`, `tab_context_menu`, `new_tab_profile_menu`,
    /// `demote_chooser`, `profile_drawer`, `quit_confirmation`) — the seam
    /// standing in for the user interaction that opens each dialog, so a
    /// script can prove typed keys stay off the PTY while it is open.
    OpenModal { open_modal: String },
    /// Seed clipboard-history entries into the focused pane's terminal
    /// (B70): in production only selection copies feed that history, which a
    /// script cannot drive — this stands in for the copy, newest last.
    SeedClipboard { seed_clipboard: Vec<String> },
    /// Close a dialog opened by `open_modal` (clears the seeded state; the
    /// dialogs' own button/Escape handling is egui-side, via `press`).
    CloseModal { close_modal: String },
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
    /// Operand values stashed by `capture` steps, keyed by operand name.
    pub(crate) captured: std::collections::HashMap<String, String>,
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
    push(o.agent_usage_panel.visible, "agent_usage_panel");
    push(o.tmux_session_picker_ui.visible, "tmux_session_picker_ui");
    push(o.tree_picker_ui.visible, "tree_picker_ui");
    push(o.pane_context_menu.is_open(), "pane_context_menu");
    push(o.shader_install_ui.visible, "shader_install_ui");
    push(o.integrations_ui.visible, "integrations_ui");
    push(o.ssh_connect_ui.is_visible(), "ssh_connect_ui");
    push(
        o.remote_shell_install_ui.is_visible(),
        "remote_shell_install_ui",
    );
    push(o.quit_confirmation_ui.is_visible(), "quit_confirmation_ui");
    // B61 additions — same order as the guard's B61 block.
    push(
        o.close_confirmation_ui.is_visible(),
        "close_confirmation_ui",
    );
    push(o.mux_last_tab_ui.is_visible(), "mux_last_tab_ui");
    push(
        !ws.trigger_state.pending_trigger_actions.is_empty(),
        "trigger_confirm",
    );
    push(
        !ws.agent_commands.pending_confirmations.is_empty(),
        "agent_command_confirm",
    );
    push(
        ws.update_state.show_dialog && ws.update_state.last_result.is_some(),
        "update_dialog",
    );
    push(ws.tab_bar_ui.is_context_menu_open(), "tab_context_menu");
    push(
        ws.tab_bar_ui.show_new_tab_profile_menu,
        "new_tab_profile_menu",
    );
    push(ws.pane_transfer_state.is_active(), "demote_chooser");
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
                Self::render_ui_test_frame(ws);
                StepOutcome::Performed(format!("type_text \"{type_text}\" (egui)"))
            }
            UiTestAction::Press { press } => {
                // A "Shift+"-prefixed name (e.g. "Shift+Enter") carries the
                // modifier on the egui event — panels distinguish plain from
                // shifted keys (clipboard history's Enter vs Shift+Enter).
                let (key_name, shift) = match press.strip_prefix("Shift+") {
                    Some(rest) => (rest, true),
                    None => (press.as_str(), false),
                };
                let Some(egui_key) = press_to_egui_key(key_name) else {
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
                    modifiers: egui::Modifiers {
                        shift,
                        ..Default::default()
                    },
                });
                Self::render_ui_test_frame(ws);
                StepOutcome::Performed(format!("press {press} (egui)"))
            }
            UiTestAction::SeedClipboard { seed_clipboard } => {
                let Some(id) = terminal_id else {
                    return StepOutcome::Failed("seed_clipboard: no terminal window yet".into());
                };
                let ws = self.windows.get_mut(&id).expect("id from keys()");
                let Some(tab) = ws.tab_manager.active_tab() else {
                    return StepOutcome::Failed("seed_clipboard: no active tab".into());
                };
                let terminal = tab.read_terminal_handle();
                let Ok(term) = terminal.try_read() else {
                    return StepOutcome::Failed("seed_clipboard: terminal lock busy".into());
                };
                for content in seed_clipboard {
                    term.add_to_clipboard_history(
                        par_term_terminal::ClipboardSlot::Clipboard,
                        content.clone(),
                        None,
                    );
                }
                StepOutcome::Performed(format!("seed_clipboard {} entries", seed_clipboard.len()))
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
            UiTestAction::OpenModal { open_modal } => {
                let Some(id) = terminal_id else {
                    return StepOutcome::Failed("open_modal: no terminal window yet".into());
                };
                let ws = self.windows.get_mut(&id).expect("id from keys()");
                Self::seed_modal_state(ws, open_modal, true)
            }
            UiTestAction::CloseModal { close_modal } => {
                let Some(id) = terminal_id else {
                    return StepOutcome::Failed("close_modal: no terminal window yet".into());
                };
                let ws = self.windows.get_mut(&id).expect("id from keys()");
                Self::seed_modal_state(ws, close_modal, false)
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
            UiTestAction::Capture { capture } => match self.ui_test_operand(capture) {
                Ok(actual) => {
                    self.ui_test
                        .captured
                        .insert(capture.clone(), actual.clone());
                    StepOutcome::Performed(format!("capture {capture} = {actual}"))
                }
                Err(err) => StepOutcome::Failed(format!("capture {capture}: {err}")),
            },
            UiTestAction::AssertEqCaptured { assert_eq_captured } => {
                let what = assert_eq_captured.as_str();
                let Some(expected) = self.ui_test.captured.get(what).cloned() else {
                    return StepOutcome::Asserted {
                        desc: format!("assert_eq_captured {what}"),
                        passed: false,
                        detail: format!("no captured value for operand '{what}'"),
                    };
                };
                match self.ui_test_operand(what) {
                    Ok(actual) => StepOutcome::Asserted {
                        desc: format!("assert_eq_captured {what}"),
                        passed: actual == expected,
                        detail: format!("{what} = {actual:?} (captured {expected:?})"),
                    },
                    Err(err) => StepOutcome::Asserted {
                        desc: format!("assert_eq_captured {what}"),
                        passed: false,
                        detail: err,
                    },
                }
            }
        }
    }

    /// Seed (`open == true`) or clear (`open == false`) one B61 dialog's
    /// state. Opening uses each dialog's real entry point so the seeded
    /// state is exactly what the user interaction produces; closing restores
    /// the hidden state without driving egui — buttons and Escape are the
    /// egui-side `press` steps' job, this only arms/disarms the modal the
    /// key guard sums over.
    fn seed_modal_state(ws: &mut WindowState, name: &str, open: bool) -> StepOutcome {
        use crate::app::window_state::PendingTriggerAction;
        use par_term_config::agent_commands::CommandAuthor;

        let verb = if open { "open_modal" } else { "close_modal" };
        match name {
            "close_running_job" => {
                if open {
                    let Some(tab_id) = ws.tab_manager.active_tab_id() else {
                        return StepOutcome::Failed(format!("{verb} {name}: no active tab"));
                    };
                    ws.overlay_ui.close_confirmation_ui.show_for_tab(
                        tab_id,
                        "ui-test tab",
                        "sleep 100",
                    );
                } else {
                    ws.overlay_ui.close_confirmation_ui.hide();
                }
            }
            "mux_last_tab" => {
                if open {
                    ws.overlay_ui.mux_last_tab_ui.show_for_session("ui-test");
                } else {
                    ws.overlay_ui.mux_last_tab_ui.hide();
                }
            }
            "trigger_confirm" => {
                if open {
                    ws.trigger_state
                        .pending_trigger_actions
                        .push(PendingTriggerAction {
                            trigger_id: 0,
                            trigger_name: "ui-test trigger".to_string(),
                            action: par_term_emu_core_rust::terminal::ActionResult::RunCommand {
                                trigger_id: 0,
                                command: "echo".to_string(),
                                args: vec!["b61".to_string()],
                            },
                            description: "ui-test pending trigger action".to_string(),
                            target: None,
                        });
                } else {
                    ws.trigger_state.pending_trigger_actions.clear();
                }
            }
            "agent_command_confirm" => {
                if open {
                    let file = par_term_config::agent_commands::AgentCommandFile {
                        created_by: CommandAuthor::User,
                        source_agent: None,
                        created_at: None,
                        action: par_term_config::CustomActionConfig::ShellCommand {
                            id: "ui-test-b61".to_string(),
                            title: "ui-test b61".to_string(),
                            command: "echo".to_string(),
                            args: vec!["b61".to_string()],
                            notify_on_success: false,
                            timeout_secs: 30,
                            capture_output: false,
                            keybinding: None,
                            prefix_char: None,
                            keybinding_enabled: true,
                            description: None,
                        },
                    };
                    ws.agent_commands.request_confirmation(file);
                } else {
                    ws.agent_commands.pending_confirmations.clear();
                }
            }
            "update_dialog" => {
                if open {
                    ws.update_state.show_dialog = true;
                    ws.update_state.last_result = Some(
                        par_term_update::update_checker::UpdateCheckResult::UpdateAvailable(
                            par_term_update::update_checker::UpdateInfo {
                                version: "999.0.0".to_string(),
                                release_notes: None,
                                release_url: "https://example.invalid".to_string(),
                                published_at: None,
                            },
                        ),
                    );
                } else {
                    ws.update_state.show_dialog = false;
                }
            }
            "tab_context_menu" => {
                if open {
                    let Some(tab_id) = ws.tab_manager.active_tab_id() else {
                        return StepOutcome::Failed(format!("{verb} {name}: no active tab"));
                    };
                    ws.tab_bar_ui.test_open_context_menu(tab_id);
                } else {
                    ws.tab_bar_ui.test_close_context_menu();
                }
            }
            "new_tab_profile_menu" => {
                ws.tab_bar_ui.show_new_tab_profile_menu = open;
            }
            "demote_chooser" => {
                if open {
                    let Some(tab_id) = ws.tab_manager.active_tab_id() else {
                        return StepOutcome::Failed(format!("{verb} {name}: no active tab"));
                    };
                    // Prefer the direction chooser (the B61 dialog); a
                    // single-pane tab without a focused pane id falls back
                    // to the first pick phase, which the same guard term
                    // covers.
                    let pane_id = ws
                        .tab_manager
                        .active_tab()
                        .and_then(|t| t.focused_pane_id());
                    ws.pane_transfer_state = match pane_id {
                        Some(target_pane_id) => {
                            crate::app::tab_ops::pane_transfer::PaneTransferState::DemoteChooseDirection {
                                source_tab_id: tab_id,
                                target_tab_id: tab_id,
                                target_pane_id,
                            }
                        }
                        None => crate::app::tab_ops::pane_transfer::PaneTransferState::DemotePickTab {
                            source_tab_id: tab_id,
                        },
                    };
                } else {
                    ws.cancel_pane_transfer();
                }
            }
            "profile_drawer" => {
                ws.overlay_ui.profile_drawer_ui.expanded = open;
            }
            "quit_confirmation" => {
                if open {
                    ws.overlay_ui
                        .quit_confirmation_ui
                        .show_confirmation(1, None);
                } else {
                    ws.overlay_ui.quit_confirmation_ui.hide();
                }
            }
            "tmux_picker" => {
                if open {
                    ws.overlay_ui.tmux_session_picker_ui.show_picker();
                } else {
                    ws.overlay_ui.tmux_session_picker_ui.hide();
                }
            }
            // B70: opened through the real entry points (the registry
            // `toggle_*` actions call these), so a script can drive the
            // panels' egui-side navigation and prove Enter delivers to the
            // PTY sink.
            "command_history" => {
                if ws.overlay_ui.command_history_ui.visible != open {
                    ws.toggle_command_history();
                }
            }
            "clipboard_history" => {
                if ws.overlay_ui.clipboard_history_ui.visible != open {
                    ws.toggle_clipboard_history();
                }
            }
            _ => {
                return StepOutcome::Failed(format!(
                    "{verb}: unknown dialog '{name}' (see UiTestAction::OpenModal docs)"
                ));
            }
        }
        ws.request_redraw();
        StepOutcome::Performed(format!("{verb} {name}"))
    }

    /// Drive one full render frame synchronously after queueing synthetic
    /// egui input.
    ///
    /// The alternative — `request_redraw` and letting the OS deliver
    /// `RedrawRequested` — is neither immediate nor guaranteed: the FPS gate
    /// in `should_render_frame` rejects redraws that arrive close behind a
    /// rendered frame, and an occluded window may not get one at all. Both
    /// were observed as run-to-run flaky `press` delivery (b62/b64 scripts
    /// passing one run and failing the next with the key never reaching the
    /// dialog). Clearing `last_render_time` passes the gate for this one
    /// frame; the frame then flushes `pending_events` through the real
    /// `render()` → egui path, so the next step's asserts read state the
    /// input has already been applied to.
    fn render_ui_test_frame(ws: &mut WindowState) {
        ws.focus_state.last_render_time = None;
        ws.render();
        ws.request_redraw();
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

        // Mirror the real event path: an armed pane-hint mode captures every
        // key press (any press resolves it — mode-stack contract); a focused
        // plugin overlay swallows everything except Escape. Both checks sit
        // ahead of the keybinding layer in handle_key_event, so the injector
        // must sit them here too or a chord bypasses them.
        if ws.pane_hint_select.is_active() {
            let typed = match &logical {
                Key::Character(ch) => ch.chars().next(),
                _ => None,
            };
            ws.resolve_pane_hint_select(typed);
            return StepOutcome::Performed(format!("chord {chord} -> resolved pane-hint mode"));
        }
        if ws.pane_resize_mode.is_active() {
            let key = crate::app::pane_resize_mode::ResizeModeKey::from_key(
                &logical,
                modifiers.state().shift_key(),
            );
            let cell = ws.resize_mode_cell_size();
            ws.resolve_pane_resize_key(key, cell);
            return StepOutcome::Performed(format!("chord {chord} -> resize mode"));
        }
        if ws.status_bar_ui.plugin_host().focused_overlay().is_some() {
            let is_escape = matches!(&logical, Key::Named(NamedKey::Escape));
            ws.resolve_focused_overlay_key(is_escape);
            return StepOutcome::Performed(format!(
                "chord {chord} -> swallowed by focused overlay{}",
                if is_escape { " (unfocused)" } else { "" }
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
                if action == par_term_keybindings::PASS_TO_TERMINAL {
                    // A passthrough row claims the chord for the shell: the
                    // real handler skips every layer and encodes the key to
                    // the PTY, so the injector must too.
                    Self::inject_chord_to_pty(ws, chord, "pass_to_terminal", logical, physical)
                } else {
                    let handled = ws.execute_keybinding_action(&action);
                    ws.request_redraw();
                    StepOutcome::Performed(format!(
                        "chord {chord} -> keybinding '{action}' (handled={handled})"
                    ))
                }
            }
            // An unmatched key falls through every layer to the PTY in the
            // real handler; encode it the same way instead of dropping it.
            None => Self::inject_chord_to_pty(ws, chord, "unbound fallthrough", logical, physical),
        }
    }

    /// Encode a chord's key fields to the focused terminal's PTY — the tail
    /// of the real key handler that a `pass_to_terminal` row or an unmatched
    /// key reaches. Encoding goes through [`par_term_input::KeyInput`], the
    /// sanctioned construction path (a winit `KeyEvent` cannot be fabricated
    /// safely), and the write rides the shared read lock like the keyboard
    /// path so it cannot starve the refresh task.
    fn inject_chord_to_pty(
        ws: &mut WindowState,
        chord: &str,
        via: &str,
        logical: Key,
        physical: PhysicalKey,
    ) -> StepOutcome {
        // B61 mirror: the real key handler consumes any key that survives to
        // its encoding tail while a modal overlay is open (an Escape or
        // F1–F3 that no layer or keybinding claimed). The injector must
        // refuse the PTY write under the same condition, or an Escape chord
        // while a dialog is open would write `1b` the real handler now
        // never writes.
        if ws.any_modal_ui_visible() {
            return StepOutcome::Performed(format!(
                "chord {chord} -> {via} -> consumed by modal guard tail (no PTY write)"
            ));
        }
        // Same mode priority as the keyboard path: focused pane's terminal,
        // else the tab's cached modes. Scoped — the encoder needs &mut
        // input_handler, which cannot alias the tab borrow.
        let (modify_other_keys_mode, application_cursor) = {
            let Some(tab) = ws.tab_manager.active_tab() else {
                return StepOutcome::Failed(format!("chord {chord} -> PTY: no active tab"));
            };
            if let Some(ref pane_manager) = tab.pane_manager
                && let Some(focused_pane) = pane_manager.focused_pane()
                && let Ok(term) = focused_pane.terminal.try_read()
            {
                (term.modify_other_keys_mode(), term.application_cursor())
            } else {
                let (m, a, _) = tab.read_or_cached_modes();
                (m, a)
            }
        };

        let input = par_term_input::KeyInput {
            logical_key: logical,
            physical_key: physical,
            state: winit::event::ElementState::Pressed,
        };
        let Some(bytes) = ws.input_handler.handle_key_input_with_mode(
            &input,
            modify_other_keys_mode,
            application_cursor,
        ) else {
            return StepOutcome::Failed(format!("chord {chord} -> PTY: encoder declined the key"));
        };

        // Same terminal selection as the keyboard path: focused pane's
        // terminal when splits exist, else the tab's main terminal.
        let terminal = {
            let Some(tab) = ws.tab_manager.active_tab() else {
                return StepOutcome::Failed(format!("chord {chord} -> PTY: no active tab"));
            };
            if let Some(ref pane_manager) = tab.pane_manager
                && let Some(focused_pane) = pane_manager.focused_pane()
            {
                std::sync::Arc::clone(&focused_pane.terminal)
            } else {
                std::sync::Arc::clone(&tab.terminal)
            }
        };
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let byte_count = bytes.len();
        // Write synchronously when the lock is free so the step record's
        // byte claim is true at record time — a spawned write can be
        // starved or dropped at app exit, which a byte-exact sink assert
        // then reports as lost input. The spawn fallback preserves delivery
        // under momentary contention (same shape as the keyboard path).
        // The guard must drop before `terminal`/`bytes` move into the
        // fallback spawn, so the attempt runs in its own block.
        let mut write_result: Result<(), String> = Ok(());
        let mut contended = false;
        {
            match terminal.try_read() {
                Ok(term) => write_result = term.write(&bytes).map_err(|e| e.to_string()),
                Err(_) => contended = true,
            }
        }
        if contended {
            ws.runtime.spawn(async move {
                let term = terminal.read().await;
                if let Err(e) = term.write(&bytes) {
                    crate::debug_error!("INPUT", "PTY write failed (ui-test passthrough): {e}");
                }
            });
        }
        match write_result {
            Ok(()) => StepOutcome::Performed(format!(
                "chord {chord} -> {via} -> PTY ({byte_count} bytes: {hex})"
            )),
            Err(e) => StepOutcome::Failed(format!("chord {chord} -> PTY write failed: {e}")),
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
                    "plugin_overlay_interactive" => ws
                        .status_bar_ui
                        .plugin_host()
                        .overlays()
                        .values()
                        .any(|o| o.interactive),
                    "plugin_overlay_focused" => {
                        ws.status_bar_ui.plugin_host().focused_overlay().is_some()
                    }
                    "plugin_overlay_event" => {
                        ws.status_bar_ui
                            .plugin_host()
                            .overlay_events_dispatched_count()
                            > 0
                    }
                    "plugin_action_dispatched" => ws.status_bar_ui.any_plugin_action_dispatched(),
                    "modal_guard" => ws.any_modal_ui_visible(),
                    "tmux_picker_open" => ws.overlay_ui.tmux_session_picker_ui.visible,
                    "command_history_open" => ws.overlay_ui.command_history_ui.visible,
                    "clipboard_history_open" => ws.overlay_ui.clipboard_history_ui.visible,
                    "paste_special_open" => ws.overlay_ui.paste_special_ui.visible,
                    "palette_selected_visible" => {
                        ws.overlay_ui.command_palette.selected_row_is_visible()
                    }
                    "pane_hint_mode_active" => ws.pane_hint_select.is_active(),
                    "egui_keyboard" => ws.is_egui_using_keyboard(),
                    "fullscreen" => ws.window.as_ref().is_some_and(|w| w.fullscreen().is_some()),
                    _ => return None,
                }
            }
        })
    }

    /// Current value of a capture-capable keyed operand — the ones whose
    /// value a script cannot know up front (a spawned shell's PID). The
    /// literal-comparison operands stay in [`Self::ui_test_value`].
    fn ui_test_operand(&self, what: &str) -> Result<String, String> {
        match what {
            // The focused terminal's PTY child PID: the identity a
            // preserve-shell close/reopen must keep (card 01a0eafef7ee7490a7c9bce3cc52ddcd).
            "tab_shell_pid" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("tab_shell_pid: no terminal window".into());
                };
                let Some(tab) = ws.tab_manager.active_tab() else {
                    return Err("tab_shell_pid: no active tab".into());
                };
                match tab.try_with_read_terminal(|t| t.get_shell_pid()) {
                    Some(Some(pid)) => Ok(pid.to_string()),
                    Some(None) => {
                        Err("tab_shell_pid: PTY child pid unavailable (session exited?)".into())
                    }
                    None => Err("tab_shell_pid: terminal lock contention".into()),
                }
            }
            _ => Err(format!(
                "operand '{what}' has no capturable value (see AGENT_UI_VERIFICATION.md)"
            )),
        }
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
            // The palette's selected row index into the filtered list — the
            // B62 scroll proof asserts it alongside palette_selected_visible.
            "palette_selected" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("palette_selected: no terminal window".into());
                };
                let actual = ws.overlay_ui.command_palette.selected_index().to_string();
                Ok((actual.clone(), actual == expected))
            }
            // The app's open-window count — the readout a session-restore
            // proof asserts on (TW2: quit must save every window).
            "window_count" => {
                let actual = self.windows.len().to_string();
                Ok((actual.clone(), actual == expected))
            }
            // The selected command's text — the B70 navigation proof pairs
            // it with arrow presses (selection moved to the expected row).
            "command_history_selected" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("command_history_selected: no terminal window".into());
                };
                let actual = ws
                    .overlay_ui
                    .command_history_ui
                    .selected_command()
                    .unwrap_or("<none>".into());
                Ok((actual.clone(), actual == expected))
            }
            // The selected clipboard entry's content — B70, same shape as
            // command_history_selected.
            "clipboard_history_selected" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("clipboard_history_selected: no terminal window".into());
                };
                let actual = ws
                    .overlay_ui
                    .clipboard_history_ui
                    .selected_entry()
                    .map(|e| e.content.clone())
                    .unwrap_or("<none>".into());
                Ok((actual.clone(), actual == expected))
            }
            // Live font size — the B68 reset proof (decrease moves it off
            // the configured value, reset must land back on it, not 14.0).
            "font_size" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("font_size: no terminal window".into());
                };
                let actual = ws.config.load().font_size.to_string();
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
            // Byte-exact PTY-sink readout, the delivery half of a
            // pass-to-terminal proof: `["file_bytes", "<path>:<hex>"]`.
            // `"<path>*<hex>"` asserts the sink ENDS with the hex run — for
            // scripts run on a live desktop, where the app window can steal
            // focus mid-run and catch real keystrokes ahead of the scripted
            // ones (observed: stray "cc-" and TAB contaminating the line).
            // Unlike file_empty, a missing file is a failure — a delivery
            // proof must show the bytes arrived.
            "file_bytes" => {
                let (path, expected_hex, exact) = if let Some((p, h)) = expected.split_once(':') {
                    (p, h, true)
                } else if let Some((p, h)) = expected.split_once('*') {
                    (p, h, false)
                } else {
                    return Err("file_bytes: expected '<path>:<hex>' or '<path>*<hex>'".into());
                };
                let expected_hex = expected_hex.to_ascii_lowercase();
                let actual = std::fs::read(path)
                    .map(|bytes| bytes.iter().map(|b| format!("{b:02x}")).collect::<String>())
                    .unwrap_or_else(|_| "missing".to_string());
                let ok = actual == expected_hex || (!exact && actual.ends_with(&expected_hex));
                Ok((actual, ok))
            }
            // The rendered grid of every mux-attached pane, the readout a
            // resize proof asserts on: one pane reports bare "WxH", more
            // report "paneid=WxH" joined by commas in ascending pane order.
            "mux_pane_grid" => {
                let Some(ws) = self.ui_test_window_state() else {
                    return Err("mux_pane_grid: no terminal window".into());
                };
                let owners = &ws.tmux_state.tmux_pane_owners;
                if owners.is_empty() {
                    return Err("mux_pane_grid: no par-mux panes attached".into());
                }
                let mut entries: Vec<(u64, String)> = Vec::new();
                for (mux_id, (tab_id, pane_id)) in owners {
                    let Some(tab) = ws.tab_manager.get_tab(*tab_id) else {
                        return Err(format!(
                            "mux_pane_grid: tab {tab_id:?} holding %{mux_id} not found"
                        ));
                    };
                    let Some(pm) = tab.pane_manager() else {
                        return Err(format!(
                            "mux_pane_grid: tab {tab_id:?} holding %{mux_id} has no panes"
                        ));
                    };
                    let Some(pane) = pm.get_pane(*pane_id) else {
                        return Err(format!(
                            "mux_pane_grid: pane {pane_id:?} for %{mux_id} not found"
                        ));
                    };
                    let (cols, rows) = match pane.terminal.try_read() {
                        Ok(term) => term.dimensions(),
                        Err(e) => {
                            return Err(format!("mux_pane_grid: %{mux_id} terminal lock: {e}"));
                        }
                    };
                    entries.push((*mux_id, format!("{cols}x{rows}")));
                }
                entries.sort_by_key(|(id, _)| *id);
                let actual = if entries.len() == 1 {
                    entries
                        .into_iter()
                        .next()
                        .map(|(_, g)| g)
                        .unwrap_or_default()
                } else {
                    entries
                        .into_iter()
                        .map(|(id, g)| format!("%{id}={g}"))
                        .collect::<Vec<_>>()
                        .join(",")
                };
                Ok((actual.clone(), actual == expected))
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

    #[test]
    fn ui_test_action_parses_capture_steps() {
        let step: UiTestStep =
            serde_json::from_str(r#"{"capture": "tab_shell_pid"}"#).expect("parses");
        assert!(matches!(
            step.action,
            UiTestAction::Capture { ref capture } if capture == "tab_shell_pid"
        ));

        let step: UiTestStep =
            serde_json::from_str(r#"{"assert_eq_captured": "tab_shell_pid"}"#).expect("parses");
        assert!(matches!(
            step.action,
            UiTestAction::AssertEqCaptured {
                ref assert_eq_captured
            } if assert_eq_captured == "tab_shell_pid"
        ));
    }
}
