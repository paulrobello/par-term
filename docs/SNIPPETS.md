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
   - **Keybinding** (optional): Keyboard shortcut to trigger the snippet
   - **Folder** (optional): Group snippets into folders
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
4. Select the action type:
   - **Shell Command**: Enter command and arguments
   - **Insert Text**: Enter the text to insert (supports variables)
   - **Key Sequence**: Coming soon

### Using Actions

Actions are triggered via keyboard shortcuts. To assign a keybinding to an action:

1. Open Settings → **Input** tab
2. Add a new keybinding:
   - **Key**: e.g., `Ctrl+Shift+T`
   - **Action**: `action:<action_id>` (e.g., `action:run_tests`)

**Note:** Currently, action keybindings must be configured manually in the keybindings list. A future update will add keybinding fields directly to the action editor.

## Configuration

Snippets and actions are stored in your `config.yaml` file:

```yaml
# Text snippets
snippets:
  - id: "snippet_001"
    title: "Date Stamp"
    content: "echo 'Report: \(date)'"
    keybinding: "Ctrl+Shift+D"  # optional
    folder: "Common"            # optional
    enabled: true
    description: "Insert current date"
    variables: {}                # custom variables

# Custom actions
actions:
  - id: "action_001"
    title: "Run Tests"
    type: "shell_command"       # or "insert_text", "key_sequence"
    command: "npm"
    args: ["test"]
    notify_on_success: true
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
6. **Keybinding Conflicts**: Avoid using keybindings that conflict with system or app shortcuts

## Limitations

- **Key Sequence Actions**: Not yet implemented (planned for future release)
- **Action Keybindings in UI**: Currently require manual configuration in keybindings list
- **Custom Variables UI**: Custom variables must be configured in YAML (UI planned)

## Future Enhancements

- [ ] Key sequence simulation
- [ ] Import/export snippet libraries
- [ ] Snippet sharing between users
- [ ] Search/filter snippets in UI
- [ ] Custom variables editor in UI
- [ ] Action keybinding field in editor
- [ ] Snippet templates
