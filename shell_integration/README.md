# par-term Shell Integration

Shell integration provides enhanced terminal features by embedding semantic markers
in your shell's prompt and command output.

## Features

- Working directory tracking in tab titles
- Command exit status indicators
- Prompt navigation between commands
- File transfer utilities (pt-dl, pt-ul, pt-imgcat)

## Installation

### From par-term

Open Settings (F12) -> Integrations -> Install Shell Integration

### Manual (curl)

```bash
curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash
```

## Technical Details

Uses OSC 133 protocol (also used by iTerm2, VSCode, WezTerm).

## File Transfer Utilities

Three POSIX sh scripts are installed to `~/.config/par-term/bin/` and added to your PATH automatically. They use the iTerm2 OSC 1337 protocol and work over SSH on any remote host.

### pt-dl — Download files from remote to local

Encodes files as base64 and sends them via OSC 1337 `File` with `inline=0`. par-term catches the transfer and shows a native save dialog.

```bash
# Download a single file
pt-dl report.pdf

# Download multiple files
pt-dl file1.txt file2.txt file3.log

# Pipe data from stdin
cat /var/log/syslog | pt-dl --name syslog.txt
mysqldump mydb | pt-dl --name backup.sql
```

### pt-ul — Upload files from local to remote

Sends an OSC 1337 `RequestUpload` escape sequence. par-term shows a native file picker, and the selected file is sent to the remote host.

```bash
# Upload to current directory
pt-ul

# Upload to a specific directory
pt-ul /tmp/uploads
```

### pt-imgcat — Display inline images

Encodes images as base64 and sends them via OSC 1337 `File` with `inline=1` for inline display in the terminal.

```bash
# Display an image
pt-imgcat photo.png

# Display with size constraints
pt-imgcat --width 80 --height 24 diagram.png

# Display from stdin
cat screenshot.png | pt-imgcat

# Pixel-based sizing
pt-imgcat --width 400px --height 300px image.jpg

# Allow stretching (disable aspect ratio preservation)
pt-imgcat --no-preserve-aspect-ratio banner.png
```

### SSH Usage

All utilities work transparently over SSH since they only use standard Unix tools (`base64`, `wc`, `printf`, `cat`, `basename`). Install shell integration on the remote host and the utilities are available immediately:

```bash
# On remote host
curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash
source ~/.bashrc

# Now use the utilities
pt-dl /var/log/app.log
pt-imgcat /tmp/chart.png
```

### tmux/screen Support

The utilities automatically detect when running inside tmux or screen and wrap escape sequences in the appropriate passthrough format. No configuration needed.
