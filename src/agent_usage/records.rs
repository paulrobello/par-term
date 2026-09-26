//! Record types for the agent-usage file contract.
//!
//! The wire shape is omarchy's collector output, pinned 2026-09-20 against
//! `omarchy/bin/omarchy-agent-usage-{claude,codex,fireworks}`: the top-level
//! identity/limits fields plus a stats block the collectors merge in flat
//! (`record.update(stats)`), so `todayPrompts`, `recentDays`, `modelUsage`
//! etc. sit at the top level alongside `schemaVersion`.
//!
//! Parsing is deliberately tolerant: unknown fields are ignored (serde's
//! default) and every field defaults when absent, so a collector gaining a
//! key — or an agent record from a newer schemaVersion — never takes the
//! panel down. `parse_record` returns `None` for anything that does not
//! parse at all.

// The v1 panel reads the display fields (limits, balance, daily and model
// rows, hero stats); the rest of the contract — today_* aggregates,
// active_dates (v2 merge input), auth-help fields surfaced only on
// not-ready records — is parsed but not yet rendered in non-test builds.
#![cfg_attr(not(test), allow(dead_code))]

use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// One rate-limit meter entry (e.g. "Session (5-hour)", "Weekly (7-day)").
///
/// The display name arrives as `label` on most collectors but `title` on
/// scoped limits — both spellings exist in the same omarchy collector — so
/// both are accepted.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageLimit {
    /// Window display name.
    #[serde(default, alias = "title")]
    pub label: Option<String>,
    /// Window utilization, 0–100. Absent when the collector could not read it.
    #[serde(default)]
    pub percent: Option<f64>,
    /// RFC3339 reset timestamp. Absent on balance-style meters.
    #[serde(default)]
    pub resets_at: Option<String>,
}

/// One day of recent usage. The wire key `messageCount` is a token total
/// despite its legacy name (the omarchy collectors say so in a comment), so
/// it is surfaced as `tokens`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DayUsage {
    /// Calendar date, `YYYY-MM-DD`.
    #[serde(default)]
    pub date: String,
    /// Token total for the day (wire key: `messageCount`).
    #[serde(default, rename = "messageCount")]
    pub tokens: f64,
}

/// Per-model token counts, the value side of the `modelUsage` map.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ModelUsage {
    /// Input tokens billed this window.
    #[serde(default)]
    pub input_tokens: f64,
    /// Output tokens billed this window.
    #[serde(default)]
    pub output_tokens: f64,
    /// Cache-read input tokens.
    #[serde(default)]
    pub cache_read_input_tokens: f64,
    /// Cache-creation input tokens.
    #[serde(default)]
    pub cache_creation_input_tokens: f64,
}

/// Prepaid balance (fireworks-style records carry this instead of limits).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct UsageBalance {
    /// Remaining funds.
    #[serde(default)]
    pub remaining: f64,
    /// Originally funded amount.
    #[serde(default)]
    pub funded: f64,
    /// Funds spent so far.
    #[serde(default)]
    pub spent: f64,
    /// Currency code, e.g. `USD`.
    #[serde(default)]
    pub currency: String,
    /// Whether the figures are an estimate rather than a live balance read.
    #[serde(default)]
    pub estimated: bool,
}

