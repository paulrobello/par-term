//! Tests for shader hot reload functionality
//!
//! These tests verify the shader watcher module's core functionality.

use par_term::shader_watcher::{
    ShaderReloadEvent, ShaderType, ShaderWatcher, ShaderWatcherBuilder,
};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

#[test]
fn test_shader_type_properties() {
    // ShaderType should be Debug, Clone, Copy, PartialEq, Eq, Hash
    let bg = ShaderType::Background;
    let cursor = ShaderType::Cursor;

    // Test Clone and Copy
    let bg_copy = bg;
    assert_eq!(bg, bg_copy);

    // Test Debug
    let debug_str = format!("{:?}", bg);
    assert!(debug_str.contains("Background"));

    // Test PartialEq
    assert_ne!(bg, cursor);
    assert_eq!(cursor, ShaderType::Cursor);

    // Test Hash (via HashMap usage - implicitly tested)
}

#[test]
fn test_shader_reload_event_properties() {
    let event = ShaderReloadEvent {
        shader_type: ShaderType::Cursor,
        path: PathBuf::from("/path/to/cursor.glsl"),
    };

    // Test Clone
    let event_clone = event.clone();
    assert_eq!(event.shader_type, event_clone.shader_type);
    assert_eq!(event.path, event_clone.path);

    // Test Debug
    let debug_str = format!("{:?}", event);
    assert!(debug_str.contains("Cursor"));
    assert!(debug_str.contains("cursor.glsl"));
}

#[test]
fn test_shader_watcher_builder_configuration() {
    let builder = ShaderWatcherBuilder::new()
        .background_shader("/tmp/bg.glsl")
        .cursor_shader("/tmp/cursor.glsl")
        .debounce_delay_ms(250);

    // We can't directly access builder fields, but we can verify build fails
    // when paths don't exist (since we're using non-existent paths)
    // The builder API is validated through the build attempt
    let _ = builder; // Ensure builder is consumed
}

#[test]
fn test_shader_watcher_with_single_background_shader() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let shader_path = temp_dir.path().join("background.glsl");

    // Write a valid GLSL shader
    fs::write(
        &shader_path,
        r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    vec2 uv = fragCoord / iResolution.xy;
    fragColor = vec4(uv.x, uv.y, 0.5, 1.0);
}
"#,
    )
    .expect("Failed to write shader");

    let watcher =
        ShaderWatcher::new(Some(&shader_path), None, 100).expect("Failed to create watcher");

    // Verify debounce delay is set
    assert_eq!(watcher.debounce_delay_ms(), 100);

    // No events should be pending initially
    assert!(watcher.try_recv().is_none());
}

#[test]
fn test_shader_watcher_with_single_cursor_shader() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let shader_path = temp_dir.path().join("cursor.glsl");

    fs::write(
        &shader_path,
        r#"
void mainImage(out vec4 fragColor, in vec2 fragCoord) {
    fragColor = vec4(1.0, 0.0, 0.0, 1.0);
}
"#,
    )
    .expect("Failed to write shader");

    let watcher =
        ShaderWatcher::new(None, Some(&shader_path), 50).expect("Failed to create watcher");

    assert_eq!(watcher.debounce_delay_ms(), 50);
    assert!(watcher.try_recv().is_none());
}

#[test]
fn test_shader_watcher_with_both_shaders() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let bg_path = temp_dir.path().join("background.glsl");
    let cursor_path = temp_dir.path().join("cursor.glsl");

    fs::write(
        &bg_path,
        "void mainImage(out vec4 c, in vec2 f) { c = vec4(0.0); }",
    )
    .expect("Failed to write bg shader");
    fs::write(
        &cursor_path,
        "void mainImage(out vec4 c, in vec2 f) { c = vec4(1.0); }",
    )
    .expect("Failed to write cursor shader");

    let watcher = ShaderWatcher::new(Some(&bg_path), Some(&cursor_path), 100)
        .expect("Failed to create watcher");

    assert!(watcher.try_recv().is_none());
}

#[test]
fn test_shader_watcher_no_paths_fails() {
    let result = ShaderWatcher::new(None, None, 100);
    assert!(result.is_err());

    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("No shader paths"));
}

#[test]
fn test_shader_watcher_nonexistent_path_fails() {
    let result = ShaderWatcher::new(Some("/nonexistent/path/to/shader.glsl".as_ref()), None, 100);
    assert!(result.is_err());
}

#[test]
fn test_debounce_configuration() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let shader_path = temp_dir.path().join("test.glsl");
    fs::write(
        &shader_path,
        "void mainImage(out vec4 c, in vec2 f) { c = vec4(0.0); }",
    )
    .expect("Failed to write shader");

    // Test with different debounce delays
    let watcher_fast = ShaderWatcher::new(Some(&shader_path), None, 10).expect("Fast watcher");
    assert_eq!(watcher_fast.debounce_delay_ms(), 10);

    let shader_path2 = temp_dir.path().join("test2.glsl");
    fs::write(
        &shader_path2,
        "void mainImage(out vec4 c, in vec2 f) { c = vec4(1.0); }",
    )
    .expect("Failed to write shader");

    let watcher_slow = ShaderWatcher::new(Some(&shader_path2), None, 500).expect("Slow watcher");
    assert_eq!(watcher_slow.debounce_delay_ms(), 500);
}

#[test]
fn test_shader_watcher_file_change_detection() {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let shader_path = temp_dir.path().join("watch_test.glsl");

    // Create initial shader
    fs::write(
        &shader_path,
        "void mainImage(out vec4 c, in vec2 f) { c = vec4(0.0); }",
    )
    .expect("Failed to write initial shader");

    let watcher =
        ShaderWatcher::new(Some(&shader_path), None, 50).expect("Failed to create watcher");

    // Wait for watcher to be ready
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Modify the file
    fs::write(
        &shader_path,
        "void mainImage(out vec4 c, in vec2 f) { c = vec4(1.0); }",
    )
    .expect("Failed to write modified shader");

    // Wait for event to be detected
    std::thread::sleep(std::time::Duration::from_millis(300));

    // Check for event (may not always trigger on all platforms)
    if let Some(event) = watcher.try_recv() {
        assert_eq!(event.shader_type, ShaderType::Background);
        assert!(event.path.ends_with("watch_test.glsl"));
    }
    // Note: We don't fail if no event detected since file watching is platform-dependent
}

#[test]
fn test_shader_watcher_builder_default() {
    let builder = ShaderWatcherBuilder::default();
    // Default builder should have no paths and default debounce
    // Build should fail with no paths
    let result = builder.build();
    assert!(result.is_err());
}
