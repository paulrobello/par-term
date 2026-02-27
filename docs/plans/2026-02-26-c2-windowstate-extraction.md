# C2 WindowState Extraction — AgentState, TmuxState, OverlayUiState

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Extract three sub-structs from `WindowState` to reduce its ~82 top-level fields (C2 audit finding).

**Architecture:** Add `src/app/agent_state.rs`, `src/app/tmux_state.rs`, `src/app/overlay_ui_state.rs` following the existing pattern of `CursorAnimState` and `ShaderState`. Each extracted struct becomes a named field on `WindowState` (e.g. `self.agent_state.agent_rx` instead of `self.agent_rx`). Tasks must run **sequentially** — each commits before the next starts — because all touch `window_state.rs`.

**Tech Stack:** Rust 2024, cargo, `make build` for dev-release, `make lint` for clippy, `make test`.

**Established pattern to follow:**
- `src/app/cursor_anim_state.rs` — simple struct + `Default` impl
- `src/app/shader_state.rs` — struct + `new()` with parameters
- Access pattern: `self.cursor_anim.field_name` throughout all `src/app/` files

**Key borrow-checker constraint (read before Task 1):**
`process_agent_messages_tick()` currently does:
```rust
if let Some(rx) = &mut self.agent_rx {
    while let Ok(msg) = rx.try_recv() {
        self.ai_inspector.chat.flush_agent_message(); // accesses other fields
        self.agent_skill_failure_detected = true;     // accesses other fields
    }
}
```
After extraction, `agent_rx` and the other agent fields will all be in `AgentState`. Borrowing `self.agent_state.agent_rx` mutably would prevent access to `self.agent_state.agent_skill_failure_detected` etc. **Fix:** Add a `drain_messages()` helper on `AgentState` that collects all pending messages into a `Vec<AgentMessage>`, return early, then process the Vec against `self` with full field access.

---

## Task 1: Extract AgentState

**Files:**
- Create: `src/app/agent_state.rs`
- Modify: `src/app/window_state.rs`
- Modify: `src/app/window_manager.rs`
- Modify: `src/app/mod.rs` (add `pub(crate) mod agent_state;`)

**Fields to move from `WindowState` into `AgentState`:**
```
agent_rx: Option<mpsc::UnboundedReceiver<AgentMessage>>
agent_tx: Option<mpsc::UnboundedSender<AgentMessage>>
agent: Option<Arc<tokio::sync::Mutex<Agent>>>
agent_client: Option<Arc<par_term_acp::JsonRpcClient>>
pending_send_handles: std::collections::VecDeque<tokio::task::JoinHandle<()>>
agent_skill_failure_detected: bool
agent_skill_recovery_attempts: u8
pending_agent_context_replay: Option<String>
last_auto_context_sent_at: Option<std::time::Instant>
available_agents: Vec<AgentConfig>
```

**Fields that stay on `WindowState` (do NOT move):**
- `ai_inspector` — moves in Task 3 (OverlayUiState)
- `config_changed_by_agent` — coordination flag to WindowManager, stays on WindowState

### Step 1: Create src/app/agent_state.rs

