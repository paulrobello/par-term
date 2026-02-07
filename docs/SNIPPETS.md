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

Custom actions allow you to execute shell commands, insert text, or simulate key sequences via keyboard shortcuts.

### Action Types

par-term supports three types of custom actions:

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

#### 2. Insert Text
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

#### 3. Key Sequence (Coming Soon)
Simulate keyboard input. This feature is planned for a future release.

### Creating Actions

1. Open Settings (⌘+, / Ctrl+,)
2. Navigate to the **Actions** tab
3. Click **+ Add Action**
4. Fill in the action details:
   - **Title**: A human-readable name (e.g., "Run Tests")
   - **Type**: Select from Shell Command, Insert Text, or Key Sequence
   - **Keybinding** (optional): Keyboard shortcut to trigger the action
     - Click the **🎤 Record** button and press the desired key combination
     - Or type it manually (e.g., `Ctrl+Shift+T`)
     - Conflict warnings appear if the keybinding is already in use (⚠️)
   - **Type-specific fields**: Enter command, text, or key sequence based on type

### Using Actions

Actions are triggered via keyboard shortcuts. You can assign keybindings in two ways:

**Via Action Editor (Recommended):**
1. Open Settings → **Actions** tab
2. Edit or create an action
3. Click the **🎤 Record** button in the Keybinding field
4. Press the desired key combination
5. Save the action

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
    type: "shell_command"       # or "insert_text", "key_sequence"
    command: "npm"
    args: ["test"]
    notify_on_success: true

# Keybindings (auto-generated from snippets and actions)
keybindings:
  - key: "Ctrl+Shift+D"
    action: "snippet:snippet_001"
  - key: "Ctrl+Shift+T"
    action: "snippet:snippet_002"
  - key: "Ctrl+Shift+R"
    action: "action:action_001"
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

### Example 4: Server Management

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

## Limitations

- **Key Sequence Actions**: Not yet implemented (planned for future release)
- **Custom Variables UI**: Custom variables must be configured in YAML (UI planned)
- **Import/Export**: Snippet libraries cannot be imported/exported yet (planned)

## Future Enhancements

- [ ] Key sequence simulation
- [ ] Import/export snippet libraries
- [ ] Snippet sharing between users
- [ ] Search/filter snippets in UI
- [ ] Custom variables editor in UI
- [ ] Snippet templates
