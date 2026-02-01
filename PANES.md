# Split Panes & Native tmux Integration

This document specifies the implementation plan for split panes (MATRIX.md §15) and native tmux integration (MATRIX.md §19). These features are bundled because tmux pane mapping requires split pane support, and both share fundamental architecture.

## Overview

### Why Bundle These Features?

1. **tmux pane mapping requires split panes** - Cannot map tmux panes to native splits without split pane support
2. **Shared architecture** - Both features need pane layout management, focus tracking, and resize handling
3. **Incremental delivery** - Split panes alone is valuable; tmux integration builds on top

### Core Library Status

par-term-emu-core-rust already provides complete tmux control mode support:
- Complete tmux control mode parser (`src/tmux_control.rs`)
- All 24 notification types implemented
- Terminal integration (`set_tmux_control_mode()`, `drain_tmux_notifications()`)
- 42 unit tests for protocol parsing

**No additional core library work needed** - all implementation is in par-term frontend.

---

## Section 15: Split Panes

### Features

| Feature | Priority | Effort | Description |
|---------|----------|--------|-------------|
| Horizontal split | ⭐⭐⭐ | 🔵 Very High | Split terminal horizontally (panes stacked vertically) |
| Vertical split | ⭐⭐⭐ | 🔵 Very High | Split terminal vertically (panes side by side) |
| Pane navigation | ⭐⭐⭐ | 🔵 Very High | Move focus between panes with keyboard |
| Pane resizing | ⭐⭐⭐ | 🔵 Very High | Resize pane boundaries with keyboard/mouse |
| Dim inactive panes | ⭐⭐ | 🟢 Low | Visual focus indicator for inactive panes |
| Per-pane titles | ⭐⭐ | 🟡 Medium | Show title bar for each pane |
| Per-pane background | ⭐ | 🟡 Medium | Different background images per pane |
| Broadcast input | ⭐⭐ | 🟡 Medium | Type to multiple panes simultaneously |
| Division view | ⭐⭐ | 🟢 Low | Visible pane divider lines |

### Architecture Requirements

#### Pane Tree Data Structure

```
Window
└── Tab
    └── PaneNode (enum)
        ├── Leaf { terminal_id, bounds }
        └── Split { direction, ratio, children: [PaneNode, PaneNode] }
```

Key considerations:
- Binary tree allows arbitrary nesting of splits
- Each leaf node owns a `TerminalManager` instance
- Bounds are calculated recursively from parent dimensions
- Ratio (0.0-1.0) determines split position

#### Pane Manager Component

New `src/app/pane_manager.rs` module:
- `PaneManager` struct owns the pane tree
- Methods: `split_horizontal()`, `split_vertical()`, `close_pane()`, `resize_pane()`
- Focus tracking with `focused_pane_id`
- Layout calculation on window resize

#### Rendering Changes

- `CellRenderer` needs per-pane viewport support
- Each pane renders to its own region of the window
- Dividers rendered between panes (customizable color/width)
- Inactive pane dimming via shader uniform or post-process

#### Input Routing

- Mouse events routed to pane under cursor
- Keyboard input routed to focused pane
- Pane navigation: `Cmd+Opt+Arrow` (macOS), `Ctrl+Alt+Arrow` (Linux/Windows)
- Pane resize: `Cmd+Opt+Shift+Arrow` (macOS), `Ctrl+Alt+Shift+Arrow` (Linux/Windows)

### Keyboard Shortcuts (iTerm2 Compatible)

| Action | macOS | Linux/Windows |
|--------|-------|---------------|
| Split horizontally | `Cmd+D` | `Ctrl+Shift+D` |
| Split vertically | `Cmd+Shift+D` | `Ctrl+Shift+E` |
| Navigate left | `Cmd+Opt+Left` | `Ctrl+Alt+Left` |
| Navigate right | `Cmd+Opt+Right` | `Ctrl+Alt+Right` |
| Navigate up | `Cmd+Opt+Up` | `Ctrl+Alt+Up` |
| Navigate down | `Cmd+Opt+Down` | `Ctrl+Alt+Down` |
| Resize (make larger) | `Cmd+Opt+Shift+Arrow` | `Ctrl+Alt+Shift+Arrow` |
| Close pane | `Cmd+W` (if multiple panes) | `Ctrl+Shift+W` |
| Toggle broadcast | `Cmd+Opt+I` | `Ctrl+Alt+I` |