```rust
//! ACP agent connection and runtime state for a window.
//!
//! Groups the fields that manage the ACP agent lifecycle: the async channel,
//! the agent handle, the JSON-RPC client, pending send queue, error recovery
//! counters, and the list of available agent configs.

use par_term_acp::{Agent, AgentConfig, AgentMessage};
use std::sync::Arc;
use tokio::sync::mpsc;

/// ACP agent connection and runtime state.
pub(crate) struct AgentState {
    /// ACP agent message receiver
    pub(crate) agent_rx: Option<mpsc::UnboundedReceiver<AgentMessage>>,
    /// ACP agent message sender (kept to signal prompt completion)
    pub(crate) agent_tx: Option<mpsc::UnboundedSender<AgentMessage>>,
    /// ACP agent (managed via tokio)
    pub(crate) agent: Option<Arc<tokio::sync::Mutex<Agent>>>,
    /// ACP JSON-RPC client for sending responses without locking the agent.
    /// Stored separately to avoid deadlocks: `send_prompt` holds the agent lock
    /// while waiting for the prompt response, but the agent's tool calls
    /// need us to respond via this same client.
    pub(crate) agent_client: Option<Arc<par_term_acp::JsonRpcClient>>,
    /// Handles for queued send tasks (waiting on agent lock).
    /// Used to abort queued sends when the user cancels a pending message.
    pub(crate) pending_send_handles: std::collections::VecDeque<tokio::task::JoinHandle<()>>,
    /// Tracks whether the current prompt encountered a recoverable local
    /// backend tool failure or malformed inline XML-style tool markup.
    pub(crate) agent_skill_failure_detected: bool,
    /// Bounded automatic recovery retries after recoverable ACP tool failures.
    pub(crate) agent_skill_recovery_attempts: u8,
    /// One-shot transcript replay prompt injected into the next user prompt
    /// after reconnecting/switching agents.
    pub(crate) pending_agent_context_replay: Option<String>,
    /// Timestamp of the last command auto-context sent to the agent.
    pub(crate) last_auto_context_sent_at: Option<std::time::Instant>,
    /// Available agent configs
    pub(crate) available_agents: Vec<AgentConfig>,
}

impl AgentState {
    pub(crate) fn new(available_agents: Vec<AgentConfig>) -> Self {
        Self {
            agent_rx: None,
            agent_tx: None,
            agent: None,
            agent_client: None,
            pending_send_handles: std::collections::VecDeque::new(),
            agent_skill_failure_detected: false,
            agent_skill_recovery_attempts: 0,
            pending_agent_context_replay: None,
            last_auto_context_sent_at: None,
            available_agents,
        }
    }

    /// Drain all currently-available messages from `agent_rx` into a Vec.
    ///
    /// This avoids a double-borrow: callers can hold a `&mut self.agent_state`
    /// borrow only long enough to drain, then process the returned messages
    /// against the full `WindowState` without any borrow conflict.
    pub(crate) fn drain_messages(&mut self) -> Vec<AgentMessage> {
        let mut messages = Vec::new();
        if let Some(rx) = &mut self.agent_rx {
            while let Ok(msg) = rx.try_recv() {
                messages.push(msg);
            }
        }
        messages
    }
}
```

### Step 2: Add module declaration in src/app/mod.rs

In `src/app/mod.rs`, add alongside the other `pub(crate) mod` lines:
```rust
pub(crate) mod agent_state;
```

### Step 3: Replace the 10 fields in WindowState struct definition

In `src/app/window_state.rs`, in the `pub struct WindowState { ... }` block, **remove** all 10 individual agent fields and **replace** them with a single line:
```rust
    /// ACP agent connection and runtime state
    pub(crate) agent_state: crate::app::agent_state::AgentState,
```

Also remove the now-unused individual `use` imports for the agent types that are re-imported inside agent_state.rs. Keep any imports still needed elsewhere in window_state.rs (e.g. `AgentStatus`, `AgentMessage`, `AgentConfig` are still used in function bodies).

### Step 4: Update WindowState::new()

In `WindowState::new()`, replace the 10 individual field initializations:
```rust
// OLD (10 lines):
agent_rx: None,
agent_tx: None,
agent: None,
agent_client: None,
pending_send_handles: std::collections::VecDeque::new(),
agent_skill_failure_detected: false,
agent_skill_recovery_attempts: 0,
pending_agent_context_replay: None,
last_auto_context_sent_at: None,
available_agents,

// NEW (1 line):
agent_state: crate::app::agent_state::AgentState::new(available_agents),
```

### Step 5: Bulk-rename field accesses in window_state.rs

Use sed to rename all `self.` accesses (window_state.rs is an `impl WindowState` so all uses are `self.`):
```bash
cd /Users/probello/Repos/par-term
sed -i '' \
  -e 's/self\.agent_rx\b/self.agent_state.agent_rx/g' \
  -e 's/self\.agent_tx\b/self.agent_state.agent_tx/g' \
  -e 's/self\.agent_client\b/self.agent_state.agent_client/g' \
  -e 's/self\.pending_send_handles\b/self.agent_state.pending_send_handles/g' \
  -e 's/self\.agent_skill_failure_detected\b/self.agent_state.agent_skill_failure_detected/g' \
  -e 's/self\.agent_skill_recovery_attempts\b/self.agent_state.agent_skill_recovery_attempts/g' \
  -e 's/self\.pending_agent_context_replay\b/self.agent_state.pending_agent_context_replay/g' \
  -e 's/self\.last_auto_context_sent_at\b/self.agent_state.last_auto_context_sent_at/g' \
  -e 's/self\.available_agents\b/self.agent_state.available_agents/g' \
  src/app/window_state.rs
```

