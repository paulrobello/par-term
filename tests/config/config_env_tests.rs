//! Integration tests for environment variable substitution, allowlists,
//! tab bar position, auto dark mode, and auto tab style config settings.

use par_term::config::{
    Config, TabBarPosition, TabConfig, TabStyle, ThemeColorsConfig, is_env_var_allowed,
    substitute_variables, substitute_variables_with_lookup,
};

// ============================================================================
// Variable Substitution Tests
// ============================================================================

/// Substitute against a fixed variable set instead of the process environment.
///
/// These tests used to `std::env::set_var` a uniquely-named variable, justified
/// on the grounds that no other concurrent test read that name. That argument
/// does not hold: glibc's `setenv` can reallocate and free the `environ` array,
/// so a concurrent `getenv` on any thread — inside libc, a dependency, or std
/// itself, reading a completely unrelated variable — can read freed memory. The
/// hazard is the shared array, not the key. Resolving through a lookup keeps
/// these tests hermetic and removes the hazard rather than narrowing it.
///
/// A variable absent from `vars` reads as unset.
fn subst(input: &str, vars: &[(&str, &str)]) -> String {
    subst_with(input, false, vars)
}

/// As [`subst`], but with the allowlist bypassed (`allow_all_env_vars: true`).
fn subst_all(input: &str, vars: &[(&str, &str)]) -> String {
    subst_with(input, true, vars)
}

fn subst_with(input: &str, allow_all: bool, vars: &[(&str, &str)]) -> String {
    substitute_variables_with_lookup(input, allow_all, |name| {
        vars.iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| (*value).to_string())
    })
}

/// An allowlisted variable the OS always sets, used to exercise substitution of
/// an ambient (rather than test-injected) variable.
///
/// `HOME` is Unix-only — Windows sets `USERPROFILE` instead and leaves `HOME`
/// unset, which made these tests fail there. Both names are on the allowlist,
/// so either exercises the same path. This is read, never mutated — nothing in
/// this file writes to the process environment; see `subst`.
#[cfg(windows)]
const AMBIENT_HOME_VAR: &str = "USERPROFILE";
#[cfg(not(windows))]
const AMBIENT_HOME_VAR: &str = "HOME";

#[test]
fn test_substitute_variables_basic_env_var() {
    let result = subst(
        "value: ${PAR_TERM_TEST_VAR}",
        &[("PAR_TERM_TEST_VAR", "hello_world")],
    );
    assert_eq!(result, "value: hello_world");
}

#[test]
fn test_substitute_variables_home_and_user() {
    let home = std::env::var(AMBIENT_HOME_VAR)
        .unwrap_or_else(|_| panic!("{AMBIENT_HOME_VAR} should be set by the OS"));
    let result = substitute_variables(&format!("path: ${{{AMBIENT_HOME_VAR}}}/Pictures/bg.png"));
    assert_eq!(result, format!("path: {home}/Pictures/bg.png"));
}

#[test]
fn test_substitute_variables_multiple_vars() {
    let result = subst(
        "${PAR_TERM_TEST_A} and ${PAR_TERM_TEST_B}",
        &[("PAR_TERM_TEST_A", "alpha"), ("PAR_TERM_TEST_B", "beta")],
    );
    assert_eq!(result, "alpha and beta");
}

#[test]
fn test_substitute_variables_missing_var_unchanged() {
    // Unset vars should remain as-is (PAR_TERM_ prefix so it passes allowlist)
    let result = subst("value: ${PAR_TERM_NONEXISTENT_12345}", &[]);
    assert_eq!(result, "value: ${PAR_TERM_NONEXISTENT_12345}");
}

#[test]
fn test_substitute_variables_default_value() {
    let result = subst("shell: ${PAR_TERM_MISSING_WITH_DEFAULT:-/bin/bash}", &[]);
    assert_eq!(result, "shell: /bin/bash");
}

#[test]
fn test_substitute_variables_default_value_not_used_when_set() {
    let result = subst(
        "shell: ${PAR_TERM_SET_WITH_DEFAULT:-/bin/bash}",
        &[("PAR_TERM_SET_WITH_DEFAULT", "/bin/zsh")],
    );
    assert_eq!(result, "shell: /bin/zsh");
}