---

## Section 19: tmux Integration

### Features

| Feature | Priority | Effort | Description |
|---------|----------|--------|-------------|
| tmux control mode (`-CC`) | ⭐⭐⭐ | 🔵 Very High | Core protocol for native integration |
| tmux windows as native tabs | ⭐⭐⭐ | 🔵 Very High | Map tmux windows to par-term tabs |
| tmux panes as native splits | ⭐⭐⭐ | 🔵 Very High | Map tmux panes to native split panes |
| tmux session picker UI | ⭐⭐ | 🟡 Medium | List/attach sessions from GUI |
| tmux status bar in UI | ⭐⭐ | 🟡 Medium | Display status outside terminal area |
| tmux clipboard sync | ⭐⭐ | 🟡 Medium | Sync with tmux paste buffers |
| tmux pause mode handling | ⭐⭐ | 🟡 Medium | Handle slow connection pausing |
| Auto-attach on launch | ⭐⭐ | 🟢 Low | Option to auto-attach to session |
| tmux profile auto-switching | ⭐ | 🟡 Medium | Different profile for tmux sessions |

### How tmux Control Mode Works

1. **Protocol**: par-term connects via `tmux -CC` which outputs structured commands instead of terminal escape sequences
2. **Window Management**: tmux windows become par-term tabs with native UI
3. **Pane Management**: tmux panes become par-term split panes with native dividers
4. **Seamless Experience**: Users interact with native UI while tmux manages sessions server-side
5. **Session Persistence**: Closing par-term doesn't kill tmux; sessions persist and can be reattached

### Notification Types (Already Implemented in Core)

The core library (`par-term-emu-core-rust`) already parses all tmux control mode notifications:

- `%begin`, `%end`, `%error` - Command response markers
- `%client-detached`, `%client-session-changed` - Client events
- `%continue`, `%pause` - Flow control
- `%exit` - Session exit
- `%extended-output` - Extended output data
- `%layout-change` - **Pane layout changed** (critical for split sync)
- `%output` - Pane output data
- `%pane-mode-changed` - Pane mode transitions
- `%session-changed`, `%session-renamed`, `%session-window-changed` - Session events
- `%sessions-changed` - Session list changed
- `%subscription` - Subscription data
- `%unlinked-window-add`, `%unlinked-window-close`, `%unlinked-window-renamed` - Unlinked window events
- `%window-add`, `%window-close`, `%window-pane-changed`, `%window-renamed` - **Window events** (critical for tab sync)

### Architecture Requirements

#### tmux Session Manager

New `src/tmux/` module directory:
- `mod.rs` - Public API and session manager
- `session.rs` - Session state and lifecycle
- `commands.rs` - tmux command builders
- `sync.rs` - Bidirectional state synchronization

#### Integration Points

1. **Tab Creation** - When tmux sends `%window-add`, create native tab
2. **Tab Close** - When tmux sends `%window-close`, close native tab
3. **Pane Layout** - When tmux sends `%layout-change`, update split layout
4. **Output Routing** - Route `%output` to correct pane's terminal
5. **Input Routing** - Send user input to tmux for routing to correct pane

#### Configuration Options

```yaml
# tmux integration settings
tmux_auto_attach: false           # Auto-attach to last session on launch
tmux_default_session: ""          # Session name to auto-attach
tmux_show_status_bar: true        # Show tmux status in par-term UI
tmux_clipboard_sync: true         # Bidirectional clipboard sync
```

---

## Implementation Phases

### Phase 1: Split Pane Foundation (Weeks 1-3)

**Goal**: Basic split panes working without tmux

- [ ] Design and implement pane tree data structure
- [ ] Add `PaneManager` to `AppState`
- [ ] Implement horizontal split (`Cmd+D`)
- [ ] Implement vertical split (`Cmd+Shift+D`)
- [ ] Pane focus tracking and visual indicator
- [ ] Keyboard navigation (`Cmd+Opt+Arrow`)
- [ ] Close pane functionality

**Deliverable**: Can split terminal and navigate between panes

### Phase 2: Split Pane Polish (Weeks 4-5)

**Goal**: Complete split pane feature set