**Handle `self.agent` separately** — it will also match `self.agent_state`, `self.agent_rx` etc. if done naively. Use a more targeted replace:
```bash
sed -i '' 's/self\.agent\b/self.agent_state.agent/g' src/app/window_state.rs
```
Then verify no double-prefix was introduced:
```bash
grep "self\.agent_state\.agent_state" src/app/window_state.rs
```
Should return empty. Fix any occurrences manually.

### Step 6: Rename accesses in window_manager.rs

```bash
sed -i '' \
  -e 's/window_state\.agent_rx\b/window_state.agent_state.agent_rx/g' \
  -e 's/window_state\.agent_tx\b/window_state.agent_state.agent_tx/g' \
  -e 's/window_state\.agent_client\b/window_state.agent_state.agent_client/g' \
  -e 's/window_state\.pending_send_handles\b/window_state.agent_state.pending_send_handles/g' \
  -e 's/window_state\.agent_skill_failure_detected\b/window_state.agent_state.agent_skill_failure_detected/g' \
  -e 's/window_state\.agent_skill_recovery_attempts\b/window_state.agent_state.agent_skill_recovery_attempts/g' \
  -e 's/window_state\.pending_agent_context_replay\b/window_state.agent_state.pending_agent_context_replay/g' \
  -e 's/window_state\.last_auto_context_sent_at\b/window_state.agent_state.last_auto_context_sent_at/g' \
  -e 's/window_state\.available_agents\b/window_state.agent_state.available_agents/g' \
  -e 's/window_state\.agent\b/window_state.agent_state.agent/g' \
  src/app/window_manager.rs
```
Check for double-prefix: `grep "agent_state\.agent_state" src/app/window_manager.rs`

### Step 7: Refactor process_agent_messages_tick() for the drain pattern

Find `fn process_agent_messages_tick` in `window_state.rs` (around line 4956). The function currently opens with:
```rust
if let Some(rx) = &mut self.agent_rx {
    while let Ok(msg) = rx.try_recv() {
```

Replace the message-collection section to use `drain_messages()`:
```rust
fn process_agent_messages_tick(&mut self) {
    let mut saw_prompt_complete_this_tick = false;

    // Process agent messages
    let msg_count_before = self.ai_inspector.chat.messages.len();
    // Drain messages first to avoid double-borrow of agent_state while processing.
    type ConfigUpdateEntry = (
        std::collections::HashMap<String, serde_json::Value>,
        tokio::sync::oneshot::Sender<Result<(), String>>,
    );
    let mut pending_config_updates: Vec<ConfigUpdateEntry> = Vec::new();
    let messages = self.agent_state.drain_messages();
    for msg in messages {
        match msg {
            // ... (all the match arms stay identical, just the outer loop changes)
```

The key change: `if let Some(rx) = &mut self.agent_state.agent_rx { while let Ok(msg) = rx.try_recv() { match msg {` becomes `let messages = self.agent_state.drain_messages(); for msg in messages { match msg {`.

All the match arm bodies remain unchanged — they already access `self.ai_inspector`, `self.agent_state.agent_skill_failure_detected`, etc. which is now fine since `drain_messages()` has returned.

### Step 8: Also update refresh_available_agents()

```rust
pub(crate) fn refresh_available_agents(&mut self) {
    let config_dir = dirs::config_dir().unwrap_or_default().join("par-term");
    let discovered_agents = discover_agents(&config_dir);
    self.agent_state.available_agents = merge_custom_ai_inspector_agents(
        discovered_agents,
        &self.config.ai_inspector_custom_agents,
    );
}
```

### Step 9: Build and fix

```bash
cargo build --profile dev-release 2>&1 | head -60
```

Fix all compiler errors. Common patterns:
- Any remaining `self.agent_X` not yet prefixed → add `agent_state.`
- Import errors → ensure agent_state.rs has correct `use` statements
- Borrow conflicts → check drain_messages() is being used correctly

### Step 10: Lint and test

```bash
make lint
make test
```

### Step 11: Commit

```bash
git add src/app/agent_state.rs src/app/mod.rs src/app/window_state.rs src/app/window_manager.rs
git commit -m "refactor(audit): C2 — extract AgentState from WindowState"
```

