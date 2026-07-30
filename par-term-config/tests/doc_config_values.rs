//! Gate: every enum value documented in `docs/CONFIG_REFERENCE.md` must be
//! accepted by the type it documents.
//!
//! Four documented enum value-sets were wrong in ways that made a config fail to
//! load outright, and two more silently deserialized to the default. The pattern
//! is always the same: **the serde attribute, not the variant name, decides the
//! wire value**, and a human transcribing `BarWithText` writes `bar_with_text`
//! when the type accepts `barwithtext`.
//!
//! # How the gate works
//!
//! For every table row whose type column is `enum`:
//!
//! 1. **Establish the key is real.** Deserialize the key with a sentinel string
//!    no enum accepts. It must be *rejected*. Neither `Config` nor `Profile`
//!    sets `deny_unknown_fields`, so a misspelled or relocated key is silently
//!    ignored — without this probe every value for that key would "pass" while
//!    checking nothing. Rejection of the sentinel proves the field exists and
//!    is not a free-form string; that the field is an *enum* comes from the
//!    doc's own type column, which is the half the doc can be trusted for.
//! 2. **Deserialize each documented value.** Rejection is a hard failure. This
//!    is the class that made a real `config.yaml` fail to load.
//! 3. **Re-serialize and compare.** A value that parses but comes back spelled
//!    differently landed somewhere other than the doc claims.
//!
//! Documents are built by splicing the value into the type's serialized default,
//! so required fields are present and the probe reflects what a real config file
//! does. The token is spliced as parsed YAML, which keeps tag syntax
//! (`!custom /path`) intact rather than quoting it into a plain string.
//!
//! # Two types, one table format
//!
//! `CONFIG_REFERENCE.md` documents `config.yaml` (→ [`Config`]) and the
//! per-profile fields of `profiles.yaml` (→ [`Profile`]) in identical tables.
//! The target is resolved per key by probing `Config` first, then `Profile`.
//!
//! # What this does not cover
//!
//! - Only rows whose type column is exactly `enum`. Free-form strings, paths,
//!   numeric ranges and array shapes are out of scope — probing them generates
//!   noise without catching the defect class this exists for.
//! - Only keys settable at the top level of their file. Every documented enum
//!   key is, because `Config`'s sub-structs are `#[serde(flatten)]`ed; a key
//!   that stopped being reachable that way fails step 1 loudly, which is the
//!   correct outcome since the doc promises a top-level key.
//! - `docs/API.md` is not read at all.
//! - Round-trip comparison is skipped for fields whose `skip_serializing_if`
//!   omits them at their default value; those keys are listed by
//!   `round_trip_coverage_is_what_we_think_it_is` so the gap stays visible.

use par_term_config::{Config, Profile};
use serde_yaml_ng::{Mapping, Value};
use std::path::PathBuf;

// ---------------------------------------------------------------------------
// Expectations that keep the extractor honest
// ---------------------------------------------------------------------------

/// The extractor must keep finding at least this many enum rows. A parser that
/// silently matches nothing passes every other test in this file vacuously, so
/// a drop below this number fails loudly instead.
const MIN_ENUM_ROWS: usize = 38;

/// The extractor must also keep finding at least this many *values*. Rows and
/// values need separate floors: every row yields its default column for free,
/// so a `values_from_description` regression that returns nothing would leave
/// `rows.len()` at 38 and still clear a row-count floor, while silently
/// dropping every value the description lists. The live count is 131.
const MIN_ENUM_VALUES: usize = 125;

/// A value no enum variant can be named. Its *rejection* is what proves a
/// documented key exists and does not accept arbitrary strings.
const SENTINEL: &str = "__par_term_doc_gate_sentinel__";

/// Documented tokens that legitimately do not round-trip to themselves, with
/// the reason (serde aliases, canonical spellings). Every entry is a deliberate
/// exception and belongs in review. Empty on purpose — nothing has needed one.
const ROUND_TRIP_EXCEPTIONS: &[(&str, &str, &str)] = &[];