/// One agent's usage record, the content of a single `<agent-id>.json`.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentUsageRecord {
    /// Contract version; currently 1 everywhere.
    #[serde(default)]
    pub schema_version: u32,
    /// Agent identifier, e.g. `claude`. Also the record's filename stem.
    #[serde(default)]
    pub id: String,
    /// Display name, e.g. `Claude`.
    #[serde(default)]
    pub name: String,
    /// RFC3339 collector run time.
    #[serde(default)]
    pub updated_at: String,
    /// Whether there is anything to show (limits or local stats). Drives
    /// self-hiding.
    #[serde(default)]
    pub ready: bool,
    /// Whether local transcript stats contributed to this record.
    #[serde(default)]
    pub has_local_stats: bool,
    /// Subscription tier label, e.g. `Max 20x`.
    #[serde(default)]
    pub tier_label: Option<String>,
    /// One-line status replacing the plan line on auth failure.
    #[serde(default)]
    pub usage_status_text: Option<String>,
    /// Help text shown when the usage source is unreachable.
    #[serde(default)]
    pub auth_help_text: Option<String>,
    /// Rate-limit meters.
    #[serde(default)]
    pub limits: Vec<UsageLimit>,
    /// Prepaid balance, when the agent is prepaid rather than metered.
    #[serde(default)]
    pub balance: Option<UsageBalance>,
    /// Collector hint that a stale percentage may need a re-probe.
    #[serde(default)]
    pub retry_advised: bool,

    // ── Stats block (merged flat into the record by the collectors) ───────
    /// All-time prompt count.
    #[serde(default)]
    pub total_prompts: Option<u64>,
    /// All-time session count.
    #[serde(default)]
    pub total_sessions: Option<u64>,
    /// All-time distinct active days. `active_dates` travels alongside so a
    /// cross-device merge can union rather than sum.
    #[serde(default)]
    pub active_days: Option<u64>,
    /// The dates behind `active_days`.
    #[serde(default)]
    pub active_dates: Vec<String>,
    /// Prompts today.
    #[serde(default)]
    pub today_prompts: Option<u64>,
    /// Sessions today.
    #[serde(default)]
    pub today_sessions: Option<u64>,
    /// Token total today.
    #[serde(default)]
    pub today_total_tokens: Option<f64>,
    /// Today's tokens keyed by model name.
    #[serde(default)]
    pub today_tokens_by_model: HashMap<String, f64>,
    /// Recent days of usage, oldest first.
    #[serde(default)]
    pub recent_days: Vec<DayUsage>,
    /// Token counts keyed by model name.
    #[serde(default)]
    pub model_usage: HashMap<String, ModelUsage>,
}

/// Parse one record. `None` on any parse failure — a bad file is skipped,
/// never propagated as an error into the UI.
pub(crate) fn parse_record(json: &str) -> Option<AgentUsageRecord> {
    serde_json::from_str(json).ok()
}

/// Whether a record should render at all: it must have something to show and
/// not be hidden by the user's per-agent hide list. This is the self-hiding
/// rule from the design — a not-ready agent produces no widget and no tab.
pub(crate) fn should_display(record: &AgentUsageRecord, hidden: &HashSet<String>) -> bool {
    record.ready && !hidden.contains(&record.id)
}

/// Merge rule for `activeDays` across records: union the traveling
/// date lists, then take the widest of that union and either side's bare
/// count — a source that only knows a count still bounds the answer from
/// below, exactly as omarchy's `merge_stats` does.
pub(crate) fn union_active_days(a: &AgentUsageRecord, b: &AgentUsageRecord) -> u64 {
    let mut dates: HashSet<&str> = a.active_dates.iter().map(String::as_str).collect();
    dates.extend(b.active_dates.iter().map(String::as_str));
    let union_len = dates.len() as u64;
    union_len
        .max(a.active_days.unwrap_or(0))
        .max(b.active_days.unwrap_or(0))
}