#[test]
fn test_substitute_variables_escaped_dollar() {
    // $${VAR} should produce the literal ${VAR}
    let result = subst(
        "literal: $${PAR_TERM_TEST_ESC}",
        &[("PAR_TERM_TEST_ESC", "should_not_appear")],
    );
    assert_eq!(result, "literal: ${PAR_TERM_TEST_ESC}");
}

#[test]
fn test_substitute_variables_no_vars() {
    let input = "cols: 80\nrows: 24\nfont_size: 12.0";
    let result = substitute_variables(input);
    assert_eq!(result, input);
}

#[test]
fn test_substitute_variables_adjacent_vars() {
    let result = subst(
        "${PAR_TERM_TEST_X}${PAR_TERM_TEST_Y}",
        &[("PAR_TERM_TEST_X", "foo"), ("PAR_TERM_TEST_Y", "bar")],
    );
    assert_eq!(result, "foobar");
}

#[test]
fn test_substitute_variables_in_yaml_config() {
    let yaml = r#"
font_family: "${PAR_TERM_TEST_FONT}"
window_title: "${PAR_TERM_TEST_TITLE}"
cols: 120
"#;
    let substituted = subst(
        yaml,
        &[
            ("PAR_TERM_TEST_FONT", "Fira Code"),
            ("PAR_TERM_TEST_TITLE", "My Terminal"),
        ],
    );
    let config: Config = serde_yaml_ng::from_str(&substituted).unwrap();
    assert_eq!(config.font_family, "Fira Code");
    assert_eq!(config.window_title, "My Terminal");
    assert_eq!(config.cols, 120);
}

#[test]
fn test_substitute_variables_partial_string() {
    let result = subst(
        "badge: ${PAR_TERM_TEST_USER}@myhost",
        &[("PAR_TERM_TEST_USER", "testuser")],
    );
    assert_eq!(result, "badge: testuser@myhost");
}

#[test]
fn test_substitute_variables_empty_default() {
    let result = subst("val: ${PAR_TERM_EMPTY_DEFAULT:-}", &[]);
    assert_eq!(result, "val: ");
}

// ============================================================================
// Environment Variable Allowlist Tests
// ============================================================================

#[test]
fn test_allowlist_permits_common_safe_vars() {
    // These should all be on the allowlist
    assert!(is_env_var_allowed("HOME"));
    assert!(is_env_var_allowed("USER"));
    assert!(is_env_var_allowed("USERNAME"));
    assert!(is_env_var_allowed("LOGNAME"));
    assert!(is_env_var_allowed("SHELL"));
    assert!(is_env_var_allowed("TERM"));
    assert!(is_env_var_allowed("LANG"));
    assert!(is_env_var_allowed("PATH"));
    assert!(is_env_var_allowed("EDITOR"));
    assert!(is_env_var_allowed("VISUAL"));
    assert!(is_env_var_allowed("PAGER"));
    assert!(is_env_var_allowed("TMPDIR"));
    assert!(is_env_var_allowed("DISPLAY"));
    assert!(is_env_var_allowed("WAYLAND_DISPLAY"));
    assert!(is_env_var_allowed("HOSTNAME"));
    assert!(is_env_var_allowed("HOST"));
    assert!(is_env_var_allowed("COLORTERM"));
    assert!(is_env_var_allowed("TERM_PROGRAM"));
}

#[test]
fn test_allowlist_permits_xdg_vars() {
    assert!(is_env_var_allowed("XDG_CONFIG_HOME"));
    assert!(is_env_var_allowed("XDG_DATA_HOME"));
    assert!(is_env_var_allowed("XDG_STATE_HOME"));
    assert!(is_env_var_allowed("XDG_CACHE_HOME"));
    assert!(is_env_var_allowed("XDG_RUNTIME_DIR"));
}

#[test]
fn test_allowlist_permits_windows_vars() {
    assert!(is_env_var_allowed("APPDATA"));
    assert!(is_env_var_allowed("LOCALAPPDATA"));
    assert!(is_env_var_allowed("USERPROFILE"));
}

#[test]
fn test_allowlist_permits_par_term_prefix() {
    assert!(is_env_var_allowed("PAR_TERM_CUSTOM"));
    assert!(is_env_var_allowed("PAR_TERM_SOMETHING_ELSE"));
    assert!(is_env_var_allowed("PAR_TERM_"));
}

#[test]
fn test_allowlist_permits_lc_prefix() {
    assert!(is_env_var_allowed("LC_ALL"));
    assert!(is_env_var_allowed("LC_CTYPE"));
    assert!(is_env_var_allowed("LC_MESSAGES"));
    assert!(is_env_var_allowed("LC_COLLATE"));
}