/// Keys whose value cannot be observed after a round trip because the field
/// carries `skip_serializing_if` and the documented value is the default.
/// Listed explicitly so the coverage gap is visible rather than silent.
const ROUND_TRIP_NOT_OBSERVABLE: &[&str] = &["tmux_connection_mode"];

// ---------------------------------------------------------------------------
// Doc loading and table extraction
// ---------------------------------------------------------------------------

fn config_reference_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("par-term-config lives one level below the repo root")
        .join("docs/CONFIG_REFERENCE.md")
}

fn read_config_reference() -> String {
    let path = config_reference_path();
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()))
}

/// One `| key | enum | default | description |` row.
#[derive(Debug)]
struct EnumRow {
    line_number: usize,
    key: String,
    /// Default column plus description tokens, in documentation order.
    values: Vec<String>,
}

/// Strip a leading and trailing backtick pair, if present.
fn unbacktick(token: &str) -> Option<&str> {
    token
        .strip_prefix('`')
        .and_then(|rest| rest.strip_suffix('`'))
}

/// Collect every backticked token in `text`.
fn backticked_tokens(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('`') {
        let after = &rest[open + 1..];
        let Some(close) = after.find('`') else { break };
        let token = &after[..close];
        if !token.is_empty() {
            tokens.push(token.to_string());
        }
        rest = &after[close + 1..];
    }
    tokens
}

/// Remove parenthesised groups. They hold prose annotations — `(theme color)`,
/// ``(uppercase — this enum has no `rename_all`)`` — whose backticks are not
/// documented values.
fn strip_parentheticals(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0usize;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

/// Pull the value list out of a description cell.
///
/// Descriptions come in two shapes: a bare list (`` `top`, `bottom` ``) and a
/// labelled one (``Tab bar position: `top`, `bottom` ``). Splitting on the first
/// colon that survives parenthetical-stripping handles both, and dropping the
/// parentheticals first is what keeps `` (`host:~/cwd`) `` from being mistaken
/// for the label separator.
fn values_from_description(description: &str) -> Vec<String> {
    let cleaned = strip_parentheticals(description);
    let list = match cleaned.find(':') {
        Some(colon) => &cleaned[colon + 1..],
        None => &cleaned[..],
    };
    backticked_tokens(list)
}

fn parse_enum_rows(markdown: &str) -> Vec<EnumRow> {
    let mut rows = Vec::new();

    for (index, line) in markdown.lines().enumerate() {
        let line = line.trim();
        if !line.starts_with('|') {
            continue;
        }

        // Cells between the outer pipes. One row has a stray trailing `| |`,
        // tolerated by only ever indexing the first four.
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        if cells.len() < 4 {
            continue;
        }

        let (Some(key), Some(kind)) = (unbacktick(cells[0]), unbacktick(cells[1])) else {
            continue;
        };
        if kind != "enum" {
            continue;
        }

        // The default column is a documented value too, and for the two rows
        // that defer their value set to another row it is the only one.
        let mut values = Vec::new();
        if let Some(default) = unbacktick(cells[2]) {
            values.push(default.to_string());
        }
        for value in values_from_description(cells[3]) {
            if !values.contains(&value) {
                values.push(value);
            }
        }

        rows.push(EnumRow {
            line_number: index + 1,
            key: key.to_string(),
            values,
        });
    }

    rows
}

// ---------------------------------------------------------------------------
// Deserializer probing
// ---------------------------------------------------------------------------

/// Which file, and therefore which type, a documented key belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Config,
    Profile,
}

impl Target {
    const ALL: [Target; 2] = [Target::Config, Target::Profile];

