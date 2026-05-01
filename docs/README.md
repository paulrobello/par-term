# par-term Documentation

> **Note**: The canonical project README is at the repository root ([../README.md](../README.md)). This file is the documentation index for the `docs/` directory.

Navigation index for all par-term documentation. Start with the [Getting Started](guides/GETTING_STARTED.md) guide if you are new, or use the tables below to jump directly to a topic.

## Getting Started

| Document | Description |
|----------|-------------|
| [Getting Started](guides/GETTING_STARTED.md) | Install, launch, and configure par-term in under 10 minutes |
| [Keyboard Shortcuts](guides/KEYBOARD_SHORTCUTS.md) | Complete reference for all keyboard shortcuts, customizable keybindings, and available actions |
| [Mouse Features](features/MOUSE_FEATURES.md) | Text selection, URL handling, cursor positioning, and semantic history via mouse |
| [Integrations](features/INTEGRATIONS.md) | Shell integration setup, shader collection, and third-party tool support |
| [Profiles](features/PROFILES.md) | Profile system for saving and launching terminal sessions with custom configurations |

## User Guides

| Document | Description |
|----------|-------------|
| [Tabs](features/TABS.md) | Multi-tab interface, tab creation, switching, reordering, icons, and context menus |
| [Search](features/SEARCH.md) | Text search in the terminal scrollback buffer with regex and case-sensitive modes |
| [Copy Mode](features/COPY_MODE.md) | Vi-style keyboard-driven text selection and navigation |
| [Scrollback & Command Marks](features/SCROLLBACK.md) | Scrollback buffer management, command markers, and prompt navigation |
| [Command History](features/COMMAND_HISTORY.md) | Fuzzy command history search overlay for finding and re-executing previous commands |
| [Paste Special](features/PASTE_SPECIAL.md) | Clipboard content transformations (29 transforms across 5 categories) before pasting |
| [Snippets & Actions](features/SNIPPETS.md) | Saved text blocks, shell commands, and automated tasks via keyboard shortcuts |
| [SSH Host Management](features/SSH.md) | SSH host discovery, quick connect, SSH profiles, and automatic profile switching |
| [File Transfers](features/FILE_TRANSFERS.md) | Native file transfer support using the iTerm2 OSC 1337 protocol with progress overlay |
| [Semantic History](features/SEMANTIC_HISTORY.md) | Click file paths in terminal output to open them in your editor |
| [Command Separators](features/COMMAND_SEPARATORS.md) | Visual separator lines between shell commands using shell integration |

## Window & Session Management

| Document | Description |
|----------|-------------|
| [Window Management](features/WINDOW_MANAGEMENT.md) | Edge-anchored windows, multi-monitor support, and window behavior |
| [Window Arrangements](features/ARRANGEMENTS.md) | Save and restore complete window layouts including positions, sizes, and tabs |
| [Session Management](features/SESSION_MANAGEMENT.md) | Reopen closed tabs and restore complete session state on startup |
| [Session Logging](features/SESSION_LOGGING.md) | Record terminal output for review, sharing, or playback |

## Visual Customization

| Document | Description |
|----------|-------------|
| [Custom Shaders Guide](features/CUSTOM_SHADERS.md) | Writing custom GLSL shaders for background effects and post-processing |
| [Included Shaders](features/SHADERS.md) | Gallery of 73 ready-to-use GLSL shaders (61 background + 12 cursor) |
| [Compositor](architecture/COMPOSITOR.md) | GPU compositor architecture, rendering layers, transparency, and shader integration |
| [Badges](features/BADGES.md) | Dynamic session information overlays with variable substitution |
| [Status Bar](features/STATUS_BAR.md) | Configurable status bar with widgets for session info and system metrics |
| [Progress Bars](features/PROGRESS_BARS.md) | Thin overlay progress bars driven by OSC escape sequences |

## Configuration & Preferences

| Document | Description |
|----------|-------------|
| [Preferences Import/Export](features/PREFERENCES_IMPORT_EXPORT.md) | Import and export terminal configuration for backup, sharing, and team use |
| [Self-Update](features/SELF_UPDATE.md) | In-place update checking and installation for standalone and app bundle installs |
| [Accessibility](features/ACCESSIBILITY.md) | Contrast enforcement and display options for readability |

## Automation & Advanced Features

| Document | Description |
|----------|-------------|
| [Automation](features/AUTOMATION.md) | Triggers, actions, coprocesses, and observer scripts for terminal automation |
| [Assistant Panel](ASSISTANT_PANEL.md) | DevTools-style panel for terminal inspection and ACP agent chat (Claude, Ollama) |

## Architecture & Development

| Document | Description |
|----------|-------------|
| [Contributing](../CONTRIBUTING.md) | Development setup, build commands, code style, sub-crate architecture, and PR guidelines |
| [Architecture](architecture/ARCHITECTURE.md) | High-level system architecture, core components, data flow, and rendering pipeline |
| [Crate Structure](architecture/CRATE_STRUCTURE.md) | Workspace crate dependency layers, version bump order, and adding new sub-crates |
| [Concurrency](architecture/CONCURRENCY.md) | Lock hierarchy, mutex taxonomy, and deadlock prevention patterns |
| [Mutex Patterns](architecture/MUTEX_PATTERNS.md) | Practical reference for the three-mutex system used across the codebase |
| [State Lifecycle](architecture/STATE_LIFECYCLE.md) | Lifecycle of key state objects from creation through teardown |
| [Config Reference](CONFIG_REFERENCE.md) | Complete reference for all 200+ configuration options |
| [Migration Guide](guides/MIGRATION.md) | Breaking config changes and upgrade notes across major version groups |
| [Environment Variables](guides/ENVIRONMENT_VARIABLES.md) | Runtime environment variable reference |
| [API](API.md) | Public types across all workspace crates |
| [Logging](LOGGING.md) | Debug logging configuration, log levels, and custom debug macros |
| [ACP Harness](ACP_HARNESS.md) | CLI debugging tool for reproducing ACP agent behavior outside the GUI |
| [Troubleshooting](guides/TROUBLESHOOTING.md) | Solutions for common issues and error messages |
| [Enterprise Deployment](ENTERPRISE_DEPLOYMENT.md) | Managed deployment, configuration distribution, and fleet management |
| [Quick Start: Fonts](guides/QUICK_START_FONTS.md) | Set up font families, CJK, emoji, and math symbols in 5 minutes |
| [Documentation Style Guide](DOCUMENTATION_STYLE_GUIDE.md) | Standards and best practices for project documentation |
