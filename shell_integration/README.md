# par-term Shell Integration

Shell integration provides enhanced terminal features by embedding semantic markers
in your shell's prompt and command output.

## Features

- Working directory tracking in tab titles
- Command exit status indicators
- Prompt navigation between commands

## Installation

### From par-term

Open Settings (F12) -> Integrations -> Install Shell Integration

### Manual (curl)

```bash
curl -fsSL https://paulrobello.github.io/par-term/install-shell-integration.sh | bash
```

## Technical Details

Uses OSC 133 protocol (also used by iTerm2, VSCode, WezTerm).
