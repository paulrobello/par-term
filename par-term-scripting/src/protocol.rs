//! JSON protocol types for communication between the terminal and script subprocesses.
//!
//! Scripts read [`ScriptEvent`] objects from stdin (one JSON object per line) and write
//! [`ScriptCommand`] objects to stdout (one JSON object per line).
//!
//! # Security Model
//!
//! ## Trust Assumptions
//!
//! Scripts are user-configured subprocesses launched from `ScriptConfig` entries in
//! `~/.config/par-term/config.yaml`. The script binary is implicitly trusted (it was
//! placed there by the user). However, this trust must be bounded because:
//!
//! 1. **Supply-chain attacks**: A malicious package could replace a trusted script
//!    with one that emits dangerous command payloads.
//! 2. **Injection through event data**: Malicious terminal sequences could produce
//!    events whose payloads are forwarded to the script, which could reflect them
//!    back in commands (terminal injection risk).
//! 3. **Compromised scripts**: A script may be modified after initial deployment.
//!
//! ## Command Categories
//!
//! Script commands fall into three security categories:
//!
//! ### Safe Commands (no permission required)
//! - `Log`: Write to the script's output buffer (UI only)
//! - `SetPanel` / `ClearPanel`: Display markdown content in a panel
//! - `Notify`: Show a desktop notification
//! - `SetBadge`: Set the tab badge text
//! - `SetVariable`: Set a user variable
//! - `SetWidget`: Plugin-only status-bar text; a tab *script* sending it gets
//!   an error line (see the variant docs below)
//!
//! ### Restricted Commands (require permission flags)
//! These commands require explicit opt-in via `ScriptConfig` permission fields:
//! - `WriteText`: Inject text into the PTY (requires `allow_write_text: true`)
//!   - Must strip VT/ANSI escape sequences before writing
//!   - Subject to rate limiting
//!   - Queued for user confirmation unless `prompt_before_write_text: false`
//!     (see [`crate::confirm`]) — stripping escapes leaves the printable text
//!     and newline that make up a command line
//! - `RunCommand`: Spawn an external process (requires `allow_run_command: true`)
//!   - Must check against `check_command_denylist()` from par-term-config
//!   - Must use shell tokenization (not `/bin/sh -c`) to prevent metacharacter injection
//!   - Subject to rate limiting
//! - `ChangeConfig`: Modify terminal configuration (requires `allow_change_config: true`)
//!   - Must validate config keys against an allowlist
//!
//! ## Implementation Status
//!
//! All commands are implemented:
//! - `Log`, `SetPanel`, `ClearPanel`: Safe, always allowed
//! - `Notify`, `SetBadge`, `SetVariable`: Safe, always allowed
//! - `WriteText`: Requires `allow_write_text`, rate-limited, VT sequences stripped,
//!   and confirmed by the user unless `prompt_before_write_text` is off
//! - `RunCommand`: Requires `allow_run_command`, rate-limited, denylist-checked,
//!   tokenised without shell invocation
//! - `ChangeConfig`: Requires `allow_change_config`, allowlisted keys only
//!
//! ## Dispatcher Responsibility
//!
//! The command dispatcher in `src/app/window_manager/scripting/mod.rs` is responsible for:
//! 1. Checking `command.requires_permission()` before executing restricted commands
//! 2. Verifying the corresponding `ScriptConfig.allow_*` flag is set
//! 3. Applying rate limits, denylists, and input sanitization
//! 4. Routing `WriteText` through the confirmation dialog when it is not
//!    pre-approved
//!
//! See `par-term-scripting/SECURITY.md` for the complete security model.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Event kind for [`ScriptEventData::PluginActionInvoked`]: a user invoked a
/// plugin-contributed palette action.
pub const PLUGIN_ACTION_INVOKED_KIND: &str = "plugin_action_invoked";

/// Event kind for [`ScriptEventData::OverlayEvent`]: a focused overlay's
/// widget reported a semantic interaction (click / text_changed / select).
pub const OVERLAY_EVENT_KIND: &str = "overlay_event";

/// An event sent from the terminal to a script subprocess (via stdin).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ScriptEvent {
    /// Event kind name (e.g., "bell_rang", "cwd_changed", "command_complete").
    pub kind: String,
    /// Event-specific payload.
    pub data: ScriptEventData,
}