- [ ] Pane resizing with keyboard shortcuts
- [ ] Pane resizing with divider drag (mouse)
- [ ] Division view (visual dividers with customizable style)
- [ ] Dim inactive panes (shader-based dimming)
- [ ] Per-pane titles (optional title bar)
- [ ] Settings UI for split pane options

**Deliverable**: Production-ready split panes

### Phase 3: tmux Control Mode (Weeks 6-8)

**Goal**: Basic tmux integration

- [ ] Create `src/tmux/` module structure
- [ ] Implement tmux session spawner (`tmux -CC`)
- [ ] Wire core library's notification parser to frontend
- [ ] Map `%window-add`/`%window-close` to tab creation/destruction
- [ ] Route `%output` to correct terminal instance
- [ ] Send user input through tmux control protocol
- [ ] Basic session display in window title

**Deliverable**: Can use tmux with native tab management

### Phase 4: tmux Pane Integration (Weeks 9-11)

**Goal**: Full pane synchronization

- [ ] Parse `%layout-change` notifications
- [ ] Map tmux pane layouts to native split panes
- [ ] Handle pane creation/destruction via tmux
- [ ] Bidirectional clipboard sync with tmux paste buffers
- [ ] Handle pause/continue for slow connections
- [ ] Session picker UI (egui dialog)

**Deliverable**: Full tmux integration with native panes

### Phase 5: Advanced Features (Weeks 12-14)

**Goal**: Complete feature parity with iTerm2

- [ ] Broadcast input mode (type to multiple panes)
- [ ] Per-pane backgrounds (optional feature)
- [ ] tmux status bar rendering in par-term UI
- [ ] Auto-attach configuration option
- [ ] tmux profile auto-switching (if profiles implemented)
- [ ] Comprehensive settings UI integration

**Deliverable**: Feature-complete tmux integration

---

## Dependencies and Blockers

### Hard Dependencies

1. **Split panes must come before tmux pane mapping** - Cannot map tmux panes without native split support

### Soft Dependencies

1. **Profiles feature** - tmux profile auto-switching depends on profiles (can be skipped initially)
2. **Settings UI** - All settings should be exposed in Settings UI (incremental)

### Technical Risks

1. **Layout calculation complexity** - Recursive layout with arbitrary nesting is non-trivial
2. **Renderer changes** - Multi-viewport rendering may require significant refactoring
3. **tmux version compatibility** - Control mode protocol may vary between tmux versions
4. **Performance** - Many panes = many terminal instances = potential memory/CPU concerns

### Mitigation Strategies

1. Start with simple binary splits before complex layouts
2. Add pane count limits to prevent resource exhaustion
3. Test with tmux 3.0+ (control mode is most stable in recent versions)
4. Profile memory usage with increasing pane counts

---

## Acceptance Criteria

### Split Panes

- [ ] Can split horizontally and vertically with keyboard shortcuts
- [ ] Can navigate between panes with keyboard
- [ ] Can resize panes with keyboard and mouse
- [ ] Visual feedback shows focused pane
- [ ] Closing last pane closes the tab
- [ ] All keyboard shortcuts match iTerm2 defaults
- [ ] Settings UI exposes all split pane options

### tmux Integration

- [ ] Can spawn tmux with `-CC` flag from menu or config
- [ ] tmux windows appear as native tabs
- [ ] tmux panes appear as native split panes
- [ ] Closing a native tab closes the tmux window
- [ ] Closing a native pane closes the tmux pane
- [ ] Session picker shows available sessions
- [ ] Can detach from session (session persists)
- [ ] Can reattach to existing session
- [ ] Clipboard syncs bidirectionally with tmux

---

## References

- [MATRIX.md §15](./MATRIX.md#15-split-panes) - Split Panes feature comparison
- [MATRIX.md §19](./MATRIX.md#19-tmux-integration) - tmux Integration feature comparison
- [iTerm2 tmux Integration](https://iterm2.com/documentation-tmux-integration.html) - iTerm2's implementation
- [tmux Control Mode](https://github.com/tmux/tmux/wiki/Control-Mode) - Protocol documentation
- [par-term-emu-core-rust](https://github.com/paulrobello/par-term-emu-core-rust) - Core library with tmux parser

---

*Created: 2026-02-01*
*Last Updated: 2026-02-01*