    fn type_name(self) -> &'static str {
        match self {
            Target::Config => "Config",
            Target::Profile => "Profile",
        }
    }

    /// The type's serialized default, as a mapping to splice into.
    ///
    /// Starting from a complete default means required fields (`Profile` has
    /// `id` and `name`) are present, so a rejection is always about the value
    /// under test rather than missing scaffolding.
    fn default_document(self) -> Mapping {
        let value = match self {
            Target::Config => serde_yaml_ng::to_value(Config::default()),
            Target::Profile => serde_yaml_ng::to_value(Profile::default()),
        }
        .unwrap_or_else(|error| panic!("{}::default() must serialize: {error}", self.type_name()));

        match value {
            Value::Mapping(mapping) => mapping,
            other => panic!(
                "{} serializes to {other:?}, not a mapping",
                self.type_name()
            ),
        }
    }

    /// Deserialize `{key: value}` spliced into the default document, then
    /// serialize the result back.
    fn round_trip(self, key: &str, raw_value: &str) -> Result<Value, String> {
        // Parse the token as YAML so tag syntax (`!custom /path`) survives
        // instead of being quoted into a plain string.
        let parsed: Value = serde_yaml_ng::from_str(raw_value)
            .map_err(|error| format!("`{raw_value}` is not valid YAML: {error}"))?;

        let mut document = self.default_document();
        document.insert(Value::String(key.to_string()), parsed);
        let yaml = serde_yaml_ng::to_string(&Value::Mapping(document))
            .map_err(|error| format!("cannot render probe document: {error}"))?;

        match self {
            Target::Config => serde_yaml_ng::from_str::<Config>(&yaml)
                .map_err(|error| format!("rejected by Config: {error}"))
                .and_then(|parsed| {
                    serde_yaml_ng::to_value(&parsed).map_err(|e| format!("re-serialize: {e}"))
                }),
            Target::Profile => serde_yaml_ng::from_str::<Profile>(&yaml)
                .map_err(|error| format!("rejected by Profile: {error}"))
                .and_then(|parsed| {
                    serde_yaml_ng::to_value(&parsed).map_err(|e| format!("re-serialize: {e}"))
                }),
        }
    }
}

/// Resolve which type documents `key`, by finding the one that *rejects* the
/// sentinel. An unknown key is silently ignored by both, so acceptance of the
/// sentinel means "this key is not a real enum field here".
fn resolve_target(key: &str) -> Result<Target, String> {
    for target in Target::ALL {
        if target.round_trip(key, SENTINEL).is_err() {
            return Ok(target);
        }
    }
    Err(format!(
        "`{key}` is not an enum-typed field of Config or Profile — both accept \
         the sentinel value, which means the key is being silently ignored"
    ))
}

/// What happened to one documented value.
#[derive(Debug)]
enum Accepted {
    /// The value round-tripped; this is the canonical spelling it came back as.
    Canonical(String),
    /// The field is omitted by `skip_serializing_if` at this value, so the
    /// spelling cannot be observed. Acceptance was still verified.
    NotObservable,
}

fn validate(target: Target, key: &str, value: &str) -> Result<Accepted, String> {
    let serialized = target.round_trip(key, value)?;
    match serialized.get(Value::String(key.to_string())) {
        None => Ok(Accepted::NotObservable),
        Some(landed) => {
            let token = serde_yaml_ng::to_string(landed)
                .map_err(|error| format!("token serialization failed: {error}"))?;
            Ok(Accepted::Canonical(token.trim().to_string()))
        }
    }
}

/// Resolve the target and validate in one step, for the negative tests.
fn validate_anywhere(key: &str, value: &str) -> Result<Accepted, String> {
    validate(resolve_target(key)?, key, value)
}

// ---------------------------------------------------------------------------
// The gate
// ---------------------------------------------------------------------------