---

## Task 2: Extract TmuxState

**Precondition:** Task 1 committed.

**Files:**
- Create: `src/app/tmux_state.rs`
- Modify: `src/app/window_state.rs`
- Modify: `src/app/window_manager.rs` (if it references tmux fields)
- Modify: `src/app/tmux_handler/gateway.rs`
- Modify: `src/app/tmux_handler/notifications.rs`
- Modify: `src/app/mod.rs` (add `pub(crate) mod tmux_state;`)

**Fields to move from `WindowState` into `TmuxState`:**
```
tmux_session: Option<TmuxSession>
tmux_sync: TmuxSync
tmux_session_name: Option<String>
tmux_gateway_tab_id: Option<TabId>
tmux_prefix_key: Option<crate::tmux::PrefixKey>
tmux_prefix_state: crate::tmux::PrefixState
tmux_pane_to_native_pane: std::collections::HashMap<crate::tmux::TmuxPaneId, crate::pane::PaneId>
native_pane_to_tmux_pane: std::collections::HashMap<crate::pane::PaneId, crate::tmux::TmuxPaneId>
```

**Fields that stay on `WindowState` for now:**
- `tmux_session_picker_ui` — moves in Task 3
- `tmux_status_bar_ui` — moves in Task 3

### Step 1: Create src/app/tmux_state.rs

```rust
//! tmux integration state for a window.
//!
//! Groups the fields that manage tmux control-mode connectivity: the session
//! handle, sync manager, pane-ID mappings, and prefix-key state machine.

use crate::tab::TabId;
use crate::tmux::{PrefixKey, PrefixState, TmuxPaneId, TmuxSession, TmuxSync};
use crate::pane::PaneId;

/// tmux integration state.
pub(crate) struct TmuxState {
    /// tmux control mode session (if connected)
    pub(crate) tmux_session: Option<TmuxSession>,
    /// tmux state synchronization manager
    pub(crate) tmux_sync: TmuxSync,
    /// Current tmux session name (for window title display)
    pub(crate) tmux_session_name: Option<String>,
    /// Tab ID where the tmux gateway connection lives (where we write commands)
    pub(crate) tmux_gateway_tab_id: Option<TabId>,
    /// Parsed prefix key from config (cached for performance)
    pub(crate) tmux_prefix_key: Option<PrefixKey>,
    /// Prefix key state (whether we're waiting for command key)
    pub(crate) tmux_prefix_state: PrefixState,
    /// Mapping from tmux pane IDs to native pane IDs for output routing
    pub(crate) tmux_pane_to_native_pane: std::collections::HashMap<TmuxPaneId, PaneId>,
    /// Reverse mapping from native pane IDs to tmux pane IDs for input routing
    pub(crate) native_pane_to_tmux_pane: std::collections::HashMap<PaneId, TmuxPaneId>,
}

impl TmuxState {
    pub(crate) fn new(tmux_prefix_key: Option<PrefixKey>) -> Self {
        Self {
            tmux_session: None,
            tmux_sync: TmuxSync::new(),
            tmux_session_name: None,
            tmux_gateway_tab_id: None,
            tmux_prefix_key,
            tmux_prefix_state: PrefixState::new(),
            tmux_pane_to_native_pane: std::collections::HashMap::new(),
            native_pane_to_tmux_pane: std::collections::HashMap::new(),
        }
    }
}
```

Check the actual import paths by looking at how `TmuxSession`, `TmuxSync`, `PrefixKey`, `PrefixState`, `TmuxPaneId` are imported in `window_state.rs` (`use crate::tmux::{...}`) and replicate.

### Step 2: Add module declaration in src/app/mod.rs

```rust
pub(crate) mod tmux_state;
```

### Step 3: Replace fields in WindowState struct

In the `pub struct WindowState { ... }` block, remove the 8 tmux fields and add:
```rust
    /// tmux integration state
    pub(crate) tmux_state: crate::app::tmux_state::TmuxState,
```

### Step 4: Update WindowState::new()

Replace the 8 individual tmux initializations:
```rust
// OLD:
tmux_session: None,
tmux_sync: TmuxSync::new(),
tmux_session_name: None,
tmux_gateway_tab_id: None,
tmux_prefix_key,
tmux_prefix_state: crate::tmux::PrefixState::new(),
tmux_pane_to_native_pane: std::collections::HashMap::new(),
native_pane_to_tmux_pane: std::collections::HashMap::new(),

// NEW:
tmux_state: crate::app::tmux_state::TmuxState::new(tmux_prefix_key),
```