/// Widest-value merge of two records for the same agent id — one account
/// synced from two machines. The fresher collector run (`updatedAt`) is the
/// base; every account-scoped field then widens, never sums: limits and
/// balance keep the better-informed side, [`union_active_days`] unions
/// activity by date, counts take the max, per-model maps take the per-key
/// max, and recent days union by date keeping each date's larger total.
pub(crate) fn merge_records(a: AgentUsageRecord, b: AgentUsageRecord) -> AgentUsageRecord {
    let days_union = union_active_days(&a, &b);
    // The fresher run is the base; the merge below only widens it.
    let (mut m, older) = if b.updated_at > a.updated_at {
        (b, a)
    } else {
        (a, b)
    };

    m.has_local_stats |= older.has_local_stats;
    m.ready |= older.ready;
    m.tier_label = m.tier_label.or(older.tier_label);
    m.usage_status_text = m.usage_status_text.or(older.usage_status_text);
    m.auth_help_text = m.auth_help_text.or(older.auth_help_text);
    m.total_prompts = m.total_prompts.max(older.total_prompts);
    m.total_sessions = m.total_sessions.max(older.total_sessions);
    m.today_prompts = m.today_prompts.max(older.today_prompts);
    m.today_sessions = m.today_sessions.max(older.today_sessions);
    // f64 is not Ord, so Option::max is unavailable; widen by comparison.
    m.today_total_tokens = match (m.today_total_tokens, older.today_total_tokens) {
        (Some(a), Some(b)) => Some(a.max(b)),
        (one, None) | (None, one) => one,
    };

    let mut dates: HashSet<String> = older.active_dates.iter().cloned().collect();
    dates.extend(m.active_dates.iter().cloned());
    if dates.is_empty() {
        // No traveling dates on either side: the bare counts still bound
        // each other (widest wins), but there is nothing to union.
        m.active_days = m.active_days.max(older.active_days);
    } else {
        m.active_days = Some(days_union);
        let mut sorted = dates.into_iter().collect::<Vec<_>>();
        sorted.sort();
        m.active_dates = sorted;
    }

    // Limits keep the side with more live readings — the fresher run can
    // lose a meter it failed to read, and the account still knows it.
    // Counted and settled before the map loops below partially move `older`.
    let live = |limits: &[UsageLimit]| limits.iter().filter(|l| l.percent.is_some()).count();
    if live(&older.limits) > live(&m.limits) {
        m.limits = older.limits;
    }
    m.balance = m.balance.or(older.balance);

    for (model, older_usage) in older.model_usage {
        let entry = m.model_usage.entry(model).or_default();
        entry.input_tokens = entry.input_tokens.max(older_usage.input_tokens);
        entry.output_tokens = entry.output_tokens.max(older_usage.output_tokens);
        entry.cache_read_input_tokens = entry
            .cache_read_input_tokens
            .max(older_usage.cache_read_input_tokens);
        entry.cache_creation_input_tokens = entry
            .cache_creation_input_tokens
            .max(older_usage.cache_creation_input_tokens);
    }
    for (model, older_tokens) in older.today_tokens_by_model {
        m.today_tokens_by_model
            .entry(model)
            .and_modify(|tokens| *tokens = (*tokens).max(older_tokens))
            .or_insert(older_tokens);
    }

    let mut by_date: std::collections::BTreeMap<String, f64> = older
        .recent_days
        .into_iter()
        .map(|day| (day.date, day.tokens))
        .collect();
    for day in m.recent_days.drain(..) {
        by_date
            .entry(day.date)
            .and_modify(|tokens| *tokens = (*tokens).max(day.tokens))
            .or_insert(day.tokens);
    }
    m.recent_days = by_date
        .into_iter()
        .map(|(date, tokens)| DayUsage { date, tokens })
        .collect();
    m
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    const FULL_RECORD: &str = r#"{
        "schemaVersion": 1,
        "id": "claude",
        "name": "Claude",
        "updatedAt": "2026-09-20T17:06:14Z",
        "ready": true,
        "hasLocalStats": true,
        "tierLabel": "Max 20x",
        "usageStatusText": "",
        "authHelpText": "",
        "limits": [
            {"label": "Session (5-hour)", "percent": 42.0, "resetsAt": "2026-09-20T19:00:00Z"},
            {"label": "Weekly (7-day)", "percent": 71.5, "resetsAt": "2026-09-22T00:00:00Z"}
        ],
        "totalPrompts": 1234,
        "totalSessions": 56,
        "activeDays": 87,
        "activeDates": ["2026-09-19", "2026-09-20"],
        "todayPrompts": 12,
        "todaySessions": 2,
        "todayTotalTokens": 8200000,
        "todayTokensByModel": {"opus-5": 4100000, "sonnet-5": 4100000},
        "recentDays": [
            {"date": "2026-09-19", "messageCount": 6100000},
            {"date": "2026-09-20", "messageCount": 8200000}
        ],
        "modelUsage": {
            "opus-5": {"inputTokens": 1000, "outputTokens": 2000,
                       "cacheReadInputTokens": 3000, "cacheCreationInputTokens": 4000}
        }
    }"#;

    #[test]
    fn full_record_parses_every_field() {
        let record = parse_record(FULL_RECORD).expect("full fixture must parse");
        assert_eq!(record.schema_version, 1);
        assert_eq!(record.id, "claude");
        assert_eq!(record.name, "Claude");
        assert_eq!(record.updated_at, "2026-09-20T17:06:14Z");
        assert!(record.ready && record.has_local_stats);
        assert_eq!(record.tier_label.as_deref(), Some("Max 20x"));
        assert_eq!(record.limits.len(), 2);
        assert_eq!(record.limits[0].label.as_deref(), Some("Session (5-hour)"));
        assert_eq!(record.limits[0].percent, Some(42.0));
        assert_eq!(
            record.limits[0].resets_at.as_deref(),
            Some("2026-09-20T19:00:00Z")
        );
        assert_eq!(record.total_prompts, Some(1234));
        assert_eq!(record.total_sessions, Some(56));
        assert_eq!(record.active_days, Some(87));
        assert_eq!(record.active_dates, vec!["2026-09-19", "2026-09-20"]);
        assert_eq!(record.today_prompts, Some(12));
        assert_eq!(record.today_sessions, Some(2));
        assert_eq!(record.today_total_tokens, Some(8_200_000.0));
        assert_eq!(
            record.today_tokens_by_model.get("opus-5"),
            Some(&4_100_000.0)
        );
        assert_eq!(record.recent_days.len(), 2);
        assert_eq!(record.recent_days[1].date, "2026-09-20");
        // The wire key messageCount is surfaced as tokens.
        assert_eq!(record.recent_days[1].tokens, 8_200_000.0);
        let opus = record
            .model_usage
            .get("opus-5")
            .expect("modelUsage key present");
        assert_eq!(opus.input_tokens, 1000.0);
        assert_eq!(opus.output_tokens, 2000.0);
        assert_eq!(opus.cache_read_input_tokens, 3000.0);
        assert_eq!(opus.cache_creation_input_tokens, 4000.0);
        assert!(record.balance.is_none());
        assert!(!record.retry_advised);
    }

    #[test]
    fn limit_display_name_accepts_the_title_spelling() {
        let record = parse_record(
            r#"{"id":"x","ready":true,"limits":[{"title":"Custom caps","percent":10}]}"#,
        )
        .expect("title alias must parse");
        assert_eq!(record.limits[0].label.as_deref(), Some("Custom caps"));
        assert_eq!(record.limits[0].percent, Some(10.0));
        assert_eq!(record.limits[0].resets_at, None);
    }

    #[test]
    fn prepaid_balance_parses() {
        let record = parse_record(
            r#"{"id":"fireworks","ready":true,
                "balance":{"remaining":12.5,"funded":50.0,"spent":37.5,
                           "currency":"USD","estimated":false}}"#,
        )
        .expect("balance fixture must parse");
        let balance = record.balance.expect("balance present");
        assert_eq!(balance.remaining, 12.5);
        assert_eq!(balance.funded, 50.0);
        assert_eq!(balance.spent, 37.5);
        assert_eq!(balance.currency, "USD");
        assert!(!balance.estimated);
    }

    #[test]
    fn auth_failure_shape_parses_and_does_not_display() {
        // Auth failure: not ready, a status line replacing the plan, no limits.
        let record = parse_record(
            r#"{"schemaVersion":1,"id":"claude","name":"Claude",
                "updatedAt":"2026-09-20T17:06:14Z","ready":false,
                "hasLocalStats":false,"tierLabel":null,
                "usageStatusText":"Claude limits unavailable",
                "authHelpText":"Run `claude login`","limits":[]}"#,
        )
        .expect("auth-failure fixture must parse");
        assert!(!record.ready);
        assert_eq!(
            record.usage_status_text.as_deref(),
            Some("Claude limits unavailable")
        );
        assert_eq!(record.auth_help_text.as_deref(), Some("Run `claude login`"));
        assert!(record.limits.is_empty());
        assert!(!should_display(&record, &HashSet::new()));
    }

    #[test]
    fn minimal_not_ready_record_parses_via_defaults() {
        let record = parse_record(r#"{"id":"new-agent"}"#).expect("only the id is guaranteed");
        assert_eq!(record.id, "new-agent");
        assert!(!record.ready);
        assert_eq!(record.schema_version, 0);
        assert!(record.limits.is_empty() && record.recent_days.is_empty());
        assert!(!should_display(&record, &HashSet::new()));
    }

    #[test]
    fn unknown_fields_are_ignored() {
        // Forward compatibility: collectors gain keys the panel does not know.
        let record =
            parse_record(r#"{"id":"x","ready":true,"weekTokens":{"opus":1},"futureThing":[1,2]}"#)
                .expect("unknown keys must not reject the record");
        assert!(record.ready);
    }

    #[test]
    fn garbage_json_returns_none() {
        assert!(parse_record("{not json").is_none());
        assert!(parse_record(r#"{"id":"truncated,"#).is_none());
        assert!(parse_record("").is_none());
    }

    #[test]
    fn hidden_agent_does_not_display() {
        let record = parse_record(FULL_RECORD).expect("fixture parses");
        assert!(should_display(&record, &HashSet::new()));
        let hidden: HashSet<String> = ["claude".to_string()].into();
        assert!(!should_display(&record, &hidden));
    }

    #[test]
    fn union_active_days_unions_dates_never_sums() {
        let a =
            parse_record(r#"{"id":"a","activeDays":2,"activeDates":["2026-09-18","2026-09-19"]}"#)
                .expect("a parses");
        let b =
            parse_record(r#"{"id":"b","activeDays":2,"activeDates":["2026-09-19","2026-09-20"]}"#)
                .expect("b parses");
        // One shared date: 3 distinct days, not 4.
        assert_eq!(union_active_days(&a, &b), 3);
        assert_eq!(union_active_days(&b, &a), 3);
    }

    #[test]
    fn union_active_days_bounds_below_by_bare_counts() {
        // A source with only a count (no traveling dates) still raises the
        // floor: max(union=1, a=5, b=0) = 5.
        let a = parse_record(r#"{"id":"a","activeDays":5,"activeDates":["2026-09-20"]}"#)
            .expect("a parses");
        let b = parse_record(r#"{"id":"b"}"#).expect("b parses");
        assert_eq!(union_active_days(&a, &b), 5);
    }

    /// The v2 account merge: one `claude` account synced from two machines.
    /// Every field widens, nothing sums, and the fresher run is the base.
    #[test]
    fn merge_records_unions_dates_and_widens_one_account_from_two_machines() {
        // Machine A: fresher run, one live limit, local stats, dates 18–19.
        let a = parse_record(
            r#"{"id":"claude","ready":true,"updatedAt":"2026-09-20T18:00:00Z",
                "limits":[{"label":"Weekly","percent":70.0}],
                "hasLocalStats":true,
                "activeDays":2,"activeDates":["2026-09-18","2026-09-19"],
                "totalPrompts":900,
                "recentDays":[{"date":"2026-09-19","messageCount":40}]}"#,
        )
        .expect("a parses");
        // Machine B: older run, a second live limit A's collector missed, a
        // balance A has none of, dates 19–20, and its own counts.
        let b = parse_record(
            r#"{"id":"claude","ready":true,"updatedAt":"2026-09-20T17:00:00Z",
                "limits":[
                    {"label":"Weekly","percent":70.0},
                    {"label":"Session (5-hour)","percent":12.0}
                ],
                "balance":{"remaining":12.5,"funded":50.0,"spent":37.5,
                            "currency":"USD","estimated":false},
                "activeDays":2,"activeDates":["2026-09-19","2026-09-20"],
                "totalPrompts":1200,
                "recentDays":[{"date":"2026-09-20","messageCount":80},
                               {"date":"2026-09-19","messageCount":55}]}"#,
        )
        .expect("b parses");

        let merged = merge_records(a, b);

        // Active days union by date (18, 19, 20 = 3), never summed (2+2).
        assert_eq!(merged.active_days, Some(3));
        assert_eq!(
            merged.active_dates,
            vec!["2026-09-18", "2026-09-19", "2026-09-20"]
        );
        // Counts widen, never sum.
        assert_eq!(merged.total_prompts, Some(1200));
        // The fresher run is the base; the older run's extra live limit and
        // balance widen it.
        assert_eq!(merged.limits.len(), 2, "limits keep the wider set");
        assert_eq!(merged.balance.as_ref().map(|b| b.remaining), Some(12.5));
        assert!(merged.has_local_stats);
        // Recent days union by date, keeping each date's larger total.
        let days: Vec<(&str, f64)> = merged
            .recent_days
            .iter()
            .map(|d| (d.date.as_str(), d.tokens))
            .collect();
        assert_eq!(
            days,
            vec![("2026-09-19", 55.0), ("2026-09-20", 80.0)],
            "19 keeps machine B's larger total; 20 arrives from B"
        );
    }

    #[test]
    fn merge_records_fresher_base_wins_ties_and_bare_counts_bound() {
        // Neither side carries dates: bare counts bound each other instead.
        let a = parse_record(
            r#"{"id":"claude","updatedAt":"2026-09-20T18:00:00Z",
                "activeDays":40,"tierLabel":"Max 20x"}"#,
        )
        .expect("a parses");
        let b = parse_record(r#"{"id":"claude","updatedAt":"2026-09-20T17:00:00Z"}"#)
            .expect("b parses");
        let merged = merge_records(a, b);
        assert_eq!(merged.active_days, Some(40));
        // The fresher run is the base, so its tier label survives; the older
        // side's absent label adds nothing.
        assert_eq!(merged.tier_label.as_deref(), Some("Max 20x"));

        // Merge order must not matter: swap the arguments, same outcome.
        let a =
            parse_record(r#"{"id":"claude","updatedAt":"2026-09-20T18:00:00Z","activeDays":40}"#)
                .expect("a parses");
        let b =
            parse_record(r#"{"id":"claude","updatedAt":"2026-09-20T17:00:00Z","activeDays":40}"#)
                .expect("b parses");
        assert_eq!(
            merge_records(a.clone(), b.clone()).active_days,
            merge_records(b, a).active_days
        );
    }
}
