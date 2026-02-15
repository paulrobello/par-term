# File Transfer Utilities Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Create POSIX sh utility scripts (pt-dl, pt-ul, pt-imgcat) that use iTerm2 OSC 1337 escape sequences for file transfer and inline image display, and integrate them into both the embedded and curl-based shell integration installers.

**Architecture:** Three standalone POSIX sh scripts in `shell_integration/` are embedded via `include_str!()` in the Rust installer. At install time, scripts are written to `~/.config/par-term/bin/` with executable permissions, and `PATH` is updated in the RC file marker block. The curl installer mirrors this by downloading the scripts from GitHub raw.

**Tech Stack:** POSIX sh, base64, wc, printf, cat, basename (standard Unix utilities)

---

### Task 1: Create pt-dl download utility script

**Files:**
- Create: `shell_integration/pt-dl`

**Step 1: Write pt-dl script**

Create `shell_integration/pt-dl` as a POSIX sh script (~60 lines) that:
- Accepts one or more file arguments: `pt-dl <file> [file2 ...]`
- Accepts stdin with `--name` flag: `cat data | pt-dl --name output.txt`
- Validates file exists and is readable before sending
- Detects tmux/screen (`$TERM` starts with `screen` or `tmux`) and wraps escape sequences
- Base64-encodes filename for the `name=` parameter
- Gets file size in bytes with `wc -c`
- Sends OSC 1337 protocol: `ESC ] 1337 ; File=name=<b64name>;size=<bytes>;inline=0 : <b64data> BEL`
- Shows progress to stderr for files over 100KB
- Loops over multiple file arguments

Protocol details:
```
\033]1337;File=name=<base64-encoded-filename>;size=<file-size-bytes>;inline=0:<base64-encoded-data>\007
```

tmux passthrough wrapping:
```
\033Ptmux;\033\033]1337;File=name=...;inline=0:...\007\033\\
```

**Step 2: Verify script is valid sh**

Run: `sh -n shell_integration/pt-dl`
Expected: No syntax errors

**Step 3: Commit**

```bash
git add shell_integration/pt-dl
git commit -m "feat(shell-integration): add pt-dl download utility script"
```

---

### Task 2: Create pt-ul upload utility script

**Files:**
- Create: `shell_integration/pt-ul`

**Step 1: Write pt-ul script**

Create `shell_integration/pt-ul` as a POSIX sh script (~80 lines) that:
- Accepts optional destination directory: `pt-ul [destination_dir]`
- Sends OSC 1337 `RequestUpload=format=tgz` escape sequence
- Detects tmux/screen and wraps escape sequences
- Waits for base64-encoded response from terminal (reads until `\007` or timeout)
- Decodes the response and extracts filename + data
- Writes file to current directory or specified destination
- Handles upload cancellation gracefully (empty/error response)
- Prints status messages to stderr

Protocol:
```
Send: \033]1337;RequestUpload=format=tgz\007
Receive: base64-encoded file data terminated by newline
```

**Step 2: Verify script is valid sh**

Run: `sh -n shell_integration/pt-ul`
Expected: No syntax errors

**Step 3: Commit**

```bash
git add shell_integration/pt-ul
git commit -m "feat(shell-integration): add pt-ul upload utility script"
```

---

### Task 3: Create pt-imgcat inline image utility script

**Files:**
- Create: `shell_integration/pt-imgcat`

**Step 1: Write pt-imgcat script**

Create `shell_integration/pt-imgcat` as a POSIX sh script (~100 lines) that:
- Reads from file argument or stdin: `pt-imgcat [options] <file|->` or `cat image.png | pt-imgcat`
- Supports options:
  - `--width <N>` — Width in cells or pixels (e.g., `80`, `400px`)
  - `--height <N>` — Height in cells or pixels
  - `--preserve-aspect-ratio` — Maintain aspect ratio (default: yes)
  - `--no-preserve-aspect-ratio` — Allow stretching
- Parses options with a while/case loop
- Detects tmux/screen and wraps escape sequences
- Base64-encodes filename for the `name=` parameter
- Gets file size with `wc -c` (or estimates from stdin)
- Sends OSC 1337 protocol: `ESC ] 1337 ; File=name=<b64name>;size=<bytes>;inline=1[;width=N][;height=N][;preserveAspectRatio=0|1] : <b64data> BEL`
- Falls back to stdin if no file argument and stdin is not a terminal