#[test]
fn test_allowlist_blocks_sensitive_vars() {
    assert!(!is_env_var_allowed("AWS_SECRET_ACCESS_KEY"));
    assert!(!is_env_var_allowed("API_KEY"));
    assert!(!is_env_var_allowed("GITHUB_TOKEN"));
    assert!(!is_env_var_allowed("DATABASE_URL"));
    assert!(!is_env_var_allowed("SECRET_KEY"));
    assert!(!is_env_var_allowed("OPENAI_API_KEY"));
    assert!(!is_env_var_allowed("SSH_PRIVATE_KEY"));
    assert!(!is_env_var_allowed("RANDOM_VAR"));
}

#[test]
fn test_substitute_allowlisted_var_resolves() {
    // An allowlisted, OS-provided variable must resolve.
    let home = std::env::var(AMBIENT_HOME_VAR)
        .unwrap_or_else(|_| panic!("{AMBIENT_HOME_VAR} should be set by the OS"));
    let result = substitute_variables(&format!("path: ${{{AMBIENT_HOME_VAR}}}/config"));
    assert_eq!(result, format!("path: {home}/config"));
}

#[test]
fn test_substitute_non_allowlisted_var_blocked() {
    // A non-allowlisted variable must NOT be substituted, even when it resolves
    let result = subst(
        "key: ${SECRET_API_KEY_TEST_M3}",
        &[("SECRET_API_KEY_TEST_M3", "super_secret")],
    );
    // Should remain as literal text, not resolved
    assert_eq!(result, "key: ${SECRET_API_KEY_TEST_M3}");
}

#[test]
fn test_substitute_par_term_prefix_resolves() {
    let result = subst(
        "setting: ${PAR_TERM_MY_SETTING}",
        &[("PAR_TERM_MY_SETTING", "custom_value")],
    );
    assert_eq!(result, "setting: custom_value");
}

#[test]
fn test_substitute_lc_prefix_resolves() {
    let result = subst(
        "locale: ${LC_TEST_LOCALE}",
        &[("LC_TEST_LOCALE", "en_US.UTF-8")],
    );
    assert_eq!(result, "locale: en_US.UTF-8");
}

#[test]
fn test_substitute_allow_all_overrides_allowlist() {
    // With allow_all=true, even non-allowlisted vars should resolve
    let result = subst_all(
        "key: ${SECRET_OVERRIDE_TEST_M3}",
        &[("SECRET_OVERRIDE_TEST_M3", "resolved_secret")],
    );
    assert_eq!(result, "key: resolved_secret");
}

#[test]
fn test_substitute_allow_all_false_blocks_non_allowlisted() {
    let result = subst(
        "val: ${BLOCKED_VAR_TEST_M3}",
        &[("BLOCKED_VAR_TEST_M3", "should_not_appear")],
    );
    assert_eq!(result, "val: ${BLOCKED_VAR_TEST_M3}");
}

#[test]
fn test_substitute_mixed_allowed_and_blocked() {
    let result = subst(
        "good: ${PAR_TERM_GOOD}, bad: ${NAUGHTY_SECRET_TEST_M3}",
        &[
            ("PAR_TERM_GOOD", "allowed"),
            ("NAUGHTY_SECRET_TEST_M3", "blocked"),
        ],
    );
    assert_eq!(result, "good: allowed, bad: ${NAUGHTY_SECRET_TEST_M3}");
}

#[test]
fn test_substitute_non_allowlisted_with_default_uses_literal() {
    // Non-allowlisted var with a default — the entire ${VAR:-default} is left as-is
    let result = subst("val: ${BLOCKED_DEFAULT_TEST_M3:-fallback}", &[]);
    // The variable is blocked, so the placeholder stays as literal text
    assert_eq!(result, "val: ${BLOCKED_DEFAULT_TEST_M3:-fallback}");
}

#[test]
fn test_substitute_escaped_dollar_still_works_with_allowlist() {
    // Escaped dollars should still produce literal ${VAR} regardless of allowlist
    let result = substitute_variables("literal: $${HOME}");
    assert_eq!(result, "literal: ${HOME}");
}

#[test]
fn test_config_default_allow_all_env_vars_is_false() {
    let config = Config::default();
    assert!(!config.security.allow_all_env_vars);
}

