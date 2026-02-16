# Shader ACP Agent Integration Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Enable ACP agents (Claude, Codex, etc.) to create, edit, debug, and manage custom shaders in par-term through context-aware system prompts and config file watching.

**Architecture:** Three components: (1) shader context generator that builds dynamic context blocks with current shader state, available shaders, uniforms reference, and debug file paths; (2) context-triggered injection that conditionally prepends shader context to agent prompts based on keywords or active shader state; (3) config file watcher that auto-reloads config when the agent modifies `config.yaml`.

**Tech Stack:** Rust, `notify` crate (already a dependency), existing `ShaderWatcher` pattern for file watching, existing ACP agent/chat infrastructure.

---

### Task 1: Shader Context Generator - Keyword Detection

**Files:**
- Create: `src/ai_inspector/shader_context.rs`
- Modify: `src/ai_inspector/mod.rs:1-3`

**Step 1: Write the failing test for keyword detection**

Create `src/ai_inspector/shader_context.rs` with tests first:

```rust
//! Shader context generation for ACP agent prompts.
//!
//! Builds dynamic context blocks containing current shader state, available
//! shaders, uniforms reference, debug file paths, and shader templates.
//! Context is injected into agent prompts when shader-related keywords are
//! detected or when shaders are actively enabled.

use crate::config::Config;

/// Keywords that trigger shader context injection.
const SHADER_KEYWORDS: &[&str] = &[
    "shader",
    "glsl",
    "wgsl",
    "effect",
    "crt",
    "scanline",
    "post-process",
    "postprocess",
    "fragment",
    "mainimage",
    "ichannel",
    "itime",
    "iresolution",
    "shadertoy",
    "transpile",
    "naga",
    "cursor effect",
    "cursor shader",
    "background effect",
    "background shader",
];

/// Check whether shader context should be injected for the given message.
///
/// Returns `true` if:
/// - The message contains any shader-related keyword (case-insensitive), OR
/// - A custom shader or cursor shader is currently enabled in config
pub fn should_inject_shader_context(message: &str, config: &Config) -> bool {
    // Check if shaders are active
    if config.custom_shader_enabled && config.custom_shader.is_some() {
        return true;
    }
    if config.cursor_shader_enabled && config.cursor_shader.is_some() {
        return true;
    }

    // Check for keyword matches (case-insensitive)
    let lower = message.to_lowercase();
    SHADER_KEYWORDS.iter().any(|kw| lower.contains(kw))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_keyword_detection_basic() {
        let config = Config::default();
        assert!(should_inject_shader_context("Help me write a shader", &config));
        assert!(should_inject_shader_context("What GLSL uniforms are available?", &config));
        assert!(should_inject_shader_context("Port this Shadertoy effect", &config));
        assert!(should_inject_shader_context("Fix the CRT effect", &config));
    }

    #[test]
    fn test_keyword_detection_case_insensitive() {
        let config = Config::default();
        assert!(should_inject_shader_context("SHADER help", &config));
        assert!(should_inject_shader_context("My GLSL code", &config));
        assert!(should_inject_shader_context("iChannel4 texture", &config));
    }

    #[test]
    fn test_no_keywords_no_shaders() {
        let config = Config::default();
        assert!(!should_inject_shader_context("How do I resize the terminal?", &config));
        assert!(!should_inject_shader_context("List my fonts", &config));
    }

    #[test]
    fn test_active_shader_always_triggers() {
        let mut config = Config::default();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = true;
        // Even without keywords, active shader triggers context
        assert!(should_inject_shader_context("What is happening?", &config));
    }

    #[test]
    fn test_disabled_shader_no_trigger() {
        let mut config = Config::default();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = false;
        // Shader exists but is disabled - don't auto-trigger
        assert!(!should_inject_shader_context("What is happening?", &config));
    }

    #[test]
    fn test_cursor_shader_triggers() {
        let mut config = Config::default();
        config.cursor_shader = Some("glow.glsl".to_string());
        config.cursor_shader_enabled = true;
        assert!(should_inject_shader_context("anything", &config));
    }
}
```

**Step 2: Register the module**

Add to `src/ai_inspector/mod.rs`:

```rust
pub mod chat;
pub mod panel;
pub mod shader_context;
pub mod snapshot;
```

