//! Environment variable allowlist and substitution for config file processing.
//!
//! Only allowlisted variables (and `PAR_TERM_*` / `LC_*` prefixed ones) are
//! resolved by default to prevent shared or downloaded config files from
//! exfiltrating sensitive environment variables.

use regex::Regex;
use std::sync::LazyLock;

/// Regex pattern for matching `${VAR_NAME}` or `${VAR_NAME:-default_value}` syntax.
/// Compiled once at startup using LazyLock to avoid recompiling on every substitution call.
static ENV_VAR_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-((?:[^}\\]|\\.)*))?}")
        .expect("env-var substitution regex is a compile-time constant and must be valid")
});

/// Regex pattern for detecting `allow_all_env_vars: true` at the top level of YAML.
/// Compiled once at startup using LazyLock to avoid recompiling on every pre-scan call.
static ALLOW_ALL_ENV_VARS_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^allow_all_env_vars:\s*true\s*$")
        .expect("allow_all_env_vars pre-scan regex is a compile-time constant and must be valid")
});

/// Environment variables that are safe to substitute in config files.
///
/// Only these variables (and any `PAR_TERM_*` prefixed variables) are resolved
/// by default. This prevents a shared/downloaded config from exfiltrating
/// sensitive environment variables (API keys, tokens, etc.) via `${SECRET_KEY}`.
///
/// Set `allow_all_env_vars: true` in config to bypass this restriction.
pub const ALLOWED_ENV_VARS: &[&str] = &[
    // User / home
    "HOME",
    "USER",
    "USERNAME",
    "LOGNAME",
    "USERPROFILE", // Windows
    // Shell / terminal
    "SHELL",
    "TERM",
    "LANG",
    "COLORTERM",
    "TERM_PROGRAM",
    // XDG directories
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_STATE_HOME",
    "XDG_CACHE_HOME",
    "XDG_RUNTIME_DIR",
    // System paths
    "PATH",
    "TMPDIR",
    "TEMP",
    "TMP",
    // Display
    "DISPLAY",
    "WAYLAND_DISPLAY",
    // Host
    "HOSTNAME",
    "HOST",
    // Editors
    "EDITOR",
    "VISUAL",
    "PAGER",
    // Windows paths
    "APPDATA",
    "LOCALAPPDATA",
];

/// Check whether a variable name is on the substitution allowlist.
///
/// A variable is allowed if it appears in [`ALLOWED_ENV_VARS`], starts with
/// `PAR_TERM_`, or starts with `LC_` (locale variables).
pub fn is_env_var_allowed(var_name: &str) -> bool {
    ALLOWED_ENV_VARS.contains(&var_name)
        || var_name.starts_with("PAR_TERM_")
        || var_name.starts_with("LC_")
}

/// Substitute `${VAR_NAME}` patterns in a string with environment variable values.
///
/// - `${VAR}` is replaced with the value of the environment variable `VAR`.
/// - If the variable is not set, the `${VAR}` placeholder is left unchanged.
/// - `$${VAR}` (doubled dollar sign) is an escape and produces the literal `${VAR}`.
/// - Supports `${VAR:-default}` syntax for providing a default value when the variable is unset.
///
/// Only variables on the allowlist (see [`ALLOWED_ENV_VARS`]) and those prefixed
/// with `PAR_TERM_` or `LC_` are substituted by default. Non-allowlisted
/// variables are left as-is and a warning is logged. Pass `allow_all = true` to
/// bypass the allowlist and resolve any environment variable.
///
/// This is applied to the raw YAML config string before deserialization, so all
/// string-typed config values benefit from substitution.
pub fn substitute_variables(input: &str) -> String {
    substitute_variables_with_allowlist(input, false)
}

/// Substitute variables with explicit allowlist control.
///
/// When `allow_all` is `true`, **every** environment variable is resolved
/// (the pre-M3 behaviour). When `false`, only allowlisted variables are
/// resolved and non-allowlisted references are left as literal text with a
/// warning logged.
pub fn substitute_variables_with_allowlist(input: &str, allow_all: bool) -> String {
    // First, replace escaped `$${` with a placeholder that won't match the regex
    let escaped_placeholder = "\x00ESC_DOLLAR\x00";
    let working = input.replace("$${", escaped_placeholder);

    // Use the pre-compiled static regex pattern
    let result = ENV_VAR_PATTERN.replace_all(&working, |caps: &regex::Captures| {
        let var_name = &caps[1];

        // Check allowlist unless the user opted into unrestricted mode
        if !allow_all && !is_env_var_allowed(var_name) {
            log::warn!(
                "Config references non-allowlisted environment variable: ${{{var_name}}} — skipped. \
                 Add `allow_all_env_vars: true` to your config to allow all variables."
            );
            return caps[0].to_string();
        }

        match std::env::var(var_name) {
            Ok(val) => val,
            Err(_) => {
                // Use default value if provided, otherwise leave the placeholder as-is
                caps.get(2)
                    .map(|m| m.as_str().replace("\\}", "}"))
                    .unwrap_or_else(|| caps[0].to_string())
            }
        }
    });

    // Restore escaped dollar signs
    result.replace(escaped_placeholder, "${")
}

/// Quick pre-scan of raw YAML text for the `allow_all_env_vars: true` setting.
///
/// This is intentionally simple: it looks for a top-level YAML key rather than
/// fully parsing the document, because we need the answer *before* variable
/// substitution runs (and therefore before serde deserialization).
pub(crate) fn pre_scan_allow_all_env_vars(raw_yaml: &str) -> bool {
    // Use the pre-compiled static regex pattern
    ALLOW_ALL_ENV_VARS_PATTERN.is_match(raw_yaml)
}