/// Event-specific payload data.
///
/// Tagged with `data_type` so the JSON includes a discriminant field for Python scripts
/// to easily dispatch on.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "data_type")]
pub enum ScriptEventData {
    /// Empty payload for events that carry no additional data (e.g., BellRang).
    Empty {},

    /// The current working directory changed.
    CwdChanged {
        /// New working directory path.
        cwd: String,
    },

    /// A command completed execution.
    CommandComplete {
        /// The command that completed.
        command: String,
        /// Exit code, if available.
        exit_code: Option<i32>,
    },

    /// The terminal title changed.
    TitleChanged {
        /// New terminal title.
        title: String,
    },

    /// The terminal size changed.
    SizeChanged {
        /// Number of columns.
        cols: usize,
        /// Number of rows.
        rows: usize,
    },

    /// A user variable changed.
    VariableChanged {
        /// Variable name.
        name: String,
        /// New value.
        value: String,
        /// Previous value, if any.
        old_value: Option<String>,
    },

    /// An environment variable changed.
    EnvironmentChanged {
        /// Environment variable key.
        key: String,
        /// New value.
        value: String,
        /// Previous value, if any.
        old_value: Option<String>,
    },

    /// The badge text changed.
    BadgeChanged {
        /// New badge text, or None if cleared.
        text: Option<String>,
    },

    /// A trigger pattern was matched.
    TriggerMatched {
        /// The trigger pattern that matched.
        pattern: String,
        /// The text that matched.
        matched_text: String,
        /// Line number where the match occurred.
        line: usize,
    },

    /// A semantic zone event occurred.
    ZoneEvent {
        /// Zone identifier.
        zone_id: u64,
        /// Type of zone.
        zone_type: String,
        /// Event type (e.g., "enter", "exit").
        event: String,
    },

    /// A plugin-contributed palette action was invoked.
    ///
    /// Tab scripts never receive this variant: it is written directly to the
    /// contributing plugin process's stdin when the host invokes one of its
    /// actions, and is never routed through the observer fan-out that
    /// carries other events to scripts.
    PluginActionInvoked {
        /// Manifest id of the invoked action.
        action: String,
    },

    /// Fallback for unmapped events. Carries arbitrary key-value fields.
    Generic {
        /// Arbitrary event fields.
        fields: HashMap<String, serde_json::Value>,
    },

    /// A semantic event from a focused interactive overlay's own widget
    /// (overlay kind only). Written directly to the overlay process's
    /// stdin when the host's renderer reports a widget interaction —
    /// never raw keys (design: input & focus routing).
    OverlayEvent {
        /// Overlay id the widget belongs to.
        overlay: String,
        /// The widget's scene id.
        widget: String,
        /// What happened on the widget.
        event: OverlayWidgetEvent,
    },
}

/// What a focused overlay widget reports to the plugin process.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum OverlayWidgetEvent {
    /// A button was clicked.
    Click {},
    /// A text input's value changed.
    TextChanged {
        /// Current value of the input.
        value: String,
    },
    /// A list row was selected.
    Select {
        /// Index of the selected row.
        index: usize,
    },
}