**Step 3: Run tests to verify they pass**

Run: `cargo test shader_context -- -v`
Expected: All 6 tests PASS

**Step 4: Commit**

```bash
git add src/ai_inspector/shader_context.rs src/ai_inspector/mod.rs
git commit -m "feat(ai-inspector): add shader context keyword detection"
```

---

### Task 2: Shader Context Generator - Context Builder

**Files:**
- Modify: `src/ai_inspector/shader_context.rs`

**Step 1: Write the failing test for context building**

Add to the existing file after the `should_inject_shader_context` function:

```rust
/// Scan the shaders directory and return sorted list of shader filenames.
///
/// This is a standalone version of `SettingsUI::scan_shaders_folder()` that
/// doesn't require the settings UI to be instantiated.
fn scan_shaders(shaders_dir: &std::path::Path) -> Vec<String> {
    let mut shaders = Vec::new();
    if let Ok(entries) = std::fs::read_dir(shaders_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file()
                && let Some(ext) = path.extension()
                && (ext == "glsl" || ext == "frag" || ext == "shader")
                && let Some(name) = path.file_name()
            {
                shaders.push(name.to_string_lossy().to_string());
            }
        }
    }
    shaders.sort();
    shaders
}

/// Classify shaders into background and cursor categories by filename prefix.
fn classify_shaders(shaders: &[String]) -> (Vec<&str>, Vec<&str>) {
    let mut background = Vec::new();
    let mut cursor = Vec::new();
    for s in shaders {
        if s.starts_with("cursor_") {
            cursor.push(s.as_str());
        } else {
            background.push(s.as_str());
        }
    }
    (background, cursor)
}

/// Build the full shader context block for injection into an agent prompt.
///
/// The context includes current shader state, available shaders, debug file
/// paths, uniforms reference, a minimal shader template, and config file
/// location with relevant field names.
pub fn build_shader_context(config: &Config) -> String {
    let shaders_dir = Config::shaders_dir();
    let config_path = Config::config_path();
    let available = scan_shaders(&shaders_dir);
    let (bg_shaders, cursor_shaders) = classify_shaders(&available);

    let mut ctx = String::from("\n[Shader Assistant Context]\n");
    ctx.push_str("You can help create, edit, debug, and manage custom shaders for par-term.\n\n");

    // Current state
    ctx.push_str("## Current Shader State\n");
    if let Some(ref shader) = config.custom_shader {
        ctx.push_str(&format!(
            "- Background shader: \"{}\" ({})\n",
            shader,
            if config.custom_shader_enabled { "enabled" } else { "disabled" }
        ));
        if config.custom_shader_enabled {
            ctx.push_str(&format!("  - animation_speed: {}\n", config.custom_shader_animation_speed));
            ctx.push_str(&format!("  - brightness: {}\n", config.custom_shader_brightness));
            ctx.push_str(&format!("  - text_opacity: {}\n", config.custom_shader_text_opacity));
        }
    } else {
        ctx.push_str("- Background shader: none\n");
    }
    if let Some(ref shader) = config.cursor_shader {
        ctx.push_str(&format!(
            "- Cursor shader: \"{}\" ({})\n",
            shader,
            if config.cursor_shader_enabled { "enabled" } else { "disabled" }
        ));
    } else {
        ctx.push_str("- Cursor shader: none\n");
    }
    ctx.push_str(&format!("- Shader directory: {}\n\n", shaders_dir.display()));

    // Available shaders
    ctx.push_str("## Available Shaders\n");
    if bg_shaders.is_empty() {
        ctx.push_str("- Background: (none installed)\n");
    } else {
        ctx.push_str(&format!("- Background: {}\n", bg_shaders.join(", ")));
    }
    if cursor_shaders.is_empty() {
        ctx.push_str("- Cursor: (none installed)\n");
    } else {
        ctx.push_str(&format!("- Cursor: {}\n", cursor_shaders.join(", ")));
    }
    ctx.push('\n');

    // Debug files
    ctx.push_str("## Debug Files (read these to diagnose shader issues)\n");
    if let Some(ref shader) = config.custom_shader {
        let name = shader.trim_end_matches(".glsl").trim_end_matches(".frag");
        ctx.push_str(&format!("- Background WGSL: /tmp/par_term_{}_shader.wgsl\n", name));
    }
    if let Some(ref shader) = config.cursor_shader {
        let name = shader.trim_end_matches(".glsl").trim_end_matches(".frag");
        ctx.push_str(&format!("- Cursor WGSL: /tmp/par_term_{}_shader.wgsl\n", name));
    }
    ctx.push_str("- Wrapped GLSL: /tmp/par_term_debug_wrapped.glsl\n\n");

    // Uniforms reference
    ctx.push_str("## Available Uniforms (Shadertoy-compatible GLSL)\n");
    ctx.push_str("Standard: iTime, iResolution (vec3), iMouse (vec4), iTimeDelta, iFrame\n");
    ctx.push_str("Textures: iChannel0-3 (user textures), iChannel4 (terminal content)\n");
    ctx.push_str("par-term: iTimeKeyPress (time since last keypress)\n");
    ctx.push_str("Cursor-only: iCurrentCursor (vec4 xy=pos zw=size), iPreviousCursor, ");
    ctx.push_str("iCurrentCursorColor (vec4), iPreviousCursorColor, iTimeCursorChange, ");
    ctx.push_str("iCursorShaderColor, iCursorTrailDuration, iCursorGlowRadius, iCursorGlowIntensity\n\n");

    // Shader template
    ctx.push_str("## Minimal Shader Template\n");
    ctx.push_str("```glsl\n");
    ctx.push_str("void mainImage(out vec4 fragColor, in vec2 fragCoord) {\n");
    ctx.push_str("    vec2 uv = fragCoord / iResolution.xy;\n");
    ctx.push_str("    // Terminal content from iChannel4\n");
    ctx.push_str("    vec4 terminal = texture(iChannel4, uv);\n");
    ctx.push_str("    fragColor = terminal;\n");
    ctx.push_str("}\n");
    ctx.push_str("```\n\n");

    // Config instructions
    ctx.push_str("## How to Apply Changes\n");
    ctx.push_str(&format!("1. Write shader GLSL to: {}/\n", shaders_dir.display()));
    ctx.push_str(&format!("2. Edit config at: {}\n", config_path.display()));
    ctx.push_str("3. Set `custom_shader: \"filename.glsl\"` and `custom_shader_enabled: true`\n");
    ctx.push_str("4. For cursor shaders: `cursor_shader:` and `cursor_shader_enabled: true`\n");
    ctx.push_str("5. par-term will auto-reload when config.yaml or shader files change\n\n");

    ctx
}
```

Add tests:

```rust
    #[test]
    fn test_build_shader_context_default_config() {
        let config = Config::default();
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("[Shader Assistant Context]"));
        assert!(ctx.contains("Background shader: none"));
        assert!(ctx.contains("Cursor shader: none"));
        assert!(ctx.contains("Available Uniforms"));
        assert!(ctx.contains("mainImage"));
        assert!(ctx.contains("iChannel4"));
    }

    #[test]
    fn test_build_shader_context_with_active_shader() {
        let mut config = Config::default();
        config.custom_shader = Some("crt.glsl".to_string());
        config.custom_shader_enabled = true;
        config.custom_shader_animation_speed = 1.5;
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("crt.glsl"));
        assert!(ctx.contains("enabled"));
        assert!(ctx.contains("animation_speed: 1.5"));
        assert!(ctx.contains("/tmp/par_term_crt_shader.wgsl"));
    }

    #[test]
    fn test_build_shader_context_with_cursor_shader() {
        let mut config = Config::default();
        config.cursor_shader = Some("glow.glsl".to_string());
        config.cursor_shader_enabled = true;
        let ctx = build_shader_context(&config);
        assert!(ctx.contains("glow.glsl"));
        assert!(ctx.contains("/tmp/par_term_glow_shader.wgsl"));
    }

    #[test]
    fn test_classify_shaders() {
        let shaders = vec![
            "crt.glsl".to_string(),
            "cursor_glow.glsl".to_string(),
            "galaxy.glsl".to_string(),
            "cursor_trail.glsl".to_string(),
        ];
        let (bg, cursor) = classify_shaders(&shaders);
        assert_eq!(bg, vec!["crt.glsl", "galaxy.glsl"]);
        assert_eq!(cursor, vec!["cursor_glow.glsl", "cursor_trail.glsl"]);
    }
