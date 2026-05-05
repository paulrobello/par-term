# par-term Enhancement Ideas

**Important** Remove completed items from this list to save context for future runs. Ensure you update changelog.

---

## AI Assistant Panel

- **Terminal output summarizer**: Add a one-click "Summarize" action that sends the visible terminal buffer (or last N lines) to the connected ACP agent with a summarization prompt. Useful for long build logs, test output, or error cascades.
- **Smart paste guard**: Before pasting multi-line content into the terminal, offer an AI-powered review that warns about destructive commands (`rm -rf`, `DROP TABLE`, force pushes) and suggests safer alternatives.
- **Context-aware command suggestions**: Track the current directory, recent commands, and visible error messages to offer contextual shell suggestions via the assistant panel (e.g., "you ran `cargo build` and it failed — try `cargo build 2>&1 | less`").
- **Agent workspace persistence**: Save agent conversation history across sessions (with opt-in) so users can resume AI-assisted debugging or development without re-explaining context.
- **Multi-agent orchestration**: Allow chaining multiple ACP agents in sequence — e.g., Claude analyzes a bug, then Codex writes the fix, then a linter agent validates it.

## Automation & Scripting

- **Visual trigger builder**: Add a drag-and-drop UI in Settings for building automation triggers without writing regex. Preview matches against recent terminal output in real-time.
- **Trigger templates library**: Bundle common trigger patterns (detect build failures, watch for SSH disconnects, monitor test results, track deployment progress) as importable templates.
- **Script hot-reload**: Watch observer scripts for file changes and automatically restart them, similar to shader hot-reload.
- **Automation health dashboard**: Show trigger match counts, coprocess uptime, script health, and action execution history in a unified UI panel.

## Performance & Diagnostics

- **GPU metrics overlay**: Real-time overlay showing FPS, frame time, draw call count, texture memory usage, and atlas occupancy. Toggle with a keybinding. Useful for shader authors and performance debugging.
- **Startup profiler**: Instrument and log time spent in each initialization phase (config load, font atlas build, GPU device creation, PTY spawn) to surface slow startups.
- **Memory usage breakdown**: Add a diagnostic command or panel showing memory allocated per subsystem (glyph cache, scrollback buffers, inline image textures, egui state).
- **Rendering pipeline visualization**: Debug overlay that highlights which panes are being re-rendered each frame and why, helping identify unnecessary full-redraws.

## Terminal Features

- **Image protocol unification**: Abstract Sixel, iTerm2, and Kitty image protocols behind a single "inline image" API so protocols can be negotiated per-session and images degrade gracefully on incompatible terminals.
- **Terminal hyperlinks (OSC 8) rendering**: Render OSC 8 hyperlinks with visible underline styling and hover tooltip showing the URL, in addition to the existing click-to-open behavior.
- **Unicode grapheme cluster cursor**: Make cursor movement respect grapheme cluster boundaries (emoji sequences, combining characters) so the cursor never splits a logical character.
- **Column-based selection mode**: Add a fifth selection mode that selects full columns, useful for extracting tabular data from terminal output.
- **Bracketed paste markers**: Visual indicator when content is pasted in bracketed-paste mode, so users can distinguish typed vs pasted input.

## Theming & Visual

- **Theme system with import/export**: Define named color themes (beyond the current foreground/background/accent) that can be exported/imported as YAML files. Include community theme gallery support.
- **Per-profile color themes**: Allow each profile to specify its own color theme, automatically switching when the profile activates.
- **Tab bar color themes**: Separate color theming for the tab bar area, independent of terminal colors.
- **Transparent background with blur**: Extend background transparency (currently macOS-only) to Linux/Wayland compositors that support it, with configurable blur radius.
- **ANSI color palette preview**: In Settings, show a live preview grid of all 16 ANSI colors with sample text for readability checking.

## Accessibility

- **Screen reader support**: Integrate with platform accessibility APIs (macOS Accessibility, AT-SPI on Linux) to expose terminal content as accessible text.
- **High-contrast mode**: A one-toggle mode that ensures minimum 4.5:1 contrast ratio for all text, overriding user/theme colors where needed.
- **Keyboard-driven pane management**: Add keyboard shortcuts for all pane operations (create, resize, close, swap, equalize) without reaching for the mouse.
- **Audible bell customization**: Allow custom sound files for the terminal bell, with per-profile volume and sound selection.

## Tabs & Panes