### Step 5: Bulk-rename in window_state.rs

```bash
sed -i '' \
  -e 's/self\.tmux_session\b/self.tmux_state.tmux_session/g' \
  -e 's/self\.tmux_sync\b/self.tmux_state.tmux_sync/g' \
  -e 's/self\.tmux_session_name\b/self.tmux_state.tmux_session_name/g' \
  -e 's/self\.tmux_gateway_tab_id\b/self.tmux_state.tmux_gateway_tab_id/g' \
  -e 's/self\.tmux_prefix_key\b/self.tmux_state.tmux_prefix_key/g' \
  -e 's/self\.tmux_prefix_state\b/self.tmux_state.tmux_prefix_state/g' \
  -e 's/self\.tmux_pane_to_native_pane\b/self.tmux_state.tmux_pane_to_native_pane/g' \
  -e 's/self\.native_pane_to_tmux_pane\b/self.tmux_state.native_pane_to_tmux_pane/g' \
  src/app/window_state.rs
```

Verify no double-prefix: `grep "tmux_state\.tmux_state" src/app/window_state.rs`

### Step 6: Bulk-rename in tmux_handler/gateway.rs and tmux_handler/notifications.rs

In the tmux_handler files, `WindowState` fields are accessed via the `self` or a `ws` / `window_state` binding. Check the actual binding name used:
```bash
grep -n "tmux_session\|tmux_sync\|tmux_pane\|native_pane\|tmux_prefix" src/app/tmux_handler/gateway.rs | head -20
grep -n "tmux_session\|tmux_sync\|tmux_pane\|native_pane\|tmux_prefix" src/app/tmux_handler/notifications.rs | head -20
```

Then apply the appropriate renames. Example if the binding is `self`:
```bash
sed -i '' \
  -e 's/self\.tmux_session\b/self.tmux_state.tmux_session/g' \
  -e 's/self\.tmux_sync\b/self.tmux_state.tmux_sync/g' \
  ...
  src/app/tmux_handler/gateway.rs src/app/tmux_handler/notifications.rs
```

Also update `window_manager.rs` if it references any of these fields directly.

### Step 7: Build and fix

```bash
cargo build --profile dev-release 2>&1 | head -60
```

Fix all compiler errors. Common issues:
- Import paths in tmux_state.rs — check `crate::tmux::*` vs `crate::pane::PaneId`
- Any remaining un-renamed accesses in tmux_handler files

### Step 8: Lint and test

```bash
make lint
make test
```

### Step 9: Commit

```bash
git add src/app/tmux_state.rs src/app/mod.rs src/app/window_state.rs src/app/window_manager.rs src/app/tmux_handler/gateway.rs src/app/tmux_handler/notifications.rs
git commit -m "refactor(audit): C2 — extract TmuxState from WindowState"
```

---

## Task 3: Extract OverlayUiState

**Precondition:** Task 2 committed.

**Files:**
- Create: `src/app/overlay_ui_state.rs`
- Modify: `src/app/window_state.rs`
- Modify: `src/app/window_manager.rs`
- Modify: `src/app/keyboard_handlers.rs`
- Modify: `src/app/handler/window_state_impl.rs`
- Modify: `src/app/tab_ops.rs`
- Modify: `src/app/search_highlight.rs`
- Modify: `src/app/mouse_events.rs`
- Modify: `src/app/input_events/keybinding_actions.rs`
- Modify: `src/app/input_events/key_handler.rs`
- Modify: `src/app/mod.rs` (add `pub(crate) mod overlay_ui_state;`)

**Fields to move from `WindowState` into `OverlayUiState`:**
```
help_ui: HelpUI
clipboard_history_ui: ClipboardHistoryUI
command_history_ui: CommandHistoryUI
command_history: CommandHistory
synced_commands: std::collections::HashSet<String>
paste_special_ui: PasteSpecialUI
tmux_session_picker_ui: TmuxSessionPickerUI
tmux_status_bar_ui: TmuxStatusBarUI
search_ui: SearchUI
ai_inspector: AIInspectorPanel
last_inspector_width: f32
shader_install_ui: ShaderInstallUI
shader_install_receiver: Option<std::sync::mpsc::Receiver<Result<usize, String>>>
integrations_ui: IntegrationsUI
close_confirmation_ui: CloseConfirmationUI
quit_confirmation_ui: QuitConfirmationUI
remote_shell_install_ui: RemoteShellInstallUI
ssh_connect_ui: SshConnectUI
profile_drawer_ui: ProfileDrawerUI
profile_manager: ProfileManager
```

