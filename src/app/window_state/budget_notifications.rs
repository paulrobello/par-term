//! Budget-refusal notifications (ENH-062): the core's unified
//! untrusted-output memory budget emits `OutputBudgetExceeded` when refused
//! output would have been retained; this module turns those events into
//! rate-limited notifications instead of silent drops. Split from
//! `notifications.rs` to keep it under the 800-line gate.

use crate::tab::TabId;
use par_term_emu_core_rust::graphics::BudgetStore;
use par_term_terminal::BudgetRefusal;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Minimum spacing between visible budget-refusal notifications per (tab,
/// store) key — the flood gate for refused inline images and transfers
/// (ENH-062 `OutputBudgetExceeded` events). Repeats inside the window are
/// logged but not re-notified.
const BUDGET_REFUSAL_WINDOW: Duration = Duration::from_secs(10);

static BUDGET_REFUSAL_LAST: OnceLock<Mutex<HashMap<(TabId, u8), Instant>>> = OnceLock::new();

/// Flood gate: whether a budget-refusal notification for `key` may be shown
/// now. The first refusal passes and stamps the key; repeats inside
/// [`BUDGET_REFUSAL_WINDOW`] are suppressed.
pub(super) fn budget_refusal_allowed(key: (TabId, u8), now: Instant) -> bool {
    let last = BUDGET_REFUSAL_LAST.get_or_init(|| Mutex::new(HashMap::new()));
    let mut last = last.lock().unwrap_or_else(|e| e.into_inner());
    match last.get(&key) {
        Some(t) if now.saturating_duration_since(*t) < BUDGET_REFUSAL_WINDOW => false,
        _ => {
            last.insert(key, now);
            true
        }
    }
}

/// Stable ordinal for a [`BudgetStore`], the per-store half of the
/// budget-refusal flood-gate key (BudgetStore carries no Hash derive).
pub(super) fn budget_store_ordinal(store: BudgetStore) -> u8 {
    match store {
        BudgetStore::Graphics => 0,
        BudgetStore::Multipart => 1,
        BudgetStore::FileTransfer => 2,
    }
}

/// The (title, message) a budget refusal surfaces as, so the user text is
/// pinned by test independent of the delivery path.
pub(super) fn budget_refusal_message(tab_id: TabId, refusal: &BudgetRefusal) -> (String, String) {
    let store_label = match refusal.store {
        BudgetStore::Graphics => "graphics",
        BudgetStore::Multipart => "transfer",
        BudgetStore::FileTransfer => "file transfer",
    };
    (
        "Inline output refused".to_string(),
        format!(
            "{store_label} allocation of {} refused: untrusted-output memory budget exceeded (tab {tab_id})",
            format_bytes_for_notification(refusal.bytes)
        ),
    )
}

/// Human-readable size for a budget-refusal notification message.
fn format_bytes_for_notification(bytes: usize) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let b = bytes as f64;
    if b >= GIB {
        format!("{:.1} GiB", b / GIB)
    } else if b >= MIB {
        format!("{:.1} MiB", b / MIB)
    } else if b >= KIB {
        format!("{:.1} KiB", b / KIB)
    } else {
        format!("{bytes} B")
    }
}

#[cfg(test)]
mod tests {
    use super::{budget_refusal_allowed, budget_refusal_message, budget_store_ordinal};
    use crate::tab::TabId;
    use par_term_emu_core_rust::graphics::BudgetStore;
    use par_term_terminal::BudgetRefusal;
    use std::time::{Duration, Instant};

    /// A flood of refusals for the same tab+store collapses to one visible
    /// notification per window; other stores and other tabs stay independent.
    #[test]
    fn repeated_refusals_rate_limit_to_one_per_window() {
        let tab: TabId = 7;
        let now = Instant::now();

        // First refusal passes; repeats inside the window are suppressed.
        assert!(budget_refusal_allowed(
            (tab, budget_store_ordinal(BudgetStore::Graphics)),
            now
        ));
        assert!(!budget_refusal_allowed(
            (tab, budget_store_ordinal(BudgetStore::Graphics)),
            now + Duration::from_secs(1)
        ));
        assert!(!budget_refusal_allowed(
            (tab, budget_store_ordinal(BudgetStore::Graphics)),
            now + Duration::from_secs(9)
        ));
        assert!(budget_refusal_allowed(
            (tab, budget_store_ordinal(BudgetStore::Graphics)),
            now + Duration::from_secs(10)
        ));

        // Other stores and other tabs are independent keys.
        assert!(budget_refusal_allowed(
            (tab, budget_store_ordinal(BudgetStore::Multipart)),
            now + Duration::new(1, 0)
        ));
        assert!(budget_refusal_allowed(
            (tab + 1, budget_store_ordinal(BudgetStore::Graphics)),
            now + Duration::from_secs(1)
        ));
    }

    /// The refusal's user-facing text names the store and the byte size.
    #[test]
    fn budget_refusal_message_names_store_and_bytes() {
        let refusal = BudgetRefusal {
            store: BudgetStore::Graphics,
            bytes: 300 * 1024 * 1024,
        };
        let (title, message) = budget_refusal_message(9, &refusal);
        assert_eq!(title, "Inline output refused");
        assert!(
            message.contains("graphics"),
            "store name expected in: {message}"
        );
        assert!(
            message.contains("300.0 MiB"),
            "human-readable size expected in: {message}"
        );
    }
}
