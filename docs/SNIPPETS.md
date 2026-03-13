# Snippets & Actions

par-term supports text snippets and custom actions, similar to iTerm2's snippets and actions system. This feature allows you to save frequently-used text blocks, execute shell commands, and automate tasks via keyboard shortcuts.

## Table of Contents

- [Snippets](#snippets)
  - [Creating Snippets](#creating-snippets)
  - [Using Snippets](#using-snippets)
  - [Snippet Variables](#snippet-variables)
  - [Organizing Snippets](#organizing-snippets)
- [Custom Actions](#custom-actions)
  - [Action Types](#action-types)
  - [Creating Actions](#creating-actions)
  - [Using Actions](#using-actions)
  - [Split Pane Actions](#split-pane-actions)
- [Configuration](#configuration)
- [Examples](#examples)

## Snippets

Snippets are saved text blocks that can be quickly inserted into the terminal. They support variable substitution for dynamic content.

### Creating Snippets

1. Open Settings (⌘+, / Ctrl+,)
2. Navigate to the **Snippets** tab
3. Click **+ Add Snippet**
4. Fill in the snippet details:
   - **Title**: A human-readable name (e.g., "Git Commit Message")
   - **Content**: The text to insert (supports variables)
     - For commands that need to run in a specific directory, include the cd command: `cd ~/projects && npm test`
     - The **Folder** field is only for organizing snippets in the UI, not for changing directories
   - **Auto-execute** (optional): Check this box to automatically send Enter after inserting the snippet
     - Useful for commands that should run immediately
     - Equivalent to adding `\n` at the end of the content
   - **Keybinding** (optional): Keyboard shortcut to trigger the snippet
     - Click the **🎤 Record** button and press the desired key combination
     - Or type it manually (e.g., `Ctrl+Shift+D`)
     - Conflict warnings appear if the keybinding is already in use (⚠️)
     - **Enable keybinding** checkbox: Uncheck to disable the keybinding without removing it (useful for temporary disable)
   - **Folder** (optional): Group snippets into folders for organization (e.g., "Git", "Docker", "AWS")
   - **Description** (optional): Notes about what the snippet does

### Using Snippets

**Via Keyboard Shortcut:**
If you've assigned a keybinding to your snippet, simply press the key combination and the snippet will be inserted at the cursor position.

**Via Settings:**
1. Open Settings → Snippets tab
2. Find your snippet in the list
3. Click **Edit** to view/copy the content

### Snippet Variables

Snippets support dynamic variable substitution using the `\(variable)` syntax. When a snippet is inserted, variables are replaced with their current values.

#### Built-in Variables

| Variable | Description | Example |
|----------|-------------|---------|
| `\(date)` | Current date (YYYY-MM-DD) | `2026-02-06` |
| `\(time)` | Current time (HH:MM:SS) | `23:45:30` |
| `\(datetime)` | Current date and time | `2026-02-06 23:45:30` |
| `\(hostname)` | System hostname | `my-computer` |
| `\(user)` | Current username | `alice` |
| `\(path)` | Current working directory | `/home/alice/projects` |
| `\(git_branch)` | Current git branch | `main` |
| `\(git_commit)` | Current git commit hash (short) | `a1b2c3d` |
| `\(uuid)` | Random UUID | `550e8400-e29b-41d4-a716-446655440000` |
| `\(random)` | Random number (0-999999) | `482910` |

#### Session Variables (Live Terminal State)

Snippets can also access live session variables from the badge/automation system using the `\(session.*)` syntax. These variables reflect the current terminal state:

| Variable | Description | Example |
|----------|-------------|---------|
| `\(session.hostname)` | Current hostname (from SSH session or system) | `my-server` |
| `\(session.username)` | Current username | `alice` |
| `\(session.path)` | Current working directory | `/home/alice/projects` |
| `\(session.job)` | Foreground job name (if any) | `vim` |
| `\(session.last_command)` | Last executed command | `git status` |
| `\(session.profile_name)` | Active profile name | `Development` |
| `\(session.tty)` | TTY device name | `/dev/pts/0` |
| `\(session.columns)` | Terminal column count | `120` |
| `\(session.rows)` | Terminal row count | `40` |
| `\(session.bell_count)` | Number of bells received | `5` |
| `\(session.selection)` | Currently selected text | `selected text` |
| `\(session.tmux_pane_title)` | tmux pane title (in tmux mode) | `vim` |
| `\(session.exit_code)` | Last command exit code | `0` |
| `\(session.current_command)` | Currently running command name | `npm` |

**Variable Priority:**
1. Custom snippet variables (highest)
2. Session variables (`session.*`)
3. Built-in variables (lowest)

This means you can override built-in or session variables by defining a custom variable with the same name.

#### Example Snippets

**Date Stamp:**
```yaml
Title: Date Stamp
Content: echo "Report generated on \(date)"
```
Inserts: `echo "Report generated on 2026-02-06"`

**Git Commit Template:**
```yaml
Title: Git Commit
Content: git commit -m "feat(\(user)): \(datetime)"
```
Inserts: `git commit -m "feat(alice): 2026-02-06 23:45:30"`

**Project Header:**
```yaml
Title: Project Header
Content: echo "Working on \(user)@\(hostname) in \(path)"
```
Inserts: `echo "Working on alice@my-computer in /home/alice/projects"`

**Session State Snippet:**
```yaml
Title: Save Context
Content: echo "Working on \(session.job) in \(session.path) on \(session.hostname)"
Auto-execute: true
```
When triggered while editing a file in vim: `echo "Working on vim in /home/alice/projects on my-server"`

**Command with Auto-execute:**
```yaml
Title: Run Tests
Content: cd ~/projects/myapp && npm test
Auto-execute: true
Keybinding: Ctrl+Shift+T
```
When triggered, changes directory and runs tests immediately (sends Enter automatically)

### Organizing Snippets

Snippets can be organized into folders for better management:

1. When creating/editing a snippet, enter a folder name (e.g., "Git", "Docker", "AWS")
2. Snippets are grouped by folder in the UI
3. Folders help keep related snippets together

## Custom Actions

Custom actions allow you to execute shell commands, open a new tab with an optional command, insert text, simulate key sequences, or split a pane and run a command in it — all via keyboard shortcuts.

Custom actions can also use a two-stroke prefix trigger:

- Set a global **Prefix key** in Settings -> **Snippets & Actions** -> **Custom Actions** (for example `Ctrl+B`)
- Give an action a single-character **Prefix char** (for example `g` or `%`)
- Press the prefix key, release it, then press the action's prefix char to run it
- A prefix toast stays visible while prefix mode is armed; press `Esc` to cancel it

This works alongside the existing per-action keybinding, so an action can have either trigger style or both.

### Action Types

par-term supports five types of custom actions:

#### 1. Shell Command
Execute a shell command with optional arguments.

**Use cases:**
- Run frequently-used commands
- Execute build scripts
- Start services
- Run tests

**Example:**
```yaml
Title: Run Tests
Command: npm
Arguments: test
Notify on Success: true
```

#### 2. New Tab
Open a new tab and optionally run a command in that tab's shell.

**Use cases:**
- Open a clean shell in a new tab
- Start a watcher, server, or TUI in a separate tab
- Launch a project-specific command without disturbing the current tab

**Behavior:**
- If **Command** is empty, the action just opens a normal new tab.
- If **Command** is set, par-term opens a new tab and sends that command to the new tab's shell.

**Example:**
```yaml
Title: Open lazygit tab
Type: New Tab
Command: lazygit
```

#### 3. Insert Text
Insert text into the terminal (similar to snippets, but without the editing UI).

**Use cases:**
- Quick text insertion
- Templates with variables
- Frequently-used commands

**Example:**
```yaml
Title: SSH to Server
Text: ssh user@\(hostname).example.com
```

#### 4. Key Sequence
Simulate keyboard input by sending terminal byte sequences to the PTY.

**Supported sequences:**
- Ctrl combos (e.g., `Ctrl+C`, `Ctrl+D`)
- Arrow keys, Home, End, PageUp, PageDown
- Function keys (F1-F12)
- Enter, Tab, Escape, Backspace

**Use cases:**
- Navigate TUI applications
- Send control characters
- Automate keyboard-driven workflows

**Example:**
```yaml
Title: Exit Vim
Type: Key Sequence
Sequence: Escape :wq Enter
```

#### 5. Split Pane
Split the active pane horizontally (new pane below) or vertically (new pane to the right) and optionally send a command to the new pane.

**Fields:**
| Field | Description | Default |
|-------|-------------|---------|
| `direction` | `horizontal` (below) or `vertical` (right) | `horizontal` |
| `command` | Optional command for the new pane (see `command_is_direct`) | _(none)_ |
| `command_is_direct` | When `true`, `command` is the pane's initial process — the pane closes when it exits. When `false`, `command` is sent as text to the shell. | `false` |
| `focus_new_pane` | Move focus to the new pane after splitting | `true` |
| `delay_ms` | Milliseconds to wait before sending shell-mode text (ignored when `command_is_direct: true`) | `200` |
| `split_percent` | Percentage of the current pane the **existing** pane retains (10–90). The new pane receives the remainder. | `66` |

**Command modes:**

- **Shell mode** (`command_is_direct: false`, default): the command string is typed into the shell with a trailing Enter, just like you typed it yourself. The shell remains running after the command finishes.
- **Direct mode** (`command_is_direct: true`): the pane's PTY runs the command directly as its process (no shell wrapper). **The pane closes automatically when the command exits.** Useful for tools like `htop`, `vim`, or `watch` where you want the pane to disappear when you quit the tool.

**Use cases:**
- Open a monitoring tool (e.g., `htop`, `watch`, log tail) alongside your current work
- Start a server or dev watcher in a companion pane
- Any workflow where you want to run something in a new pane without leaving your current one

**Example — direct mode (pane closes when htop exits):**
```yaml
Title: Split and run htop
Type: Split Pane
Direction: Vertical (right)
Command: htop
Run as pane command: ✓ (checked)
Focus new pane: true
Split percent: 66%
```

**Example — shell mode (shell stays after command):**
```yaml
Title: Split and tail log
Type: Split Pane
Direction: Horizontal (below)
Command: tail -f /var/log/system.log
Run as pane command: (unchecked)
Focus new pane: false
Command delay: 200 ms
Split percent: 33%
```

### Creating Actions

1. Open Settings (⌘+, / Ctrl+,)
2. Navigate to the **Snippets & Actions** tab
3. Scroll to the **Custom Actions** section and click **+ Add Action**
4. Fill in the action details:
   - **Title**: A human-readable name (e.g., "Run Tests")
   - **Type**: Select from Shell Command, New Tab, Insert Text, Key Sequence, or Split Pane
   - **Prefix char** (optional): Single character used after the global custom action prefix key
   - **Keybinding** (optional): Keyboard shortcut to trigger the action
     - Click the **🎤 Record** button and press the desired key combination
     - Or type it manually (e.g., `Ctrl+Shift+T`)
     - Conflict warnings appear if the keybinding is already in use (⚠️)
   - **Type-specific fields**: Enter command, text, or key sequence based on type

### Using Actions

Actions are triggered via keyboard shortcuts. You can assign keybindings in two ways:

**Via Action Editor (Recommended):**
1. Open Settings → **Snippets & Actions** tab
2. Scroll to the **Custom Actions** section and edit or create an action
3. Click the **🎤 Record** button in the Keybinding field
4. Press the desired key combination
5. Save the action

**Via Prefix Key:**
1. Open Settings -> **Snippets & Actions** -> **Custom Actions**
2. Set the section-level **Prefix key** once (for example `Ctrl+B`)
   Use the **🎤 Record** button if you want par-term to capture the combo for you
3. Give one or more actions a **Prefix char**
4. Press the prefix key, release it, then press the action character to execute it
5. If you change your mind, press `Esc` to cancel prefix mode before the follow-up key

**Via Keybindings List:**
1. Open Settings → **Input** tab
2. Add a new keybinding:
   - **Key**: e.g., `Ctrl+Shift+T`
   - **Action**: `action:<action_id>` (e.g., `action:run_tests`)

**Keybinding Recording:**
- The **🎤 Record** button captures your exact key combination
- A **🔴 Recording...** indicator appears while recording
- **⚠️ Conflict warnings** show if the keybinding is already used elsewhere
- When editing, the current keybinding is excluded from conflict detection

## Split Pane Actions

Split Pane actions open a new pane next to the active one and optionally run a command in it. There are two command modes:

**Direct mode** (`command_is_direct: true`) — the pane runs the command as its own process. The pane closes automatically when the command exits. Best for interactive tools like `htop`, `vim`, or `watch`.

```yaml
actions:
  - id: "split-htop"
    title: "Split and run htop"
    type: split_pane
    direction: vertical
    command: htop
    command_is_direct: true     # pane IS htop; closes when htop quits
    focus_new_pane: true
    split_percent: 66           # existing pane keeps 66%, htop pane gets 34%
    keybinding: "Ctrl+Shift+H"
```

**Shell mode** (`command_is_direct: false`, default) — the command is sent as text to the shell with a trailing newline. The shell remains running after the command finishes.

```yaml
actions:
  - id: "split-tail"
    title: "Split and tail log"
    type: split_pane
    direction: horizontal
    command: "tail -f /var/log/system.log"
    command_is_direct: false    # default — typed into the shell
    focus_new_pane: false
    delay_ms: 200               # wait for shell to start (shell mode only)
    split_percent: 66           # existing pane keeps 66%, log pane gets 34%
    keybinding: "Ctrl+Shift+L"
```

Omit `command` entirely to split without running anything. Omit `split_percent` to use the default (66%).

> **Tip:** For `command_is_direct: false`, increase `delay_ms` (e.g., to 500) for slow-starting shells such as remote SSH sessions.

## Configuration

Snippets and actions are stored in your `config.yaml` file. Keybindings are automatically saved to the keybindings list when you set them in the snippet or action editor.

```yaml
# Text snippets
snippets:
  - id: "snippet_001"
    title: "Date Stamp"
    content: "echo 'Report: \(date)'"
    keybinding: "Ctrl+Shift+D"  # optional, set via Record button
    keybinding_enabled: true    # enable/disable keybinding (default: true)
    auto_execute: false         # send Enter after inserting (default: false)
    folder: "Common"            # optional
    enabled: true
    description: "Insert current date"
    variables: {}                # custom variables

  - id: "snippet_002"
    title: "Run Tests"
    content: "cd ~/projects/myapp && npm test"
    keybinding: "Ctrl+Shift+T"
    auto_execute: true          # Automatically runs the command
    folder: "Development"

# Custom actions
actions:
  - id: "action_001"
    title: "Run Tests"
    type: shell_command         # shell_command | insert_text | key_sequence | split_pane
    command: "npm"
    args: ["test"]
    notify_on_success: true

  - id: "action_002"
    title: "Split and tail log"
    type: split_pane
    direction: horizontal       # new pane below
    command: "tail -f /var/log/system.log"
    command_is_direct: false    # send as shell text (default)
    focus_new_pane: true
    delay_ms: 200               # ms before sending text (shell mode only)
    split_percent: 66           # existing pane keeps 66% (default)
    keybinding: "Ctrl+Shift+L"

  - id: "action_003"
    title: "Split and run htop"
    type: split_pane
    direction: vertical         # new pane to the right
    command: htop
    command_is_direct: true     # pane IS htop; closes when htop exits
    focus_new_pane: true
    split_percent: 66           # existing pane keeps 66% (default)
    keybinding: "Ctrl+Shift+H"

# Keybindings (auto-generated from snippets and actions)
keybindings:
  - key: "Ctrl+Shift+D"
    action: "snippet:snippet_001"
  - key: "Ctrl+Shift+T"
    action: "snippet:snippet_002"
  - key: "Ctrl+Shift+R"
    action: "action:action_001"
  - key: "Ctrl+Shift+L"
    action: "action:action_002"
```

## Examples

### Example 1: Git Workflow Snippets

Create a folder called "Git" with these snippets:

**Git Status:**
```yaml
Title: Git Status
Content: git status
Keybinding: Ctrl+Shift+G
Folder: Git
```

**Git Commit:**
```yaml
Title: Git Commit
Content: git commit -m "chore: updates on \(date)"
Keybinding: Ctrl+Shift+C
Folder: Git
```

**Git Push:**
```yaml
Title: Git Push
Content: git push
Keybinding: Ctrl+Shift+P
Folder: Git
```

### Example 2: Docker Actions

Create actions for common Docker operations:

**Build Docker Image:**
```yaml
Type: Shell Command
Title: Docker Build
Command: docker
Arguments: build -t myapp .
```

**List Containers:**
```yaml
Type: Shell Command
Title: Docker PS
Command: docker
Arguments: ps
```

### Example 3: Project Template

Create a snippet for new project initialization:

```yaml
Title: New Project Template
Content: |
  #!/bin/bash
  # Project: \(user)
  # Created: \(datetime)

  mkdir -p src tests docs
  echo "# \(path)" > README.md
  git init
  npm init -y
```

### Example 4: Split Pane Workflows

Open a monitoring or companion pane with a single key press:

**Tail a log in a new pane below (shell mode — shell stays when `tail` exits):**
```yaml
- id: tail-log
  title: "Tail system log"
  type: split_pane
  direction: horizontal
  command: "tail -f /var/log/system.log"
  command_is_direct: false    # default
  split_percent: 66           # existing pane keeps 2/3
  keybinding: "Ctrl+Shift+L"
```

**Open htop to the right — pane closes when you quit htop (direct mode):**
```yaml
- id: split-htop
  title: "Open htop (right)"
  type: split_pane
  direction: vertical
  command: htop
  command_is_direct: true     # pane closes on exit
  focus_new_pane: true
  split_percent: 66           # existing pane keeps 2/3
  keybinding: "Ctrl+Shift+H"
```

**50/50 split — equal panes:**
```yaml
- id: split-equal
  title: "Equal split (right)"
  type: split_pane
  direction: vertical
  split_percent: 50
  keybinding: "Ctrl+Shift+E"
```

**Just split — no command (uses default 66%):**
```yaml
- id: split-blank
  title: "New blank pane (right)"
  type: split_pane
  direction: vertical
  keybinding: "Ctrl+Shift+N"
```

### Example 5: Server Management

Snippets for SSH connections:

```yaml
Title: SSH to Production
Content: ssh admin@production.example.com
Folder: SSH
```

```yaml
Title: SSH to Staging
Content: ssh admin@staging.example.com
Folder: SSH
```

## Tips and Best Practices

1. **Use Descriptive Titles**: Make snippet titles clear and specific
2. **Organize with Folders**: Group related snippets (Git, Docker, AWS, etc.)
3. **Leverage Variables**: Use built-in variables for dynamic content
4. **Test Snippets**: Try snippets in a safe environment before relying on them
5. **Backup Your Config**: Snippets and actions are part of your config.yaml
6. **Keybinding Conflicts**: Use the **🎤 Record** button to detect conflicts automatically
   - **⚠️ Yellow warnings** indicate existing keybindings
   - Conflicts show what the keybinding is currently assigned to
   - When editing, your current keybinding is excluded from conflict detection
7. **Use Record Button**: The Record button ensures accurate keybinding capture
   - Captures modifier keys correctly (Ctrl, Shift, Alt, Super/Windows/Command)
   - Avoids typos from manual entry
   - Shows real-time feedback during recording (🔴)
8. **Disable Keybindings Temporarily**: Use the "Enable keybinding" checkbox to temporarily disable a keybinding without removing it
   - Useful when you need to free up a keybinding for another use
   - The keybinding configuration is preserved but won't trigger
   - Re-enable the checkbox to restore the keybinding functionality
9. **Use Auto-execute for Commands**: Check "Auto-execute" for snippets that should run immediately
   - Perfect for frequently-run commands (tests, builds, git operations)
   - Automatically sends Enter after inserting the snippet
   - Equivalent to adding `\n` at the end of the content
10. **Folders Don't Change Directories**: The folder field is only for organizing snippets in the UI
    - To run commands in a specific directory, include `cd /path/to/dir &&` in the snippet content
    - Example: `cd ~/projects/myapp && npm test`
11. **Split Pane vs. Trigger SplitPane**: Custom action Split Pane is triggered manually via keybinding; [trigger SplitPane](AUTOMATION.md#split-pane) fires automatically when a regex pattern matches terminal output. Use custom actions for on-demand splits and triggers for automated splits.
12. **Increase delay_ms for slow shells**: The default 200 ms delay before sending a command to a new pane is enough for local shells. For SSH sessions or slow-starting environments, increase it to 500–1000 ms.

## Import and Export

Snippets can be exported to and imported from YAML files for backup or sharing.

### Exporting

1. Open Settings > Snippets tab
2. Click **Export** to save all snippets to a YAML file
3. Choose a save location

### Importing

1. Open Settings > Snippets tab
2. Click **Import** and select a YAML file
3. par-term automatically handles conflicts:
   - Snippets with duplicate IDs are skipped (not imported)
   - Keybindings that conflict with existing ones are cleared from the imported snippet
4. Imported snippets are added to your existing collection

## Custom Variables

Each snippet can define custom variables that override built-in and session variables.

### Using the Variables Editor

1. Edit a snippet in Settings > Snippets tab
2. Expand the **Custom Variables** section (collapsible)
3. Add variable name/value pairs in the grid
4. Use the `+` button to add rows and the delete button to remove them
5. Reference variables in snippet content with `\(variable_name)` syntax

### Variable Priority

1. Custom snippet variables (highest)
2. Session variables (`session.*`)
3. Built-in variables (lowest)

## Workflow Actions

Workflow actions let you compose, branch, and repeat existing actions from within par-term's config. Three new action types enable multi-step automation without leaving the terminal.

### Sequence

Runs a list of actions in order. Each step can have a delay and an on-failure behavior.

```yaml
actions:
  - type: sequence
    id: build-and-test
    title: Build and Test
    keybinding: "Ctrl+Shift+B"
    steps:
      - action_id: run-build        # runs the "run-build" action first
        delay_ms: 0
        on_failure: abort           # stop and show error toast
      - action_id: run-tests
        delay_ms: 500               # wait 500ms before running tests
        on_failure: continue        # report but keep going
      - action_id: notify-done
        delay_ms: 0
        on_failure: stop            # halt silently on failure
```

**Step failure**: A step "fails" when:
- It is a `ShellCommand` with `capture_output: true` and exits with a non-zero code
- It is a `Condition` whose check evaluates to false
- Steps of all other types (InsertText, KeySequence, NewTab, SplitPane) always succeed

**`on_failure` values**:
| Value | Effect |
|-------|--------|
| `abort` (default) | Halt sequence and show an error toast |
| `stop` | Halt sequence silently |
| `continue` | Ignore failure and proceed to the next step |

**Sequence composition**: Sequences can reference other Sequence actions. Circular references are detected at execution time and show an error toast.

---

### Condition

Evaluates a check and branches to a different action based on the result.

**Standalone use** (direct keybinding): executes `on_true_id` or `on_false_id` depending on the check result.

**Inside a Sequence step**: the check result determines success/failure for the step's `on_failure` behavior. `on_true_id`/`on_false_id` are ignored in this context.

```yaml
actions:
  # Check exit code of the last captured ShellCommand
  - type: condition
    id: check-build-ok
    title: Check Build Result
    check:
      kind: exit_code
      value: 0
    on_true_id: deploy-action       # run if exit code == 0
    on_false_id: notify-failure     # run if exit code != 0

  # Check whether output contains a pattern
  - type: condition
    id: check-tests-pass
    title: Check Test Output
    check:
      kind: output_contains
      pattern: "test result: ok"
      case_sensitive: false
    on_true_id: merge-action

  # Check an environment variable
  - type: condition
    id: check-ci-env
    title: Check CI Environment
    keybinding: "Ctrl+Shift+C"
    check:
      kind: env_var
      name: CI
      # value: omit to check existence only; set a value to check equality
    on_true_id: ci-deploy
    on_false_id: local-deploy

  # Match the current shell directory with a glob
  - type: condition
    id: check-project-dir
    title: Check Project Directory
    check:
      kind: dir_matches
      pattern: "/home/user/projects/*"
    on_true_id: project-action

  # Match the current git branch with a glob
  - type: condition
    id: check-main-branch
    title: Check Main Branch
    keybinding: "Ctrl+Shift+M"
    check:
      kind: git_branch
      pattern: "main"
    on_true_id: deploy-to-prod
    on_false_id: deploy-to-staging
```

**Check types**:
| Kind | Fields | Description |
|------|--------|-------------|
| `exit_code` | `value: i32` | Compares last captured shell command exit code |
| `output_contains` | `pattern: String`, `case_sensitive: bool` | Searches last captured output |
| `env_var` | `name: String`, `value?: String` | Checks env var existence or equality |
| `dir_matches` | `pattern: String` (glob) | Matches shell's current working directory |
| `git_branch` | `pattern: String` (glob) | Matches current git branch name |

**Note**: `exit_code` and `output_contains` require a preceding `ShellCommand` with `capture_output: true`.

---

### Repeat

Runs a single action up to N times with an optional delay between repetitions.

```yaml
actions:
  # Retry a deploy up to 3 times, stop when one succeeds
  - type: repeat
    id: retry-deploy
    title: Retry Deploy (up to 3×)
    keybinding: "Ctrl+Shift+D"
    action_id: deploy-action        # any action type, including Sequence
    count: 3
    delay_ms: 2000                  # wait 2s between retries
    stop_on_success: true           # stop early if action succeeds
    stop_on_failure: false          # keep trying even on failure

  # Run a check 5 times with no delay
  - type: repeat
    id: run-health-checks
    title: Run 5 Health Checks
    action_id: health-check-action
    count: 5
```

**Fields**:
| Field | Default | Description |
|-------|---------|-------------|
| `action_id` | required | ID of the action to repeat (any type) |
| `count` | required | Maximum repetitions (1–100) |
| `delay_ms` | `0` | Milliseconds to wait between repetitions |
| `stop_on_success` | `false` | Stop early if the action succeeds |
| `stop_on_failure` | `false` | Stop early if the action fails |

---

### Capturing Shell Output for Conditions

Add `capture_output: true` to a `ShellCommand` action to make its stdout/stderr and exit code available to subsequent `Condition` checks:

```yaml
actions:
  - type: shell_command
    id: run-build
    title: Run Build
    command: cargo
    args: ["build", "--release"]
    capture_output: true            # capture stdout+stderr (capped at 64 KB)

  - type: condition
    id: after-build
    title: After Build Branch
    check:
      kind: exit_code
      value: 0
    on_true_id: run-deploy
    on_false_id: show-build-errors

  - type: sequence
    id: build-then-deploy
    title: Build Then Deploy
    keybinding: "Ctrl+Shift+R"
    steps:
      - action_id: run-build
        on_failure: abort
      - action_id: after-build
        on_failure: stop
```

## Related Documentation

- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - Keybinding configuration and management
- [Automation](AUTOMATION.md) - Triggers, coprocesses, and shell integration
- [Configuration Reference](CONFIG_REFERENCE.md) - Complete configuration options
- [Status Bar](STATUS_BAR.md) - Session variables and badge system

## Future Enhancements

- [ ] Snippet sharing between users
- [ ] Search/filter snippets in UI
- [ ] Snippet templates