// ============================================================================
// Tab Bar Position Configuration Tests
// ============================================================================

#[test]
fn test_tab_bar_position_default() {
    let config = Config::default();
    assert_eq!(config.tabs.tab_bar_position, TabBarPosition::Top);
    assert_eq!(config.tabs.tab_bar_width, 160.0);
}

#[test]
fn test_tab_bar_position_serialization() {
    // Round-trip serialization for all variants
    for &position in TabBarPosition::all() {
        let config = Config {
            tabs: TabConfig {
                tab_bar_position: position,
                ..Default::default()
            },
            ..Config::default()
        };

        let yaml = serde_yaml_ng::to_string(&config).unwrap();
        let deserialized: Config = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(
            deserialized.tabs.tab_bar_position, position,
            "Round-trip failed for {:?}",
            position
        );
    }
}

#[test]
fn test_tab_bar_position_yaml_variants() {
    let yaml = r#"tab_bar_position: top"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.tab_bar_position, TabBarPosition::Top);

    let yaml = r#"tab_bar_position: bottom"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.tab_bar_position, TabBarPosition::Bottom);

    let yaml = r#"tab_bar_position: left"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.tab_bar_position, TabBarPosition::Left);
}

#[test]
fn test_tab_bar_position_partial_yaml() {
    // Missing tab_bar_position should default to Top
    let yaml = r#"
cols: 100
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.tab_bar_position, TabBarPosition::Top);
    assert_eq!(config.tabs.tab_bar_width, 160.0);
}

#[test]
fn test_tab_bar_width_yaml_deserialization() {
    let yaml = r#"
tab_bar_position: left
tab_bar_width: 250.0
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.tab_bar_position, TabBarPosition::Left);
    assert!((config.tabs.tab_bar_width - 250.0).abs() < f32::EPSILON);
}

#[test]
fn test_tab_bar_width_yaml_serialization() {
    let config = Config {
        tabs: TabConfig {
            tab_bar_position: TabBarPosition::Left,
            tab_bar_width: 200.0,
            ..Default::default()
        },
        ..Config::default()
    };

    let yaml = serde_yaml_ng::to_string(&config).unwrap();
    assert!(yaml.contains("tab_bar_position: left"));
    assert!(yaml.contains("tab_bar_width: 200.0"));
}

// ============================================================================
// Auto Dark Mode Tests
// ============================================================================

#[test]
fn test_auto_dark_mode_defaults() {
    let config = Config::default();
    assert!(!config.theme_colors.auto_dark_mode);
    assert_eq!(config.theme_colors.light_theme, "light-background");
    assert_eq!(config.theme_colors.dark_theme, "dark-background");
}

#[test]
fn test_apply_system_theme_disabled() {
    let mut config = Config {
        theme_colors: ThemeColorsConfig {
            auto_dark_mode: false,
            ..Default::default()
        },
        ..Config::default()
    };
    // Should not change theme when auto_dark_mode is off
    assert!(!config.apply_system_theme(true));
    assert!(!config.apply_system_theme(false));
    assert_eq!(config.theme_colors.theme, "dark-background");
}

#[test]
fn test_apply_system_theme_dark() {
    let mut config = Config {
        theme_colors: ThemeColorsConfig {
            auto_dark_mode: true,
            theme: "light-background".to_string(),
            dark_theme: "dracula".to_string(),
            ..Default::default()
        },
        ..Config::default()
    };

    assert!(config.apply_system_theme(true));
    assert_eq!(config.theme_colors.theme, "dracula");
}

#[test]
fn test_apply_system_theme_light() {
    let mut config = Config {
        theme_colors: ThemeColorsConfig {
            auto_dark_mode: true,
            theme: "dark-background".to_string(),
            light_theme: "solarized-light".to_string(),
            ..Default::default()
        },
        ..Config::default()
    };

    assert!(config.apply_system_theme(false));
    assert_eq!(config.theme_colors.theme, "solarized-light");
}

#[test]
fn test_apply_system_theme_no_change() {
    let mut config = Config {
        theme_colors: ThemeColorsConfig {
            auto_dark_mode: true,
            theme: "dark-background".to_string(),
            dark_theme: "dark-background".to_string(),
            ..Default::default()
        },
        ..Config::default()
    };

    // Already using the dark theme, should return false (no change)
    assert!(!config.apply_system_theme(true));
    assert_eq!(config.theme_colors.theme, "dark-background");
}

