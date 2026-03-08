# Terminal Search

par-term includes a powerful search feature for finding text in the terminal's scrollback buffer.

## Table of Contents
- [Overview](#overview)
- [Opening Search](#opening-search)
- [Search Modes](#search-modes)
- [Navigation](#navigation)
- [Configuration](#configuration)
- [Keyboard Shortcuts](#keyboard-shortcuts)
- [Technical Details](#technical-details)
- [Related Documentation](#related-documentation)

## Overview

The search feature provides real-time text search with multiple modes:

```mermaid
graph TD
    Search[Search UI]
    Input[Search Input]
    Options[Search Options]
    Results[Match Results]
    Highlight[Match Highlighting]

    Search --> Input
    Search --> Options
    Input --> Results
    Options --> Results
    Results --> Highlight

    Options --> CaseSensitive[Case Sensitive]
    Options --> Regex[Regex Mode]
    Options --> WholeWord[Whole Word]

    style Search fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Input fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Options fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Results fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Highlight fill:#880e4f,stroke:#c2185b,stroke-width:2px,color:#ffffff
    style CaseSensitive fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style Regex fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
    style WholeWord fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

## Opening Search

Press `Cmd+F` (macOS) or `Ctrl+Shift+F` (Windows/Linux) to open the search bar.

### Search Bar UI

```mermaid
graph LR
    subgraph Row1["Row 1: Input & Navigation"]
        Label["Search:"]
        Input["[Text Input]"]
        Counter["3 of 42"]
        Up["▲"]
        Down["▼"]
        Close["✕"]
    end

    subgraph Row2["Row 2: Options"]
        Case["Aa"]
        Regex[".*"]
        Word["\\b"]
    end

    subgraph Row3["Row 3: Keyboard Hints"]
        Hints["Enter: Next | Shift+Enter: Prev | Escape: Close"]
    end

    style Label fill:#37474f,stroke:#78909c,stroke-width:1px,color:#ffffff
    style Input fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Counter fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Case fill:#37474f,stroke:#78909c,stroke-width:1px,color:#ffffff
    style Regex fill:#37474f,stroke:#78909c,stroke-width:1px,color:#ffffff
    style Word fill:#37474f,stroke:#78909c,stroke-width:1px,color:#ffffff
```

## Search Modes

### Plain Text (Default)
- Finds literal string matches
- Supports substring matching
- Multiple matches per line

### Case Sensitive
Toggle with the **Aa** button.

| Mode | "Hello" matches |
|------|-----------------|
| Off (default) | "hello", "HELLO", "Hello" |
| On | "Hello" only |

### Regular Expression
Toggle with the **.\*** button.

| Pattern | Matches |
|---------|---------|
| `error\|warn` | "error" or "warn" |
| `\d{4}` | Any 4-digit number |
| `^$` | Empty lines |
| `func\w+\(` | Function calls |

**Invalid Regex:** Shows "Invalid" in match counter with error message.

### Whole Word
Toggle with the **\b** button.

| Query | Whole Word Off | Whole Word On |
|-------|----------------|---------------|
| "test" | "test", "testing", "contest" | "test" only |
| "log" | "log", "logging", "dialog" | "log" only |

## Navigation

### Match Counter
Displays current position: `3 of 42`

### Navigation Controls
- **▲ (Up Arrow)** - Previous match
- **▼ (Down Arrow)** - Next match
- **✕** - Close search

### Auto-Scroll
When navigating, the terminal automatically scrolls to center the current match on screen.

### Wrap Around
Navigation wraps from last match to first (and vice versa).

## Configuration

Add these options to `~/.config/par-term/config.yaml`:

```yaml
# Default search behavior
search_case_sensitive: false    # Case-insensitive by default
search_regex: false             # Plain text by default
search_wrap_around: true        # Wrap navigation

# Highlight colors [R, G, B, A] (0-255)
search_highlight_color: [255, 200, 0, 180]          # Yellow
search_current_highlight_color: [255, 100, 0, 220]  # Orange
```

### Settings UI Options

The Terminal tab in Settings provides:

| Option | Description |
|--------|-------------|
| **Match highlight** | Color picker for all matches |
| **Current match** | Color picker for current match |
| **Case sensitive by default** | Start searches case-sensitive |
| **Use regex by default** | Start searches in regex mode |
| **Wrap around when navigating** | Enable wrap-around navigation |

### Default Highlight Colors

| Match Type | Color | RGBA |
|------------|-------|------|
| Regular matches | Yellow | (255, 200, 0, 180) |
| Current match | Orange | (255, 100, 0, 220) |

## Keyboard Shortcuts

| Shortcut | Action |
|----------|--------|
| `Cmd+F` (macOS) / `Ctrl+Shift+F` (Linux/Windows) | Open/close search bar |
| `Enter` | Next match |
| `Shift + Enter` | Previous match |
| `Escape` | Close search bar |
| `Cmd+G` (macOS) / `Ctrl+G` (Linux/Windows) | Next match (when search is open) |
| `Cmd+Shift+G` (macOS) / `Ctrl+Shift+G` (Linux/Windows) | Previous match (when search is open) |

## Technical Details

### Architecture

```mermaid
graph TD
    UI[SearchUI]
    Engine[SearchEngine]
    Config[SearchConfig]
    Matches[Vec&lt;SearchMatch&gt;]

    UI --> Engine
    UI --> Config
    Engine --> Matches

    subgraph SearchEngine
        PlainSearch[Plain Text Search]
        RegexSearch[Regex Search]
        Cache[Cached Regex]
    end

    Engine --> PlainSearch
    Engine --> RegexSearch
    RegexSearch --> Cache

    style UI fill:#e65100,stroke:#ff9800,stroke-width:3px,color:#ffffff
    style Engine fill:#1b5e20,stroke:#4caf50,stroke-width:2px,color:#ffffff
    style Config fill:#0d47a1,stroke:#2196f3,stroke-width:2px,color:#ffffff
    style Matches fill:#4a148c,stroke:#9c27b0,stroke-width:2px,color:#ffffff
    style Cache fill:#37474f,stroke:#78909c,stroke-width:2px,color:#ffffff
```

### Search Scope
- Current screen content
- All scrollback buffer lines
- Multiple matches per line supported

### Performance
- 150ms debounce on query changes
- Regex patterns compiled once and cached for repeated searches
- Handles Unicode and emoji correctly
- Wide characters properly positioned

### Match Data
Each match tracks:
- Line number (absolute position in scrollback)
- Column (character position)
- Length (match length in characters)

### Implementation Files
- `src/search/mod.rs` - Search UI overlay (`SearchUI`)
- `src/search/engine.rs` - Search engine with regex caching (`SearchEngine`)
- `src/search/types.rs` - Search types (`SearchMatch`, `SearchConfig`, `SearchAction`)
- `par-term-config/src/config/config_struct/search_config.rs` - Configuration struct
- `par-term-settings-ui/src/terminal_tab/search.rs` - Settings UI section

## Related Documentation

- [Command History](COMMAND_HISTORY.md) - Fuzzy command history search (separate from terminal text search)
- [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) - All keyboard shortcuts
- [README.md](../README.md) - Project overview