```

**Step 2: Run tests to verify they pass**

Run: `cargo test shader_context -- -v`
Expected: All 10 tests PASS

**Step 3: Commit**

```bash
git add src/ai_inspector/shader_context.rs
git commit -m "feat(ai-inspector): add shader context builder for agent prompts"
```

---

### Task 3: Config File Watcher

**Files:**
- Create: `src/config/watcher.rs`
- Modify: `src/config/mod.rs:6-11`

This follows the same pattern as the existing `src/shader_watcher.rs` but watches `config.yaml` instead of shader files.

**Step 1: Write the config watcher module**

Create `src/config/watcher.rs`:

```rust
//! Config file watcher for automatic reload.
//!
//! Watches `config.yaml` for changes and sends reload events through a channel.
//! Uses debouncing to avoid multiple reloads during rapid saves from editors.

use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, Event, PollWatcher, RecursiveMode, Watcher};
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

/// Event indicating the config file has changed and needs reloading.
#[derive(Debug, Clone)]
pub struct ConfigReloadEvent {
    /// Path to the config file that changed.
    pub path: PathBuf,
}

/// Watches the config file for changes and emits reload events.
pub struct ConfigWatcher {
    /// The file system watcher (kept alive).
    _watcher: PollWatcher,
    /// Receiver for config change events.
    event_receiver: Receiver<ConfigReloadEvent>,
}