#[test]
fn test_auto_dark_mode_yaml_deserialization() {
    let yaml = r#"
auto_dark_mode: true
light_theme: solarized-light
dark_theme: dracula
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(config.theme_colors.auto_dark_mode);
    assert_eq!(config.theme_colors.light_theme, "solarized-light");
    assert_eq!(config.theme_colors.dark_theme, "dracula");
}

#[test]
fn test_auto_dark_mode_yaml_defaults_when_absent() {
    let yaml = "cols: 120\n";
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert!(!config.theme_colors.auto_dark_mode);
    assert_eq!(config.theme_colors.light_theme, "light-background");
    assert_eq!(config.theme_colors.dark_theme, "dark-background");
}

// =============================================================================
// Auto Tab Style Tests
// =============================================================================

#[test]
fn test_auto_tab_style_defaults() {
    let config = Config::default();
    assert_eq!(config.tabs.tab_style, TabStyle::Dark);
    assert_eq!(config.tabs.light_tab_style, TabStyle::Light);
    assert_eq!(config.tabs.dark_tab_style, TabStyle::Dark);
}

#[test]
fn test_apply_system_tab_style_disabled_when_not_automatic() {
    let mut config = Config {
        tabs: TabConfig {
            tab_style: TabStyle::Dark,
            ..Default::default()
        },
        ..Config::default()
    };
    assert!(!config.apply_system_tab_style(true));
    assert!(!config.apply_system_tab_style(false));
}

#[test]
fn test_apply_system_tab_style_dark() {
    let mut config = Config {
        tabs: TabConfig {
            tab_style: TabStyle::Automatic,
            dark_tab_style: TabStyle::HighContrast,
            ..Default::default()
        },
        ..Config::default()
    };

    assert!(config.apply_system_tab_style(true));
    // Should have applied HighContrast colors but kept Automatic as the tab_style
    assert_eq!(config.tabs.tab_style, TabStyle::Automatic);
    // HighContrast sets tab_bar_background to [0, 0, 0]
    assert_eq!(config.tab_colors.tab_bar_background, [0, 0, 0]);
}

#[test]
fn test_apply_system_tab_style_light() {
    let mut config = Config {
        tabs: TabConfig {
            tab_style: TabStyle::Automatic,
            light_tab_style: TabStyle::Light,
            ..Default::default()
        },
        ..Config::default()
    };

    assert!(config.apply_system_tab_style(false));
    assert_eq!(config.tabs.tab_style, TabStyle::Automatic);
    // Light sets tab_bar_background to [235, 235, 235]
    assert_eq!(config.tab_colors.tab_bar_background, [235, 235, 235]);
}

#[test]
fn test_apply_system_tab_style_preserves_automatic() {
    let mut config = Config {
        tabs: TabConfig {
            tab_style: TabStyle::Automatic,
            dark_tab_style: TabStyle::Compact,
            ..Default::default()
        },
        ..Config::default()
    };

    config.apply_system_tab_style(true);
    // tab_style must remain Automatic after applying
    assert_eq!(config.tabs.tab_style, TabStyle::Automatic);
}

#[test]
fn test_tab_style_all_concrete_excludes_automatic() {
    let concrete = TabStyle::all_concrete();
    assert!(!concrete.contains(&TabStyle::Automatic));
    assert_eq!(concrete.len(), 5);
}

#[test]
fn test_tab_style_all_includes_automatic() {
    let all = TabStyle::all();
    assert!(all.contains(&TabStyle::Automatic));
    assert_eq!(all.len(), 6);
}

#[test]
fn test_auto_tab_style_yaml_deserialization() {
    let yaml = r#"
tab_style: automatic
light_tab_style: compact
dark_tab_style: high_contrast
"#;
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.tab_style, TabStyle::Automatic);
    assert_eq!(config.tabs.light_tab_style, TabStyle::Compact);
    assert_eq!(config.tabs.dark_tab_style, TabStyle::HighContrast);
}

#[test]
fn test_auto_tab_style_yaml_defaults_when_absent() {
    let yaml = "cols: 120\n";
    let config: Config = serde_yaml_ng::from_str(yaml).unwrap();
    assert_eq!(config.tabs.light_tab_style, TabStyle::Light);
    assert_eq!(config.tabs.dark_tab_style, TabStyle::Dark);
}