#[test]
fn extractor_still_matches_the_documented_enum_rows() {
    let rows = parse_enum_rows(&read_config_reference());

    assert!(
        rows.len() >= MIN_ENUM_ROWS,
        "extractor matched {} enum rows, expected at least {MIN_ENUM_ROWS} — \
         did the CONFIG_REFERENCE.md table format change? A parser that matches \
         nothing passes every other test in this file vacuously.",
        rows.len()
    );

    let empty: Vec<String> = rows
        .iter()
        .filter(|row| row.values.is_empty())
        .map(|row| format!("CONFIG_REFERENCE.md:{} `{}`", row.line_number, row.key))
        .collect();
    assert!(
        empty.is_empty(),
        "these enum rows yielded no candidate values, so nothing about them is \
         checked:\n{}",
        empty.join("\n")
    );
}

#[test]
fn every_documented_enum_key_is_a_real_enum_field() {
    let mut failures: Vec<String> = Vec::new();

    for row in parse_enum_rows(&read_config_reference()) {
        if let Err(reason) = resolve_target(&row.key) {
            failures.push(format!(
                "CONFIG_REFERENCE.md:{} — {reason}",
                row.line_number
            ));
        }
    }

    assert!(
        failures.is_empty(),
        "documented enum keys that no type actually has. Neither Config nor \
         Profile sets deny_unknown_fields, so a real config file would ignore \
         these silently:\n{}",
        failures.join("\n")
    );
}