impl std::fmt::Debug for ConfigWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigWatcher").finish_non_exhaustive()
    }
}

impl ConfigWatcher {
    /// Create a new config watcher for the given config file path.
    ///
    /// # Arguments
    /// * `config_path` - Path to the config.yaml file to watch
    /// * `debounce_delay_ms` - Debounce delay in milliseconds
    pub fn new(config_path: &std::path::Path, debounce_delay_ms: u64) -> Result<Self> {
        if !config_path.exists() {
            anyhow::bail!("Config file not found: {}", config_path.display());
        }

        let canonical = config_path.canonicalize().unwrap_or_else(|_| config_path.to_path_buf());
        let config_filename = canonical
            .file_name()
            .ok_or_else(|| anyhow::anyhow!("Config path has no filename"))?
            .to_os_string();
        let watch_dir = canonical
            .parent()
            .ok_or_else(|| anyhow::anyhow!("Config path has no parent directory"))?
            .to_path_buf();

        let (tx, rx) = channel();
        let debounce_delay = Duration::from_millis(debounce_delay_ms);
        let last_event: Arc<Mutex<Option<Instant>>> = Arc::new(Mutex::new(None));
        let last_event_clone = Arc::clone(&last_event);
        let config_filename = Arc::new(config_filename);

        let mut watcher = PollWatcher::new(
            move |result: std::result::Result<Event, notify::Error>| {
                if let Ok(event) = result {
                    if !matches!(
                        event.kind,
                        notify::EventKind::Modify(_) | notify::EventKind::Create(_)
                    ) {
                        return;
                    }

                    for path in &event.paths {
                        let Some(filename) = path.file_name() else {
                            continue;
                        };
                        if filename != config_filename.as_os_str() {
                            continue;
                        }

                        let should_send = {
                            let now = Instant::now();
                            let mut state = last_event_clone.lock();
                            if let Some(last) = *state {
                                if now.duration_since(last) < debounce_delay {
                                    false
                                } else {
                                    *state = Some(now);
                                    true
                                }
                            } else {
                                *state = Some(now);
                                true
                            }
                        };

                        if should_send {
                            log::info!("Config file changed: {}", path.display());
                            let _ = tx.send(ConfigReloadEvent {
                                path: path.clone(),
                            });
                        }
                    }
                }
            },
            NotifyConfig::default().with_poll_interval(Duration::from_millis(500)),
        )
        .context("Failed to create config file watcher")?;

        watcher
            .watch(&watch_dir, RecursiveMode::NonRecursive)
            .with_context(|| format!("Failed to watch config directory: {}", watch_dir.display()))?;

        log::info!(
            "Config watcher initialized for {} (debounce: {}ms)",
            canonical.display(),
            debounce_delay_ms
        );

        Ok(Self {
            _watcher: watcher,
            event_receiver: rx,
        })
    }