/// A command sent from a script subprocess to the terminal (via stdout).
///
/// Tagged with `type` for easy JSON dispatch.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ScriptCommand {
    /// Write text to the PTY.
    WriteText {
        /// Text to write.
        text: String,
    },

    /// Show a desktop notification.
    Notify {
        /// Notification title.
        title: String,
        /// Notification body.
        body: String,
    },

    /// Set the tab badge text.
    SetBadge {
        /// Badge text to display.
        text: String,
    },

    /// Set a user variable.
    SetVariable {
        /// Variable name.
        name: String,
        /// Variable value.
        value: String,
    },

    /// Execute a shell command.
    RunCommand {
        /// Command to execute.
        command: String,
    },

    /// Change a configuration value.
    ChangeConfig {
        /// Configuration key.
        key: String,
        /// New value.
        value: serde_json::Value,
    },

    /// Log a message.
    Log {
        /// Log level (e.g., "info", "warn", "error", "debug").
        level: String,
        /// Log message.
        message: String,
    },

    /// Set a markdown panel.
    SetPanel {
        /// Panel title.
        title: String,
        /// Markdown content.
        content: String,
    },

    /// Clear the markdown panel.
    ClearPanel {},

    /// Upsert a plugin-owned overlay surface (overlay kind only). Idempotent
    /// by id: same id replaces the whole overlay. Phase 1 is display-only —
    /// the host forces `interactive` off pending the manifest capability.
    SetOverlay {
        /// Overlay id; unique within the plugin.
        id: String,
        /// Position: a named anchor or a free rect (fractions of window).
        position: OverlayPosition,
        /// Size as fractions of the window (0.0–1.0).
        size: OverlaySize,
        /// Opacity (0.0–1.0, default 1.0).
        #[serde(default = "overlay_default_opacity")]
        opacity: f32,
        /// Interactive request; forced off without the manifest capability.
        #[serde(default)]
        interactive: bool,
        /// Scene to render (declarative tree).
        content: OverlayScene,
    },

    /// Clear a plugin-owned overlay by id (overlay kind only).
    ClearOverlay {
        /// Overlay id to remove.
        id: String,
    },

    /// Set the text of a plugin-provided status-bar widget.
    ///
    /// Display-only and plugin-sourced: no permission flag exists for it
    /// because a widget's text is strictly less powerful than `SetBadge`
    /// (which scripts already send unrestricted). A tab *script* emitting
    /// this command gets an error line in its output pane — the plugin host
    /// is the only consumer.
    SetWidget {
        /// Widget text to display (last write wins; empty text hides the widget).
        text: String,
    },
}

fn overlay_default_opacity() -> f32 {
    1.0
}

/// Where a plugin overlay sits: a named anchor or a free rect as window
/// fractions. Free rects are clamped on-screen by the host.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case", untagged)]
pub enum OverlayPosition {
    /// One of nine named positions (corners, edge midpoints, center) or an
    /// edge strip (`top-strip`, `bottom-strip`, `left-strip`, `right-strip`).
    Anchor(OverlayAnchor),
    /// Free rect: x/y/width/height as fractions of the window (0.0–1.0).
    Free {
        #[serde(default)]
        x: f32,
        #[serde(default)]
        y: f32,
    },
}

/// Named overlay anchors (kebab-case on the wire).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OverlayAnchor {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
    /// Top edge strip spanning the window width.
    TopStrip,
    /// Bottom edge strip spanning the window width.
    BottomStrip,
    /// Left edge strip spanning the window height.
    LeftStrip,
    /// Right edge strip spanning the window height.
    RightStrip,
}

/// Overlay size as fractions of the window, clamped by the host (half the
/// window per axis).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct OverlaySize {
    /// Width fraction (0.0–1.0).
    pub w: f32,
    /// Height fraction (0.0–1.0).
    pub h: f32,
}

/// A declarative scene tree the host renders through egui. Phase 1
/// vocabulary: text, row, markdown; phase 2 adds the interactive
/// button / text_input / list (rendered only while the overlay may
/// focus — see `PluginOverlay::interactive`). No images (deferred by
/// design).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum OverlayScene {
    /// Plain text.
    Text { text: String },
    /// Children laid out horizontally.
    Row { children: Vec<OverlayScene> },
    /// Markdown block.
    Markdown { text: String },
    /// Clickable button (interactive vocabulary, O3).
    Button {
        /// Scene-unique widget id the click event names.
        id: String,
        /// Button label.
        label: String,
    },
    /// Single-line text input (interactive vocabulary, O3).
    TextInput {
        /// Scene-unique widget id events name.
        id: String,
        /// Current value; full-scene replace semantics (the plugin owns
        /// state).
        value: String,
        /// Placeholder shown when empty.
        #[serde(default)]
        placeholder: String,
    },
    /// Selectable row list (interactive vocabulary, O3).
    List {
        /// Scene-unique widget id events name.
        id: String,
        /// Row labels.
        items: Vec<String>,
        /// Currently selected row index, if any.
        #[serde(default)]
        selected: Option<usize>,
    },
}