- **Tab groups / workspaces**: Group tabs into named workspaces that can be switched as a unit (all tabs in a workspace hide/show together). Useful for separating work contexts.
- **Pane broadcast input**: Send keyboard input to all panes in a tab simultaneously (useful for running the same command across multiple servers).
- **Pane zoom toggle**: Temporarily maximize a single pane to fill the entire tab area, then toggle back to the split layout. Common in tmux.
- **Equalize panes command**: A single keybinding to reset all pane dividers to equal sizes.
- **Pane snapshot / diff**: Capture the visible content of a pane and diff it against a later snapshot, useful for monitoring log output changes.

## Search & Navigation

- **Multi-tab search**: Search across all open tabs simultaneously and show results in a unified list, clicking a result navigates to that tab and line.
- **Search result annotations**: Mark search matches in the scrollbar with colored markers, similar to IDE search result indicators.
- **Regex capture group highlighting**: When using regex search mode, highlight each capture group in a different color for easier visual parsing.
- **Jump-to-line**: Accept a line number (from compiler output, stack traces) and jump directly to that line in scrollback.

## Session Management

- **Session tabs restore ordering**: When restoring a session, preserve the exact tab order and pane layout including split ratios, not just which tabs were open.
- **Named session snapshots**: Allow users to save named session snapshots (all tabs, panes, CWDs, shell state) that can be restored later, separate from auto-restore.
- **Session diff**: Compare two session snapshots to see what changed (tabs added/removed, working directories changed).

## Shell Integration

- **Shell integration auto-installer**: Detect the user's shell and automatically install integration hooks on first run, with a guided setup wizard.
- **Command exit code in tab title**: Show the exit code of the last command in the tab title (green checkmark for 0, red X for non-zero).
- **Working directory in tab title template**: Add `{cwd}` and `{cwd_short}` variables to tab title format strings, auto-populated from OSC 7 tracking.

## Security

- **Password detection in paste**: Detect and warn when pasting content that looks like a password, API key, or secret token.
- **Per-profile permission scoping**: Restrict what automation triggers and coprocesses can do per-profile (e.g., a "production" profile disables Run Command actions).
- **Audit log**: Optional log of all automation actions taken (triggers fired, commands run, scripts started) with timestamps for compliance.

## Cross-Platform

- **Windows Terminal ConPTY integration**: Ensure first-class Windows support with ConPTY backend, Windows Terminal-compatible settings import.
- **Wayland native support**: Implement zwp_text_input_v3 for IME, zwlr_layer_shell_v1 for dropdown/quaketype windows on Wayland without XWayland dependency.
- **Linux desktop integration**: Register as a terminal handler in `.desktop` files, support activating tabs from other apps via D-Bus.

## Developer Experience

- **VT sequence inspector**: A developer panel that shows raw escape sequences received from the PTY in real-time, with decoded descriptions. Invaluable for debugging terminal rendering issues.
- **Config schema generation**: Generate a JSON Schema from the Config struct for use in editors (VS Code YAML validation, etc.).
- **Regression test harness for rendering**: Pixel-diff based regression tests that render known VT sequences and compare output against reference screenshots.
- **Shader performance profiling mode**: A mode that instruments shader execution time per frame and reports it back to the shader author via a uniform or overlay.

## Quality of Life

- **Quick command palette**: A Cmd+Shift+P style command palette (like VS Code) that searches all actions, settings, snippets, and keybindings by name.
- **Tab search / switcher**: A Cmd+P style fuzzy tab switcher overlay that shows tab titles and working directories for quick navigation when many tabs are open.
- **Clipboard history**: Maintain a configurable-size history of copied text with preview and re-paste capability, stored in memory or on disk.
- **Pin tab**: Allow pinning tabs so they cannot be accidentally closed (require explicit unpin first).
- **Tab title editing via double-click**: Double-click a tab to enter inline rename mode instead of requiring the context menu.

---

## Shader Authoring and Discovery (existing)

- **Shader preset browser**: Expand the Effects tab into a gallery with thumbnails, categories, favorites, and "safe for readability" labels for bundled and user-installed background shaders.

## Developer and Ecosystem Features (existing)

- **Gallery metadata generation**: Generate `docs/SHADERS.md`, website gallery entries, thumbnails, and `shaders/manifest.json` from a single shader metadata source.
- **Community shader submission checklist**: Document readability, license, performance, metadata, and screenshot requirements for contributed background shaders.
- **Performance budget hints**: Surface approximate frame cost per shader and recommend default animation speeds or low-power behavior.
- **Uniform debug overlay**: Add a developer overlay that displays live values for `iResolution`, `iTime`, `iMouse`, `iProgress`, cursor uniforms, channel resolutions, and current resolved shader config.