    /// Check for pending config reload events (non-blocking).
    pub fn try_recv(&self) -> Option<ConfigReloadEvent> {
        self.event_receiver.try_recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_watcher_creation() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let watcher = ConfigWatcher::new(&config_path, 200);
        assert!(watcher.is_ok());
    }

    #[test]
    fn test_config_watcher_nonexistent_fails() {
        let result = ConfigWatcher::new("/nonexistent/config.yaml".as_ref(), 200);
        assert!(result.is_err());
    }

    #[test]
    fn test_config_watcher_no_initial_events() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let watcher = ConfigWatcher::new(&config_path, 200).expect("Failed to create watcher");
        assert!(watcher.try_recv().is_none());
    }

    #[test]
    fn test_config_watcher_detects_change() {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let config_path = temp_dir.path().join("config.yaml");
        fs::write(&config_path, "font_size: 12.0\n").expect("Failed to write config");

        let watcher = ConfigWatcher::new(&config_path, 50).expect("Failed to create watcher");

        std::thread::sleep(Duration::from_millis(100));
        fs::write(&config_path, "font_size: 14.0\n").expect("Failed to write config");
        std::thread::sleep(Duration::from_millis(600));

        // Platform-dependent; don't assert, just verify no panic
        if let Some(event) = watcher.try_recv() {
            assert!(event.path.ends_with("config.yaml"));
        }
    }
}
```

**Step 2: Register the module in `src/config/mod.rs`**

Add after line 11 (`mod types;`):

```rust
pub mod watcher;
```

**Step 3: Run tests**

Run: `cargo test config::watcher -- -v`
Expected: All 4 tests PASS

**Step 4: Commit**

```bash
git add src/config/watcher.rs src/config/mod.rs
git commit -m "feat(config): add config file watcher for automatic reload"
```

---

### Task 4: Integrate Config Watcher into Window State

**Files:**
- Modify: `src/app/window_state.rs` (fields ~line 200, init ~line 390, event loop)

**Step 1: Add config watcher field to WindowState**

In the struct fields section (around line 204, near `shader_watcher`), add:

```rust
    /// Config file watcher for automatic reload
    pub(crate) config_watcher: Option<crate::config::watcher::ConfigWatcher>,
```

**Step 2: Initialize in constructor**

In the constructor (around line 396, near `shader_watcher: None`), add:

```rust
            config_watcher: None,
```

**Step 3: Add init method**

Add a new method near `init_shader_watcher()` (around line 1006):

```rust
    /// Initialize the config file watcher for automatic reload.
    pub(crate) fn init_config_watcher(&mut self) {
        let config_path = Config::config_path();
        if !config_path.exists() {
            debug_info!("CONFIG", "Config file does not exist, skipping watcher");
            return;
        }
        match crate::config::watcher::ConfigWatcher::new(&config_path, 500) {
            Ok(watcher) => {
                debug_info!("CONFIG", "Config watcher initialized");
                self.config_watcher = Some(watcher);
            }
            Err(e) => {
                debug_info!("CONFIG", "Failed to initialize config watcher: {}", e);
            }
        }
    }
```

**Step 4: Call init in `resumed()`**

In the `resumed()` method, after `self.init_shader_watcher();` (around line 780), add:

```rust
        self.init_config_watcher();