That is 20 fields. Note that `command_history` and `profile_manager` are data models but are tightly paired with their corresponding UI panels; co-locating them avoids split ownership.

### Step 1: Create src/app/overlay_ui_state.rs

Copy the `use` imports from `window_state.rs` for all the UI panel types, then declare the struct. Look at the actual `use` statements in `window_state.rs` to get exact import paths. The struct:

```rust
//! Overlay UI panel state for a window.
//!
//! Groups all transient overlay/modal/side-panel UI components together.
//! This reduces WindowState's field count and localises all panel visibility
//! logic to one struct.

use crate::ai_inspector::panel::AIInspectorPanel;
use crate::clipboard_history_ui::ClipboardHistoryUI;
use crate::close_confirmation_ui::CloseConfirmationUI;
use crate::command_history::CommandHistory;
use crate::command_history_ui::CommandHistoryUI;
use crate::config::Config;
use crate::help_ui::HelpUI;
use crate::integrations_ui::IntegrationsUI;
use crate::paste_special_ui::PasteSpecialUI;
use crate::profile::{ProfileManager, storage as profile_storage};
use crate::profile_drawer_ui::ProfileDrawerUI;
use crate::quit_confirmation_ui::QuitConfirmationUI;
use crate::remote_shell_install_ui::RemoteShellInstallUI;
use crate::search::SearchUI;
use crate::shader_install_ui::ShaderInstallUI;
use crate::ssh_connect_ui::SshConnectUI;
use crate::tmux_session_picker_ui::TmuxSessionPickerUI;
use crate::tmux_status_bar_ui::TmuxStatusBarUI;
use anyhow::Result;

/// All transient overlay / modal / side-panel UI state for a window.
pub(crate) struct OverlayUiState {
    pub(crate) help_ui: HelpUI,
    pub(crate) clipboard_history_ui: ClipboardHistoryUI,
    pub(crate) command_history_ui: CommandHistoryUI,
    /// Persistent command history model (backing command_history_ui)
    pub(crate) command_history: CommandHistory,
    /// Commands already synced from marks (avoids repeated adds)
    pub(crate) synced_commands: std::collections::HashSet<String>,
    pub(crate) paste_special_ui: PasteSpecialUI,
    pub(crate) tmux_session_picker_ui: TmuxSessionPickerUI,
    pub(crate) tmux_status_bar_ui: TmuxStatusBarUI,
    pub(crate) search_ui: SearchUI,
    pub(crate) ai_inspector: AIInspectorPanel,
    /// Last known AI Inspector panel consumed width (logical pixels).
    pub(crate) last_inspector_width: f32,
    pub(crate) shader_install_ui: ShaderInstallUI,
    /// Receiver for shader installation results (from background thread)
    pub(crate) shader_install_receiver: Option<std::sync::mpsc::Receiver<Result<usize, String>>>,
    pub(crate) integrations_ui: IntegrationsUI,
    pub(crate) close_confirmation_ui: CloseConfirmationUI,
    pub(crate) quit_confirmation_ui: QuitConfirmationUI,
    pub(crate) remote_shell_install_ui: RemoteShellInstallUI,
    pub(crate) ssh_connect_ui: SshConnectUI,
    pub(crate) profile_drawer_ui: ProfileDrawerUI,
    pub(crate) profile_manager: ProfileManager,
}

impl OverlayUiState {
    pub(crate) fn new(config: &Config) -> Self {
        let command_history_max = config.command_history_max_entries;
        let profile_manager = match profile_storage::load_profiles() {
            Ok(manager) => manager,
            Err(e) => {
                log::warn!("Failed to load profiles: {}", e);
                ProfileManager::new()
            }
        };
        Self {
            help_ui: HelpUI::new(),
            clipboard_history_ui: ClipboardHistoryUI::new(),
            command_history_ui: CommandHistoryUI::new(),
            command_history: {
                let mut ch = CommandHistory::new(command_history_max);
                ch.load();
                ch
            },
            synced_commands: std::collections::HashSet::new(),
            paste_special_ui: PasteSpecialUI::new(),
            tmux_session_picker_ui: TmuxSessionPickerUI::new(),
            tmux_status_bar_ui: TmuxStatusBarUI::new(),
            search_ui: SearchUI::new(),
            ai_inspector: AIInspectorPanel::new(config),
            last_inspector_width: 0.0,
            shader_install_ui: ShaderInstallUI::new(),
            shader_install_receiver: None,
            integrations_ui: IntegrationsUI::new(),
            close_confirmation_ui: CloseConfirmationUI::new(),
            quit_confirmation_ui: QuitConfirmationUI::new(),
            remote_shell_install_ui: RemoteShellInstallUI::new(),
            ssh_connect_ui: SshConnectUI::new(),
            profile_drawer_ui: ProfileDrawerUI::new(),
            profile_manager,
        }
    }
}
```