#[test]
fn every_documented_enum_value_is_accepted_by_its_type() {
    let mut failures: Vec<String> = Vec::new();
    let mut checked = 0usize;

    for row in parse_enum_rows(&read_config_reference()) {
        let Ok(target) = resolve_target(&row.key) else {
            // Reported by every_documented_enum_key_is_a_real_enum_field.
            continue;
        };
        for value in &row.values {
            checked += 1;
            if let Err(reason) = validate(target, &row.key, value) {
                failures.push(format!(
                    "CONFIG_REFERENCE.md:{} — `{}: {}` {reason}",
                    row.line_number, row.key, value
                ));
            }
        }
    }

    assert!(
        checked >= MIN_ENUM_VALUES,
        "only {checked} values checked, expected at least {MIN_ENUM_VALUES} — \
         the description parser has regressed and rows are being validated on \
         their default column alone"
    );
    assert!(
        failures.is_empty(),
        "{} documented value(s) the deserializer rejects. The serde attribute, \
         not the variant name, decides the wire value — fix the documentation, \
         not the type:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn every_documented_enum_value_round_trips_to_itself() {
    let mut drifted: Vec<String> = Vec::new();

    for row in parse_enum_rows(&read_config_reference()) {
        let Ok(target) = resolve_target(&row.key) else {
            continue;
        };
        for value in &row.values {
            let Ok(Accepted::Canonical(canonical)) = validate(target, &row.key, value) else {
                // Rejections and unobservable fields are covered by the two
                // tests around this one.
                continue;
            };
            if canonical == *value {
                continue;
            }
            let excused = ROUND_TRIP_EXCEPTIONS
                .iter()
                .any(|(key, token, _)| *key == row.key && *token == value.as_str());
            if !excused {
                drifted.push(format!(
                    "CONFIG_REFERENCE.md:{} — `{}: {}` parsed but serializes back as `{}`",
                    row.line_number, row.key, value, canonical
                ));
            }
        }
    }

    assert!(
        drifted.is_empty(),
        "{} documented value(s) did not survive a round trip. A value that \
         parses into something else is the 'silently deserializes to the \
         default' failure mode — the config loads and the setting is ignored. \
         If a token is a legitimate serde alias, add it to \
         ROUND_TRIP_EXCEPTIONS with a reason:\n{}",
        drifted.len(),
        drifted.join("\n")
    );
}

#[test]
fn round_trip_coverage_is_what_we_think_it_is() {
    // Fields with `skip_serializing_if` disappear at their default value, so
    // their spelling cannot be checked after a round trip — acceptance is still
    // verified, but canonicalization is not. Pin the list so the gap stays
    // visible and a new one has to be acknowledged rather than absorbed.
    let mut unobservable: Vec<String> = Vec::new();

    for row in parse_enum_rows(&read_config_reference()) {
        let Ok(target) = resolve_target(&row.key) else {
            continue;
        };
        let any_unobservable = row.values.iter().any(|value| {
            matches!(
                validate(target, &row.key, value),
                Ok(Accepted::NotObservable)
            )
        });
        if any_unobservable && !unobservable.contains(&row.key) {
            unobservable.push(row.key.clone());
        }
    }

    unobservable.sort();
    let mut expected: Vec<String> = ROUND_TRIP_NOT_OBSERVABLE
        .iter()
        .map(|key| (*key).to_string())
        .collect();
    expected.sort();

    assert_eq!(
        unobservable, expected,
        "the set of keys whose canonical spelling cannot be observed changed. \
         Update ROUND_TRIP_NOT_OBSERVABLE, and check whether the new key's \
         skip_serializing_if is intentional."
    );
}

// ---------------------------------------------------------------------------
// Negative tests — proof the gate catches the defects that motivated it
// ---------------------------------------------------------------------------

#[test]
fn the_gate_rejects_the_historical_documentation_defects() {
    // These four spellings were in CONFIG_REFERENCE.md until they were
    // corrected, and each made a real config fail to load. A gate that would
    // not have caught them manufactures confidence, so they are asserted here
    // directly rather than by mutating the doc file — which is also why this
    // test keeps working no matter how the doc is later reorganized.
    let historical_defects: &[(&str, &str, &str)] = &[
        (
            "normalization_form",
            "nfc",
            "types/unicode.rs has no rename_all and only `None` is renamed, so \
             the wire values are NFC/NFD/NFKC/NFKD",
        ),
        (
            "unicode_version",
            "unicode_9",
            "rename_all = \"snake_case\" inserts no underscore before a digit, \
             so the value is `unicode9`",
        ),
        (
            "progress_bar_style",
            "bar_with_text",
            "rename_all = \"lowercase\" on BarWithText gives `barwithtext`",
        ),
        (
            "download_save_location",
            "custom",
            "Custom(String) is a newtype variant needing YAML tag syntax \
             `!custom /path`, not a bare scalar",
        ),
    ];

    for (key, bad_value, why) in historical_defects {
        let outcome = validate_anywhere(key, bad_value);
        assert!(
            outcome.is_err(),
            "`{key}: {bad_value}` was accepted, but it must not be — {why}. \
             The gate would not have caught the original defect.\n\
             Result: {outcome:?}"
        );
    }
}

#[test]
fn the_gate_accepts_the_corrected_spellings() {
    // The other half of the previous test: the corrections must actually work,
    // otherwise "rejects everything" would pass it.
    for (key, good_value) in [
        ("normalization_form", "NFC"),
        ("unicode_version", "unicode9"),
        ("unicode_version", "unicode15_1"),
        ("progress_bar_style", "barwithtext"),
        ("download_save_location", "!custom /tmp/downloads"),
    ] {
        let outcome = validate_anywhere(key, good_value);
        assert!(
            outcome.is_ok(),
            "`{key}: {good_value}` is the corrected spelling and must be \
             accepted, got {outcome:?}"
        );
    }
}

#[test]
fn the_gate_rejects_values_no_enum_accepts() {
    // Guards against a validator that returns Ok unconditionally.
    for (key, nonsense) in [
        ("cursor_style", "trapezoid"),
        ("tab_bar_position", "diagonal"),
        ("log_level", "screaming"),
        ("vsync_mode", "eventually"),
        ("tmux_connection_mode", "telepathy"),
    ] {
        assert!(
            validate_anywhere(key, nonsense).is_err(),
            "`{key}: {nonsense}` must be rejected"
        );
    }
}

#[test]
fn the_key_probe_rejects_keys_that_do_not_exist() {
    // The false-green this gate exists to prevent.
    assert!(
        resolve_target("cursor_styel").is_err(),
        "a misspelled key must fail the probe, not silently validate nothing"
    );
    assert!(
        resolve_target("font_family").is_err(),
        "a free-form string field must fail the probe — it accepts any value, \
         so routing an `enum` row to it would validate nothing"
    );

    // The probe proves "this key exists and does not accept arbitrary strings".
    // That is not the same as "this key is an enum" — a numeric field rejects
    // the sentinel too. Enum-ness comes from the doc's own type column, which
    // is the only thing `parse_enum_rows` accepts; the probe supplies the half
    // the doc cannot be trusted for.
    assert!(
        resolve_target("font_size").is_ok(),
        "a numeric field also rejects the sentinel — documented so nobody \
         mistakes this probe for an enum-ness test"
    );

    // Prove the underlying hazard is real, so the probe cannot later be dropped
    // as redundant: the same document deserializes without complaint.
    let ignored: Result<Config, _> = serde_yaml_ng::from_str("cursor_styel: block\n");
    assert!(
        ignored.is_ok(),
        "Config silently ignores unknown keys — this is why resolve_target exists"
    );
}

#[test]
fn profile_only_keys_resolve_to_profile() {
    // `tmux_connection_mode` lives on Profile, not Config. Validating it
    // against Config would report a correct doc row as broken; this pins the
    // routing that prevents that.
    assert_eq!(
        resolve_target("tmux_connection_mode").expect("documented on Profile"),
        Target::Profile
    );
    assert_eq!(
        resolve_target("cursor_style").expect("documented on Config"),
        Target::Config
    );
}

// ---------------------------------------------------------------------------
// Extractor unit tests
// ---------------------------------------------------------------------------

#[test]
fn description_parser_handles_the_awkward_row_shapes() {
    // Bare list, no label.
    assert_eq!(
        values_from_description("`top`, `bottom`, `left`"),
        ["top", "bottom", "left"]
    );

    // Labelled list.
    assert_eq!(
        values_from_description("Cursor shape: `block`, `beam`, `underline`"),
        ["block", "beam", "underline"]
    );

    // Parenthetical annotations must not contribute tokens, and a colon inside
    // one must not be mistaken for the label separator.
    assert_eq!(
        values_from_description(
            "Tab title format when shell integration detects a remote host: \
             `user_at_host` (`user@host`), `host` (hostname only), \
             `host_and_cwd` (`host:~/cwd`)"
        ),
        ["user_at_host", "host", "host_and_cwd"]
    );
    assert_eq!(
        values_from_description(
            "Unicode normalization: `NFC`, `NFD`, `NFKC`, `NFKD`, `none` \
             (uppercase — this enum has no `rename_all`)"
        ),
        ["NFC", "NFD", "NFKC", "NFKD", "none"]
    );

    // A YAML tag value keeps its `!` and its argument.
    assert_eq!(
        values_from_description(
            "Default save location for downloaded files: `downloads`, \
             `last_used`, `cwd`, or `!custom /path/to/dir` (a YAML tag, not a mapping)"
        ),
        ["downloads", "last_used", "cwd", "!custom /path/to/dir"]
    );

    // A row that defers its value set to another row yields nothing here; the
    // default column is what makes it checkable.
    assert!(
        values_from_description("Tab style for system light mode (when `tab_style: automatic`)")
            .is_empty()
    );
}

#[test]
fn row_parser_skips_non_enum_rows_and_keeps_the_default_column() {
    let markdown = "\
| `cols` | `usize` | `80` | Number of terminal columns |
| `cursor_style` | `enum` | `block` | Cursor shape: `block`, `beam` |
| `light_tab_style` | `enum` | `light` | Tab style for light mode (when `tab_style: automatic`) |
| `font_family` | `string` | `\"JetBrains Mono\"` | Regular font family |
";
    let rows = parse_enum_rows(markdown);
    assert_eq!(rows.len(), 2);

    assert_eq!(rows[0].key, "cursor_style");
    // The default is not duplicated when the description repeats it.
    assert_eq!(rows[0].values, ["block", "beam"]);

    assert_eq!(rows[1].key, "light_tab_style");
    assert_eq!(rows[1].values, ["light"]);
}