Protocol:
```
\033]1337;File=name=<b64>;size=<bytes>;inline=1;width=<W>;height=<H>;preserveAspectRatio=<0|1>:<b64data>\007
```

**Step 2: Verify script is valid sh**

Run: `sh -n shell_integration/pt-imgcat`
Expected: No syntax errors

**Step 3: Commit**

```bash
git add shell_integration/pt-imgcat
git commit -m "feat(shell-integration): add pt-imgcat inline image utility script"
```

---

### Task 4: Update shell_integration_installer.rs to embed and install utilities

**Files:**
- Modify: `src/shell_integration_installer.rs`

**Step 1: Add include_str constants for utility scripts**

After line 18 (the existing `FISH_SCRIPT` constant), add:

```rust
// Embedded file transfer utility scripts
const PT_DL_SCRIPT: &str = include_str!("../shell_integration/pt-dl");
const PT_UL_SCRIPT: &str = include_str!("../shell_integration/pt-ul");
const PT_IMGCAT_SCRIPT: &str = include_str!("../shell_integration/pt-imgcat");
```

**Step 2: Create a helper to install utility scripts to bin directory**

Add a new function `install_utilities()` that:
- Creates `~/.config/par-term/bin/` directory (uses `Config::shell_integration_dir().join("bin")`)
- Writes `pt-dl`, `pt-ul`, `pt-imgcat` to the bin directory
- Sets executable permissions on Unix (`std::os::unix::fs::PermissionsExt`)
- Returns the bin directory path for PATH addition

```rust
/// Install file transfer utility scripts to the bin directory
fn install_utilities() -> Result<PathBuf, String> {
    let bin_dir = Config::shell_integration_dir().join("bin");
    fs::create_dir_all(&bin_dir)
        .map_err(|e| format!("Failed to create bin directory {:?}: {}", bin_dir, e))?;

    let utilities: &[(&str, &str)] = &[
        ("pt-dl", PT_DL_SCRIPT),
        ("pt-ul", PT_UL_SCRIPT),
        ("pt-imgcat", PT_IMGCAT_SCRIPT),
    ];

    for (name, content) in utilities {
        let path = bin_dir.join(name);
        fs::write(&path, content)
            .map_err(|e| format!("Failed to write {:?}: {}", path, e))?;

        // Set executable permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let perms = std::fs::Permissions::from_mode(0o755);
            fs::set_permissions(&path, perms)
                .map_err(|e| format!("Failed to set permissions on {:?}: {}", path, e))?;
        }
    }

    Ok(bin_dir)
}
```

**Step 3: Call install_utilities() from install() function**

In the `install()` function, after writing the shell integration script (after line 81), call `install_utilities()`:

```rust
// Install file transfer utilities to bin directory
let bin_dir = install_utilities()?;
```

**Step 4: Update generate_source_block() to include PATH export**

In `generate_source_block()`, add a PATH export line inside the marker block. The bin_dir path is `Config::shell_integration_dir().join("bin")`.

For bash/zsh:
```
# >>> par-term shell integration >>>
export PATH="$HOME/.config/par-term/bin:$PATH"
if [ -f "/path/to/shell_integration.bash" ]; then
    source "/path/to/shell_integration.bash"
fi
# <<< par-term shell integration <<<
```

For fish:
```
# >>> par-term shell integration >>>
set -gx PATH "$HOME/.config/par-term/bin" $PATH
if test -f "/path/to/shell_integration.fish"
    source "/path/to/shell_integration.fish"
end
# <<< par-term shell integration <<<
```

**Step 5: Update uninstall() to remove bin directory**

In the `uninstall()` function, after removing script files (after line 129), add:

```rust
// Remove bin directory with utilities
let bin_dir = Config::shell_integration_dir().join("bin");
if bin_dir.exists() {
    let _ = fs::remove_dir_all(&bin_dir);
}
```

**Step 6: Update is_installed() to also check bin directory exists**

Optionally check that the bin directory and at least one utility exist as part of the installation check.