**Note:** `WindowState::new()` currently constructs `ai_inspector` and `profile_manager` before the `Self { ... }` block because they need `&config` before it is moved. With the extraction, `OverlayUiState::new(&config)` handles this internally — remove the pre-construction of `ai_inspector` and `profile_manager` from `WindowState::new()`.

### Step 2: Add module declaration in src/app/mod.rs

```rust
pub(crate) mod overlay_ui_state;
```

### Step 3: Replace 20 fields in WindowState struct

Remove all 20 overlay fields and replace with:
```rust
    /// Overlay / modal / side-panel UI state
    pub(crate) overlay_ui: crate::app::overlay_ui_state::OverlayUiState,
```

### Step 4: Update WindowState::new()

Remove all 20 individual initializations (and the pre-construction of `ai_inspector`, `profile_manager` above the `Self { ... }` block), and replace with:
```rust
overlay_ui: crate::app::overlay_ui_state::OverlayUiState::new(&config),
```

### Step 5: Bulk-rename all field accesses

Run sed across all affected files. Do **not** replace in overlay_ui_state.rs itself. The access prefix for `impl WindowState` methods is `self.`:

```bash
FILES=(
  src/app/window_state.rs
  src/app/window_manager.rs
  src/app/keyboard_handlers.rs
  src/app/handler/window_state_impl.rs
  src/app/tab_ops.rs
  src/app/search_highlight.rs
  src/app/mouse_events.rs
  src/app/input_events/keybinding_actions.rs
  src/app/input_events/key_handler.rs
)

for f in "${FILES[@]}"; do
  sed -i '' \
    -e 's/\bself\.help_ui\b/self.overlay_ui.help_ui/g' \
    -e 's/\bself\.clipboard_history_ui\b/self.overlay_ui.clipboard_history_ui/g' \
    -e 's/\bself\.command_history_ui\b/self.overlay_ui.command_history_ui/g' \
    -e 's/\bself\.command_history\b/self.overlay_ui.command_history/g' \
    -e 's/\bself\.synced_commands\b/self.overlay_ui.synced_commands/g' \
    -e 's/\bself\.paste_special_ui\b/self.overlay_ui.paste_special_ui/g' \
    -e 's/\bself\.tmux_session_picker_ui\b/self.overlay_ui.tmux_session_picker_ui/g' \
    -e 's/\bself\.tmux_status_bar_ui\b/self.overlay_ui.tmux_status_bar_ui/g' \
    -e 's/\bself\.search_ui\b/self.overlay_ui.search_ui/g' \
    -e 's/\bself\.ai_inspector\b/self.overlay_ui.ai_inspector/g' \
    -e 's/\bself\.last_inspector_width\b/self.overlay_ui.last_inspector_width/g' \
    -e 's/\bself\.shader_install_ui\b/self.overlay_ui.shader_install_ui/g' \
    -e 's/\bself\.shader_install_receiver\b/self.overlay_ui.shader_install_receiver/g' \
    -e 's/\bself\.integrations_ui\b/self.overlay_ui.integrations_ui/g' \
    -e 's/\bself\.close_confirmation_ui\b/self.overlay_ui.close_confirmation_ui/g' \
    -e 's/\bself\.quit_confirmation_ui\b/self.overlay_ui.quit_confirmation_ui/g' \
    -e 's/\bself\.remote_shell_install_ui\b/self.overlay_ui.remote_shell_install_ui/g' \
    -e 's/\bself\.ssh_connect_ui\b/self.overlay_ui.ssh_connect_ui/g' \
    -e 's/\bself\.profile_drawer_ui\b/self.overlay_ui.profile_drawer_ui/g' \
    -e 's/\bself\.profile_manager\b/self.overlay_ui.profile_manager/g' \
    "$f"
done
```