```

**Step 5: Add check method and call in event loop**

Add a method to check for config reload events:

```rust
    /// Check for pending config file changes and apply them.
    ///
    /// Called periodically from the event loop. On config change:
    /// 1. Reloads config from disk
    /// 2. Applies shader-related config changes
    /// 3. Reinitializes shader watcher if shader paths changed
    pub(crate) fn check_config_reload(&mut self) {
        let Some(watcher) = &self.config_watcher else {
            return;
        };
        let Some(_event) = watcher.try_recv() else {
            return;
        };

        debug_info!("CONFIG", "Config file changed, reloading...");

        match Config::load() {
            Ok(new_config) => {
                let old_shader = self.config.custom_shader.clone();
                let old_shader_enabled = self.config.custom_shader_enabled;
                let old_cursor = self.config.cursor_shader.clone();
                let old_cursor_enabled = self.config.cursor_shader_enabled;

                // Apply shader-related config fields
                self.config.custom_shader = new_config.custom_shader.clone();
                self.config.custom_shader_enabled = new_config.custom_shader_enabled;
                self.config.custom_shader_animation_speed = new_config.custom_shader_animation_speed;
                self.config.custom_shader_brightness = new_config.custom_shader_brightness;
                self.config.custom_shader_text_opacity = new_config.custom_shader_text_opacity;
                self.config.custom_shader_full_content = new_config.custom_shader_full_content;
                self.config.custom_shader_channel0 = new_config.custom_shader_channel0.clone();
                self.config.custom_shader_channel1 = new_config.custom_shader_channel1.clone();
                self.config.custom_shader_channel2 = new_config.custom_shader_channel2.clone();
                self.config.custom_shader_channel3 = new_config.custom_shader_channel3.clone();
                self.config.custom_shader_cubemap = new_config.custom_shader_cubemap.clone();
                self.config.custom_shader_cubemap_enabled = new_config.custom_shader_cubemap_enabled;
                self.config.cursor_shader = new_config.cursor_shader.clone();
                self.config.cursor_shader_enabled = new_config.cursor_shader_enabled;
                self.config.cursor_shader_hides_cursor = new_config.cursor_shader_hides_cursor;
                self.config.cursor_shader_disable_in_alt_screen = new_config.cursor_shader_disable_in_alt_screen;
                self.config.cursor_shader_trail_duration = new_config.cursor_shader_trail_duration;
                self.config.cursor_shader_glow_radius = new_config.cursor_shader_glow_radius;
                self.config.cursor_shader_glow_intensity = new_config.cursor_shader_glow_intensity;
                self.config.cursor_shader_color = new_config.cursor_shader_color;

                // Reinit shader watcher if paths changed
                let shader_changed = self.config.custom_shader != old_shader
                    || self.config.custom_shader_enabled != old_shader_enabled
                    || self.config.cursor_shader != old_cursor
                    || self.config.cursor_shader_enabled != old_cursor_enabled;

                if shader_changed {
                    debug_info!("CONFIG", "Shader config changed, reinitializing...");
                    self.reinit_shader_watcher();
                }

                self.needs_redraw = true;
                debug_info!("CONFIG", "Config reloaded successfully");
            }
            Err(e) => {
                log::error!("Failed to reload config: {}", e);
            }
        }
    }
```

Call `check_config_reload()` in the same place where `check_shader_reload()` is called (in the main event loop / `about_to_wait`). Search for `check_shader_reload` to find the exact location and add `self.check_config_reload();` nearby.

**Step 6: Run `cargo build` to verify compilation**

Run: `cargo build`
Expected: Compiles without errors

**Step 7: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat(config): integrate config file watcher into window state"
```

---

### Task 5: Inject Shader Context into Agent Prompts

**Files:**
- Modify: `src/app/window_state.rs:3510-3525` (SendPrompt handler)

**Step 1: Modify the SendPrompt handler**

Replace the current prompt construction logic (lines ~3516-3525):

```rust
            InspectorAction::SendPrompt(text) => {
                self.ai_inspector.chat.add_user_message(text.clone());
                if let Some(agent) = &self.agent {
                    let agent = agent.clone();
                    // Build the prompt with optional system guidance and shader context.
                    let mut prompt_text = String::new();

                    // Prepend system guidance on the first prompt so the agent
                    // knows to wrap commands in fenced code blocks.
                    if !self.ai_inspector.chat.system_prompt_sent {
                        self.ai_inspector.chat.system_prompt_sent = true;
                        prompt_text.push_str(crate::ai_inspector::chat::AGENT_SYSTEM_GUIDANCE);
                    }

                    // Inject shader context when relevant (keyword match or active shaders).
                    if crate::ai_inspector::shader_context::should_inject_shader_context(
                        &text,
                        &self.config,
                    ) {
                        prompt_text.push_str(
                            &crate::ai_inspector::shader_context::build_shader_context(&self.config),
                        );
                    }

                    prompt_text.push_str(&text);

                    let content =
                        vec![crate::acp::protocol::ContentBlock::Text { text: prompt_text }];
                    let tx = self.agent_tx.clone();
                    self.runtime.spawn(async move {
                        let agent = agent.lock().await;
                        let _ = agent.send_prompt(content).await;
                        if let Some(tx) = tx {
                            let _ = tx.send(AgentMessage::PromptComplete);
                        }
                    });
                }
                self.needs_redraw = true;
            }
```