/// The host-side stored form of a live overlay. `interactive` is true only
/// when the request asked for it AND the plugin's manifest carries the
/// `overlay.interactive` capability (phase 2) — the ingest path forces it
/// off otherwise.
#[derive(Debug, Clone, PartialEq)]
pub struct PluginOverlay {
    /// Overlay id (unique within the plugin).
    pub id: String,
    /// Anchor or free-rect position.
    pub position: OverlayPosition,
    /// Size as window fractions.
    pub size: OverlaySize,
    /// Opacity (0.0–1.0).
    pub opacity: f32,
    /// Whether the overlay may take focus and render interactive widgets.
    pub interactive: bool,
    /// Scene to render.
    pub content: OverlayScene,
}

/// Strip VT/ANSI escape sequences from text before PTY injection.
///
/// Removes CSI (`ESC[`), OSC (`ESC]`), DCS (`ESC P`), APC (`ESC _`),
/// PM (`ESC ^`), SOS (`ESC X`) sequences, and bare two-byte `ESC x`
/// sequences. Printable characters and newlines are passed through.
///
/// This is required for safe `WriteText` dispatch: a script must not be
/// able to embed control sequences that reposition the cursor, exfiltrate
/// data, or otherwise corrupt the terminal state.
pub fn strip_vt_sequences(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();

    while let Some(c) = chars.next() {
        if c != '\x1b' {
            result.push(c);
            continue;
        }
        // ESC seen — classify and skip the sequence
        match chars.peek().copied() {
            Some('[') => {
                // CSI: ESC [ ... <final-byte>
                chars.next(); // consume '['
                while let Some(&ch) = chars.peek() {
                    chars.next();
                    if ch.is_ascii_alphabetic() || ch == '@' || ch == '`' {
                        break;
                    }
                }
            }
            Some(']') => {
                // OSC: ESC ] ... BEL or ST (ESC \)
                chars.next(); // consume ']'
                while let Some(ch) = chars.next() {
                    if ch == '\x07' {
                        break;
                    }
                    if ch == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            Some('P') | Some('_') | Some('^') | Some('X') => {
                // DCS / APC / PM / SOS: ESC <type> ... ST (ESC \)
                chars.next(); // consume the type byte
                while let Some(ch) = chars.next() {
                    if ch == '\x1b' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            Some('(') | Some(')') | Some('*') | Some('+') => {
                // Character-set designation: ESC ( x — skip two bytes
                chars.next();
                chars.next();
            }
            Some(_) => {
                // Generic two-byte ESC sequence — skip one byte
                chars.next();
            }
            None => {}
        }
    }
    result
}

impl ScriptCommand {
    /// Returns `true` if this command requires explicit permission in the script config.
    ///
    /// Commands that return `true` must have their corresponding `allow_*` flag set
    /// in `ScriptConfig` before the dispatcher will execute them.
    ///
    /// # Security Classification
    ///
    /// | Command | Requires Permission | Risk Level |
    /// |---------|--------------------| -----------|
    /// | `Log` | No | Low (UI output only) |
    /// | `SetPanel` / `ClearPanel` | No | Low (UI display only) |
    /// | `Notify` | No | Low (desktop notification) |
    /// | `SetBadge` | No | Low (tab badge display) |
    /// | `SetVariable` | No | Low (user variable storage) |
    /// | `WriteText` | **Yes** | High (PTY injection, command execution) |
    /// | `RunCommand` | **Yes** | Critical (arbitrary process spawn) |
    /// | `ChangeConfig` | **Yes** | High (config modification) |
    pub fn requires_permission(&self) -> bool {
        matches!(
            self,
            ScriptCommand::RunCommand { .. }
                | ScriptCommand::WriteText { .. }
                | ScriptCommand::ChangeConfig { .. }
        )
    }

    /// Returns the name of the permission flag required to execute this command.
    ///
    /// Returns `None` for commands that don't require permission.
    /// The returned string corresponds to a field in `ScriptConfig`:
    /// - `"allow_run_command"` for `RunCommand`
    /// - `"allow_write_text"` for `WriteText`
    /// - `"allow_change_config"` for `ChangeConfig`
    pub fn permission_flag_name(&self) -> Option<&'static str> {
        match self {
            ScriptCommand::RunCommand { .. } => Some("allow_run_command"),
            ScriptCommand::WriteText { .. } => Some("allow_write_text"),
            ScriptCommand::ChangeConfig { .. } => Some("allow_change_config"),
            _ => None,
        }
    }

    /// Returns `true` if this command can safely be executed without rate limiting.
    ///
    /// Commands that may be emitted frequently (like `Log`) should not be rate-limited
    /// to avoid dropping important debug output. High-impact commands (`WriteText`,
    /// `RunCommand`) must be rate-limited to prevent abuse.
    pub fn is_rate_limited(&self) -> bool {
        matches!(
            self,
            ScriptCommand::RunCommand { .. } | ScriptCommand::WriteText { .. }
        )
    }

    /// Returns a human-readable name for this command type (for logging/errors).
    pub fn command_name(&self) -> &'static str {
        match self {
            ScriptCommand::WriteText { .. } => "WriteText",
            ScriptCommand::Notify { .. } => "Notify",
            ScriptCommand::SetBadge { .. } => "SetBadge",
            ScriptCommand::SetVariable { .. } => "SetVariable",
            ScriptCommand::RunCommand { .. } => "RunCommand",
            ScriptCommand::ChangeConfig { .. } => "ChangeConfig",
            ScriptCommand::Log { .. } => "Log",
            ScriptCommand::SetPanel { .. } => "SetPanel",
            ScriptCommand::ClearPanel {} => "ClearPanel",
            ScriptCommand::SetOverlay { .. } => "SetOverlay",
            ScriptCommand::ClearOverlay { .. } => "ClearOverlay",
            ScriptCommand::SetWidget { .. } => "SetWidget",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_overlay_round_trips_through_serde() {
        let line = r##"{"type":"SetOverlay","id":"hud","position":"top-right","size":{"w":0.2,"h":0.1},"opacity":0.8,"content":{"type":"markdown","text":"# hello"}}"##;
        let cmd: ScriptCommand = serde_json::from_str(line).expect("parse SetOverlay line");
        match &cmd {
            ScriptCommand::SetOverlay {
                id,
                position,
                size,
                opacity,
                interactive,
                content,
            } => {
                assert_eq!(id, "hud");
                assert_eq!(position, &OverlayPosition::Anchor(OverlayAnchor::TopRight));
                assert_eq!((size.w, size.h), (0.2, 0.1));
                assert_eq!(*opacity, 0.8);
                assert!(!interactive, "interactive defaults off");
                assert_eq!(
                    content,
                    &OverlayScene::Markdown {
                        text: "# hello".to_string()
                    }
                );
            }
            other => panic!("wrong variant: {other:?}"),
        }
        let re = serde_json::to_string(&cmd).expect("serialize SetOverlay");
        let back: ScriptCommand = serde_json::from_str(&re).expect("round-trip");
        assert_eq!(back, cmd);
    }

    #[test]
    fn clear_overlay_parses() {
        let line = r#"{"type":"ClearOverlay","id":"hud"}"#;
        let cmd: ScriptCommand = serde_json::from_str(line).expect("parse ClearOverlay line");
        assert_eq!(
            cmd,
            ScriptCommand::ClearOverlay {
                id: "hud".to_string()
            }
        );
    }

    #[test]
    fn set_overlay_free_rect_position_parses() {
        let line = r#"{"type":"SetOverlay","id":"f","position":{"x":0.1,"y":0.2},"size":{"w":0.3,"h":0.3},"content":{"type":"text","text":"t"}}"#;
        let cmd: ScriptCommand = serde_json::from_str(line).expect("parse free-rect SetOverlay");
        match cmd {
            ScriptCommand::SetOverlay { position, .. } => {
                assert_eq!(position, OverlayPosition::Free { x: 0.1, y: 0.2 });
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn set_widget_round_trips_through_serde() {
        let line = r#"{"type":"SetWidget","text":"🕒 14:32"}"#;
        let cmd: ScriptCommand = serde_json::from_str(line).expect("parse SetWidget line");
        assert_eq!(
            cmd,
            ScriptCommand::SetWidget {
                text: "🕒 14:32".into()
            }
        );
        let re = serde_json::to_string(&cmd).expect("serialize SetWidget");
        let back: ScriptCommand = serde_json::from_str(&re).expect("round-trip");
        assert_eq!(back, cmd);
    }

    #[test]
    fn set_widget_requires_no_permission_and_is_not_rate_limited() {
        let cmd = ScriptCommand::SetWidget {
            text: String::new(),
        };
        assert!(!cmd.requires_permission());
        assert!(cmd.permission_flag_name().is_none());
        assert!(!cmd.is_rate_limited());
        assert_eq!(cmd.command_name(), "SetWidget");
    }

    #[test]
    fn interactive_scene_nodes_round_trip_through_serde() {
        let scene = OverlayScene::Row {
            children: vec![
                OverlayScene::Button {
                    id: "deploy".to_string(),
                    label: "Deploy".to_string(),
                },
                OverlayScene::TextInput {
                    id: "filter".to_string(),
                    value: "par".to_string(),
                    placeholder: "filter…".to_string(),
                },
                OverlayScene::List {
                    id: "jobs".to_string(),
                    items: vec!["one".to_string(), "two".to_string()],
                    selected: Some(1),
                },
            ],
        };
        let json = serde_json::to_string(&scene).expect("serialize scene");
        let back: OverlayScene = serde_json::from_str(&json).expect("parse scene back");
        assert_eq!(back, scene);

        // Wire shapes stay snake_case with placeholder/selected optional.
        assert!(json.contains(r#""type":"button""#), "got: {json}");
        assert!(json.contains(r#""type":"text_input""#), "got: {json}");
        assert!(json.contains(r#""type":"list""#), "got: {json}");
        let no_optional: OverlayScene =
            serde_json::from_str(r#"{"type":"text_input","id":"f","value":""}"#)
                .expect("optional fields default");
        assert_eq!(
            no_optional,
            OverlayScene::TextInput {
                id: "f".to_string(),
                value: String::new(),
                placeholder: String::new(),
            }
        );
    }

    #[test]
    fn overlay_event_round_trips_through_serde() {
        for data in [
            ScriptEventData::OverlayEvent {
                overlay: "hud".to_string(),
                widget: "deploy".to_string(),
                event: OverlayWidgetEvent::Click {},
            },
            ScriptEventData::OverlayEvent {
                overlay: "hud".to_string(),
                widget: "filter".to_string(),
                event: OverlayWidgetEvent::TextChanged {
                    value: "par".to_string(),
                },
            },
            ScriptEventData::OverlayEvent {
                overlay: "hud".to_string(),
                widget: "jobs".to_string(),
                event: OverlayWidgetEvent::Select { index: 2 },
            },
        ] {
            let json = serde_json::to_string(&data).expect("serialize event data");
            let back: ScriptEventData = serde_json::from_str(&json).expect("parse back");
            assert_eq!(back, data);
        }
        // One exact wire shape: nested tag + flat fields, plugin-parseable.
        let json = serde_json::to_string(&ScriptEventData::OverlayEvent {
            overlay: "hud".to_string(),
            widget: "deploy".to_string(),
            event: OverlayWidgetEvent::Click {},
        })
        .expect("serialize");
        assert_eq!(
            json,
            r#"{"data_type":"OverlayEvent","overlay":"hud","widget":"deploy","event":{"type":"Click"}}"#
        );
    }

    #[test]
    fn plugin_action_invoked_round_trips_through_serde() {
        let data = ScriptEventData::PluginActionInvoked {
            action: "greet".to_string(),
        };
        let json = serde_json::to_string(&data).expect("serialize event data");
        assert_eq!(
            json, r#"{"data_type":"PluginActionInvoked","action":"greet"}"#,
            "the wire shape plugin processes parse must stay exact"
        );
        let back: ScriptEventData = serde_json::from_str(&json).expect("parse event data back");
        assert_eq!(back, data);

        let event = ScriptEvent {
            kind: PLUGIN_ACTION_INVOKED_KIND.to_string(),
            data,
        };
        let line = serde_json::to_string(&event).expect("serialize event");
        assert_eq!(
            line,
            r#"{"kind":"plugin_action_invoked","data":{"data_type":"PluginActionInvoked","action":"greet"}}"#
        );
        let parsed: ScriptEvent = serde_json::from_str(&line).expect("parse event line back");
        assert_eq!(parsed, event);
    }
}
