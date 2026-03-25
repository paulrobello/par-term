# par-term Documentation

Navigation index for all par-term documentation. Start with the [Getting Started](GETTING_STARTED.md) guide if you are new, or use the tables below to jump directly to a topic.

## Getting Started

| Document | Description |
|----------|-------------|
| [Keyboard Shortcuts](KEYBOARD_SHORTCUTS.md) | Complete reference for all keyboard shortcuts, customizable keybindings, and available actions |
| [Mouse Features](MOUSE_FEATURES.md) | Text selection, URL handling, cursor positioning, and semantic history via mouse |
| [Integrations](INTEGRATIONS.md) | Shell integration setup, shader collection, and third-party tool support |
| [Profiles](PROFILES.md) | Profile system for saving and launching terminal sessions with custom configurations |

## User Guides

| Document | Description |
|----------|-------------|
| [Tabs](TABS.md) | Multi-tab interface, tab creation, switching, reordering, icons, and context menus |
| [Search](SEARCH.md) | Text search in the terminal scrollback buffer with regex and case-sensitive modes |
| [Copy Mode](COPY_MODE.md) | Vi-style keyboard-driven text selection and navigation |
| [Scrollback & Command Marks](SCROLLBACK.md) | Scrollback buffer management, command markers, and prompt navigation |
| [Command History](COMMAND_HISTORY.md) | Fuzzy command history search overlay for finding and re-executing previous commands |
| [Paste Special](PASTE_SPECIAL.md) | Clipboard content transformations (29 transforms across 5 categories) before pasting |
| [Snippets & Actions](SNIPPETS.md) | Saved text blocks, shell commands, and automated tasks via keyboard shortcuts |
| [SSH Host Management](SSH.md) | SSH host discovery, quick connect, SSH profiles, and automatic profile switching |
| [File Transfers](FILE_TRANSFERS.md) | Native file transfer support using the iTerm2 OSC 1337 protocol with progress overlay |
| [Semantic History](SEMANTIC_HISTORY.md) | Click file paths in terminal output to open them in your editor |
| [Command Separators](COMMAND_SEPARATORS.md) | Visual separator lines between shell commands using shell integration |

## Window & Session Management

| Document | Description |
|----------|-------------|
| [Window Management](WINDOW_MANAGEMENT.md) | Edge-anchored windows, multi-monitor support, and window behavior |
| [Window Arrangements](ARRANGEMENTS.md) | Save and restore complete window layouts including positions, sizes, and tabs |
| [Session Management](SESSION_MANAGEMENT.md) | Reopen closed tabs and restore complete session state on startup |
| [Session Logging](SESSION_LOGGING.md) | Record terminal output for review, sharing, or playback |

## Visual Customization

| Document | Description |
|----------|-------------|
| [Custom Shaders Guide](CUSTOM_SHADERS.md) | Writing custom GLSL shaders for background effects and post-processing |
| [Included Shaders](SHADERS.md) | Gallery of 52 ready-to-use GLSL shaders (40 background + 12 cursor) |
| [Compositor](COMPOSITOR.md) | GPU compositor architecture, rendering layers, transparency, and shader integration |
| [Badges](BADGES.md) | Dynamic session information overlays with variable substitution |
| [Status Bar](STATUS_BAR.md) | Configurable status bar with widgets for session info and system metrics |
| [Progress Bars](PROGRESS_BARS.md) | Thin overlay progress bars driven by OSC escape sequences |

## Configuration & Preferences

| Document | Description |
|----------|-------------|
| [Preferences Import/Export](PREFERENCES_IMPORT_EXPORT.md) | Import and export terminal configuration for backup, sharing, and team use |
| [Self-Update](SELF_UPDATE.md) | In-place update checking and installation for standalone and app bundle installs |
| [Accessibility](ACCESSIBILITY.md) | Contrast enforcement and display options for readability |

## Automation & Advanced Features

| Document | Description |
|----------|-------------|
| [Automation](AUTOMATION.md) | Triggers, actions, coprocesses, and observer scripts for terminal automation |
| [Content Prettifier](PRETTIFIER.md) | Rich rendering of Markdown, JSON, YAML, diffs, and diagrams in terminal output |
| [Assistant Panel](ASSISTANT_PANEL.md) | DevTools-style panel for terminal inspection and ACP agent chat (Claude, Ollama) |

## Architecture & Development

| Document | Description |
|----------|-------------|
| [Getting Started](GETTING_STARTED.md) | Install, launch, and configure par-term in under 10 minutes |
| [Architecture](ARCHITECTURE.md) | High-level system architecture, core components, data flow, and rendering pipeline |
| [Crate Structure](CRATE_STRUCTURE.md) | Workspace crate dependency layers, version bump order, and adding new sub-crates |
| [Concurrency](CONCURRENCY.md) | Lock hierarchy, mutex taxonomy, and deadlock prevention patterns |
| [Mutex Patterns](MUTEX_PATTERNS.md) | Practical reference for the three-mutex system used across the codebase |
| [State Lifecycle](STATE_LIFECYCLE.md) | Lifecycle of key state objects from creation through teardown |
| [Config Reference](CONFIG_REFERENCE.md) | Complete reference for all 200+ configuration options |
| [Environment Variables](ENVIRONMENT_VARIABLES.md) | Runtime environment variable reference |
| [API](API.md) | Public types across all workspace crates |
| [Logging](LOGGING.md) | Debug logging configuration, log levels, and custom debug macros |
| [ACP Harness](ACP_HARNESS.md) | CLI debugging tool for reproducing ACP agent behavior outside the GUI |
| [Troubleshooting](TROUBLESHOOTING.md) | Solutions for common issues and error messages |
| [Enterprise Deployment](ENTERPRISE_DEPLOYMENT.md) | Managed deployment, configuration distribution, and fleet management |
| [Quick Start: Fonts](QUICK_START_FONTS.md) | Set up font families, CJK, emoji, and math symbols in 5 minutes |
| [Documentation Style Guide](DOCUMENTATION_STYLE_GUIDE.md) | Standards and best practices for project documentation |