**Also handle `window_state.` prefix** used in `window_manager.rs`:
```bash
sed -i '' \
  -e 's/window_state\.help_ui\b/window_state.overlay_ui.help_ui/g' \
  -e 's/window_state\.clipboard_history_ui\b/window_state.overlay_ui.clipboard_history_ui/g' \
  -e 's/window_state\.ai_inspector\b/window_state.overlay_ui.ai_inspector/g' \
  -e 's/window_state\.profile_manager\b/window_state.overlay_ui.profile_manager/g' \
  -e 's/window_state\.profile_drawer_ui\b/window_state.overlay_ui.profile_drawer_ui/g' \
  ... (etc for all 20 fields with window_state. prefix)
  src/app/window_manager.rs
```

Check for double-prefix after each sed run:
```bash
grep "overlay_ui\.overlay_ui" src/app/window_state.rs src/app/window_manager.rs
```

### Step 6: Fix cross-struct borrow issues

After extraction, `self.overlay_ui.ai_inspector.show(ctx, &self.agent_state.available_agents)` borrows different structs so it's fine.

The one pattern to watch for: anything like:
```rust
self.overlay_ui.search_ui.something(&mut self.overlay_ui.command_history)
```
This would be a double-borrow of `overlay_ui`. Fix by extracting both out first:
```rust
let history = &mut self.overlay_ui.command_history;
self.overlay_ui.search_ui.something(history); // still borrows overlay_ui twice
```
Actually that also fails. Real fix — use temp vars:
```rust
let input = self.overlay_ui.search_ui.get_input();
self.overlay_ui.command_history.do_something(input);
```
Search for such patterns by looking at compiler errors and fix them case by case.

### Step 7: Fix any_modal_ui_visible() and similar methods

These methods in `window_state.rs` call multiple overlay fields. After extraction they become `self.overlay_ui.X`. No borrow issues — each field access is sequential within the function.

### Step 8: Build and fix

```bash
cargo build --profile dev-release 2>&1 | head -80
```

This will produce the most errors of the three tasks. Work through them systematically:
1. Missing imports in overlay_ui_state.rs (check against window_state.rs imports)
2. Double-prefix bugs from sed
3. Borrow-checker errors from simultaneous overlay_ui sub-field access
4. Unused import warnings in window_state.rs (remove moved imports)

### Step 9: Clean up window_state.rs imports

After compilation succeeds, remove `use` statements in `window_state.rs` that were only needed for moved types (e.g. `use crate::help_ui::HelpUI` if HelpUI is now only in overlay_ui_state.rs). Use:
```bash
make lint 2>&1 | grep "unused import"
```
Remove each flagged import.

### Step 10: Lint and test

```bash
make lint
make test
```

### Step 11: Update AUDIT.md

In `AUDIT.md`, update finding C2 to reflect that all three sub-structs are now extracted. Update the "Remaining Findings" section to reflect reduced scope.

### Step 12: Commit

```bash
git add src/app/overlay_ui_state.rs src/app/mod.rs src/app/window_state.rs src/app/window_manager.rs \
        src/app/keyboard_handlers.rs src/app/handler/window_state_impl.rs src/app/tab_ops.rs \
        src/app/search_highlight.rs src/app/mouse_events.rs src/app/input_events/keybinding_actions.rs \
        src/app/input_events/key_handler.rs AUDIT.md
git commit -m "refactor(audit): C2 — extract OverlayUiState from WindowState"
```

---

## Verification Across All Three Tasks

After all three tasks are committed, run:
```bash
make checkall
```
Expected: zero new clippy warnings, all tests pass.

Count remaining WindowState fields:
```bash
grep -c "pub(crate)" src/app/window_state.rs
```
Should be significantly reduced from ~82.

Check file sizes:
```bash
wc -l src/app/window_state.rs src/app/agent_state.rs src/app/tmux_state.rs src/app/overlay_ui_state.rs
```