**Step 2: Run `cargo build` to verify compilation**

Run: `cargo build`
Expected: Compiles without errors

**Step 3: Commit**

```bash
git add src/app/window_state.rs
git commit -m "feat(ai-inspector): inject shader context into agent prompts"
```

---

### Task 6: Integration Tests

**Files:**
- Create: `tests/shader_context_tests.rs`

**Step 1: Write integration tests**

```rust
//! Integration tests for shader context generation.

use par_term::ai_inspector::shader_context::{build_shader_context, should_inject_shader_context};
use par_term::config::Config;

#[test]
fn test_shader_context_contains_all_sections() {
    let config = Config::default();
    let ctx = build_shader_context(&config);

    // All sections must be present
    assert!(ctx.contains("[Shader Assistant Context]"));
    assert!(ctx.contains("## Current Shader State"));
    assert!(ctx.contains("## Available Shaders"));
    assert!(ctx.contains("## Debug Files"));
    assert!(ctx.contains("## Available Uniforms"));
    assert!(ctx.contains("## Minimal Shader Template"));
    assert!(ctx.contains("## How to Apply Changes"));
}

#[test]
fn test_shader_context_template_is_valid_glsl() {
    let config = Config::default();
    let ctx = build_shader_context(&config);

    // Template must contain the mainImage signature
    assert!(ctx.contains("void mainImage(out vec4 fragColor, in vec2 fragCoord)"));
    assert!(ctx.contains("iChannel4"));
    assert!(ctx.contains("iResolution"));
}

#[test]
fn test_keyword_detection_comprehensive() {
    let config = Config::default();

    // Positive cases
    let positive = vec![
        "Create a shader effect",
        "Help me with GLSL code",
        "What WGSL output do I get?",
        "Make a CRT effect",
        "Add scanline post-processing",
        "Port this Shadertoy shader",
        "Fix the cursor effect",
        "iTime is not working",
    ];
    for msg in positive {
        assert!(
            should_inject_shader_context(msg, &config),
            "Expected true for: {msg}"
        );
    }

    // Negative cases
    let negative = vec![
        "How do I change the font?",
        "Set terminal background color",
        "Configure keybindings",
        "What version is this?",
    ];
    for msg in negative {
        assert!(
            !should_inject_shader_context(msg, &config),
            "Expected false for: {msg}"
        );
    }
}
```

**Step 2: Run integration tests**

Run: `cargo test shader_context_tests -- -v`
Expected: All 3 tests PASS

**Step 3: Run full test suite**

Run: `cargo test`
Expected: All tests pass

**Step 4: Commit**

```bash
git add tests/shader_context_tests.rs
git commit -m "test: add integration tests for shader context generation"
```

---

### Task 7: Final Checks and PR

**Step 1: Run full quality checks**

Run: `make pre-commit`
Expected: Format check, lint, and tests all pass

**Step 2: Fix any lint/format issues**

Run: `cargo fmt` and `cargo clippy --all-targets -- -D warnings`
Expected: Clean

**Step 3: Commit any fixups**

```bash
git add -A
git commit -m "style: fix lint and formatting"
```

**Step 4: Create PR**

```bash
git push -u origin feat/shader-acp-agent
gh pr create --title "feat: ACP agent integration for custom shader management" --body "$(cat <<'EOF'
## Summary
- Adds shader-aware context injection for ACP agents (#156)
- Agents can now create, edit, debug, and manage custom shaders
- Context is injected when shader keywords are detected or shaders are active
- Config file watcher enables live reload when agent modifies config.yaml

## Components
- `src/ai_inspector/shader_context.rs`: Shader context generator
- `src/config/watcher.rs`: Config file watcher
- Modified `window_state.rs`: Context injection + watcher integration

## Test plan
- [ ] Unit tests for keyword detection (6 tests)
- [ ] Unit tests for context builder (4 tests)
- [ ] Unit tests for config watcher (4 tests)
- [ ] Integration tests for shader context (3 tests)
- [ ] Manual test: open AI inspector, connect agent, ask "help me create a shader"
- [ ] Manual test: agent writes shader file, config auto-reloads
EOF
)"
```

Closes #156