**Step 7: Add tests for utility installation**

Add test functions:
- `test_install_utilities()` — verifies utilities are written to a temp dir
- `test_generate_source_block_includes_path()` — verifies PATH export in marker block

**Step 8: Run tests**

Run: `cargo test shell_integration_installer`
Expected: All tests pass

**Step 9: Run lint and format**

Run: `cargo fmt && cargo clippy -- -D warnings`
Expected: No errors

**Step 10: Commit**

```bash
git add src/shell_integration_installer.rs
git commit -m "feat(shell-integration): embed utility scripts in installer with PATH setup"
```

---

### Task 5: Update curl installer to download and install utilities

**Files:**
- Modify: `gh-pages/install-shell-integration.sh`

**Step 1: Add utility download and install logic**

After the shell integration script download (around line 196), add:

```sh
# Download file transfer utilities
printf "Downloading file transfer utilities...\n"
BIN_DIR="$CONFIG_DIR/bin"
mkdir -p "$BIN_DIR"

for util in pt-dl pt-ul pt-imgcat; do
    UTIL_URL="$BASE_URL/$util"
    download_file "$UTIL_URL" "$BIN_DIR/$util"
    chmod +x "$BIN_DIR/$util"
    printf "${GREEN}Downloaded:${NC} %s\n" "$BIN_DIR/$util"
done
echo ""
```

**Step 2: Update add_integration_block() to include PATH**

Modify the `get_source_line()` function (or `add_integration_block()`) to also add the PATH export. For bash/zsh:

```sh
get_source_line() {
    shell="$1"
    script_path="$2"
    bin_dir="$3"

    case "$shell" in
        fish)
            printf 'set -gx PATH "%s" $PATH\nsource "%s"' "$bin_dir" "$script_path"
            ;;
        *)
            printf 'export PATH="%s:$PATH"\n[ -f "%s" ] && source "%s"' "$bin_dir" "$script_path" "$script_path"
            ;;
    esac
}
```

Update the caller to pass `$BIN_DIR` as the third argument.

**Step 3: Update the completion message**

Add file transfer utilities to the "Shell integration provides:" list:

```
echo "  - File transfer utilities (pt-dl, pt-ul, pt-imgcat)"
```

**Step 4: Commit**

```bash
git add gh-pages/install-shell-integration.sh
git commit -m "feat(shell-integration): add utility download to curl installer"
```

---

### Task 6: Update shell_integration/README.md

**Files:**
- Modify: `shell_integration/README.md`

**Step 1: Add utility documentation**

Add a new "File Transfer Utilities" section documenting:
- `pt-dl` — usage, examples, multiple files, stdin mode
- `pt-ul` — usage, destination directory
- `pt-imgcat` — usage, options (width, height, aspect ratio), stdin mode
- Note about SSH usage: scripts work on remote hosts since they only use standard Unix utilities
- Note about tmux/screen passthrough support

**Step 2: Commit**

```bash
git add shell_integration/README.md
git commit -m "docs(shell-integration): document file transfer utilities"
```

---

### Task 7: Update MATRIX.md and CHANGELOG.md

**Files:**
- Modify: `MATRIX.md`
- Modify: `CHANGELOG.md`

**Step 1: Update MATRIX.md**

Find the shell integration utilities row(s) and update status to reflect the new utilities.

**Step 2: Update CHANGELOG.md**

Add entry under Unreleased:
```
- **Shell Integration Utilities**: Added `pt-dl`, `pt-ul`, and `pt-imgcat` POSIX sh utilities for file download, upload, and inline image display over SSH via iTerm2 OSC 1337 protocol
```

**Step 3: Commit**

```bash
git add MATRIX.md CHANGELOG.md
git commit -m "docs(matrix,changelog): add file transfer utility entries"
```

---

### Task 8: Final verification

**Step 1: Run full check suite**

Run: `make checkall`
Expected: All format, lint, and test checks pass

**Step 2: Verify script syntax**

Run:
```bash
sh -n shell_integration/pt-dl
sh -n shell_integration/pt-ul
sh -n shell_integration/pt-imgcat
```
Expected: No syntax errors

**Step 3: Push and update PR**

Push the branch and update PR #155 with the new utility changes.
