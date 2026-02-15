# par-term File Transfer & Image Utilities Design

**Date**: 2026-02-15
**Status**: Approved

## Overview

Create standalone POSIX sh utility scripts (`pt-dl`, `pt-ul`, `pt-imgcat`) that use iTerm2 OSC 1337 escape sequences for file transfer and inline image display. These work over SSH on any remote host and integrate with par-term's frontend file transfer UI.

## Utilities

### pt-dl — Download files from remote to local

Encodes a file as base64 and sends it via OSC 1337 `File` with `inline=0`. par-term catches the completed transfer and shows a native save dialog.

```
Usage: pt-dl <file> [file2 ...]
       cat data | pt-dl --name output.txt
```

Features:
- Multiple file arguments
- Stdin mode with `--name` flag
- Progress output to stderr for large files
- Validates file exists and is readable before sending

### pt-ul — Upload files from local to remote

Sends OSC 1337 `RequestUpload` escape sequence. par-term shows a native file picker, user selects a file, data arrives for the script to decode and write.

```
Usage: pt-ul [destination_dir]
```

Features:
- Optional destination directory argument
- Decodes base64 response from terminal
- Writes file to current directory or specified destination
- Handles upload cancellation gracefully

### pt-imgcat — Display inline images

Encodes a file as base64 and sends via OSC 1337 `File` with `inline=1`.

```
Usage: pt-imgcat [options] <file|->
       cat image.png | pt-imgcat

Options:
  --width <N>       Width in cells or pixels (e.g., 80, 400px)
  --height <N>      Height in cells or pixels
  --preserve-aspect-ratio  Maintain aspect ratio (default: yes)
  --no-preserve-aspect-ratio  Allow stretching
```

Features:
- Read from file or stdin
- Width/height specification
- Aspect ratio control

## Script Implementation

All scripts are POSIX sh (`#!/bin/sh`) for maximum portability. They use only standard Unix utilities: `base64`, `wc`, `printf`, `cat`, `basename`.

### OSC 1337 Protocol

```
Download: ESC ] 1337 ; File=name=<base64>;size=<bytes>;inline=0 : <base64data> BEL
Upload:   ESC ] 1337 ; RequestUpload=format=tgz BEL
Image:    ESC ] 1337 ; File=name=<base64>;size=<bytes>;inline=1[;width=N][;height=N] : <base64data> BEL
```

- `name` is base64-encoded filename
- `size` is file size in bytes
- `inline=0` triggers save dialog, `inline=1` displays inline
- Data payload is base64-encoded file content
- BEL (0x07) or ST (ESC \) terminates the sequence

### tmux/screen Passthrough

When running inside tmux, escape sequences need wrapping:
```
ESC Ptmux; ESC <sequence> ESC \
```

The scripts detect `$TERM` starting with `screen` or `tmux` and wrap accordingly.

## Installation Integration

### Embedded in binary

`src/shell_integration_installer.rs` changes:
- Embed `pt-dl`, `pt-ul`, `pt-imgcat` via `include_str!()`
- Create `~/.config/par-term/bin/` during installation
- Write utility scripts with executable permissions
- Add `export PATH="$HOME/.config/par-term/bin:$PATH"` to the marker-wrapped RC block
- Uninstall removes the bin directory

### Curl installer

`gh-pages/install-shell-integration.sh` changes:
- Download utility scripts from GitHub raw
- Create bin directory
- Set executable permissions
- PATH addition in RC marker block

### Directory layout

```
~/.config/par-term/
├── bin/
│   ├── pt-dl
│   ├── pt-ul
│   └── pt-imgcat
├── shell_integration.bash
├── shell_integration.zsh
├── shell_integration.fish
└── config.yaml
```

## Files

### New
- `shell_integration/pt-dl` — POSIX sh download utility (~60 lines)
- `shell_integration/pt-ul` — POSIX sh upload utility (~80 lines)
- `shell_integration/pt-imgcat` — POSIX sh inline image utility (~100 lines)

### Modified
- `src/shell_integration_installer.rs` — embed new scripts, create bin dir, add PATH
- `gh-pages/install-shell-integration.sh` — download and install utilities
- `shell_integration/README.md` — document new utilities
- `MATRIX.md` — update shell integration utilities status
- `CHANGELOG.md` — add entry
