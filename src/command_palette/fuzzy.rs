//! Fuzzy subsequence matching and ranking for the command palette.

/// Score of the best subsequence match of `query` within `candidate`.
///
/// Returns `None` when `query` is not a case-insensitive subsequence of
/// `candidate`. An empty query matches everything with score 0, so an unfiltered
/// palette preserves catalog order rather than ranking arbitrarily.
///
/// Scoring rewards a character contiguous with the previous match above a
/// match at a word boundary (start of string, or the character after a space
/// or underscore), which in turn beats a bare mid-word match. Contiguity must
/// outweigh the boundary bonus: a query whose letters land on the initials of
/// four unrelated words collects a boundary bonus per letter, and only a
/// heavier contiguity weight keeps a typed prefix like "togg" ranked above
/// that (measured: at parity weights the prefix scored 24 against 36).
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn score(query: &str, candidate: &str) -> Option<u32> {
    const BOUNDARY_BONUS: u32 = 8;
    const CONTIGUOUS_BONUS: u32 = 10;
    const MATCH_BASE: u32 = 1;

    if query.is_empty() {
        return Some(0);
    }

    let mut total = 0u32;
    let mut last_match_idx: Option<usize> = None;
    let candidate_chars: Vec<char> = candidate.chars().collect();
    let mut cursor = 0usize;

    for q in query.chars().flat_map(|c| c.to_lowercase()) {
        let found = candidate_chars[cursor..]
            .iter()
            .position(|c| c.to_lowercase().next() == Some(q))
            .map(|offset| cursor + offset)?;

        total += MATCH_BASE;

        let at_boundary =
            found == 0 || matches!(candidate_chars.get(found - 1), Some(' ') | Some('_'));
        if at_boundary {
            total += BOUNDARY_BONUS;
        }

        if last_match_idx == Some(found.wrapping_sub(1)) {
            total += CONTIGUOUS_BONUS;
        }

        last_match_idx = Some(found);
        cursor = found + 1;
    }

    Some(total)
}

/// Filter `items` to those whose display name matches `query`, best-first.
///
/// `items` is `(action_id, display_name)`; matching runs against the display
/// name only — the id is machine-facing and its underscores would make almost
/// any query match something. Ties keep input order, so a catalog ordered
/// deliberately stays that way under an empty query.
///
/// The element lifetime is deliberately decoupled from the slice borrow: a
/// caller matching over a short-lived scratch vec of `&'static` ids keeps
/// `'static` on the returned ids instead of inheriting the scratch vec's
/// scope.
#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn rank<'items, 'entry>(
    query: &str,
    items: &'items [(&'entry str, &'entry str)],
) -> Vec<&'items (&'entry str, &'entry str)> {
    let mut scored: Vec<(u32, usize, &(&str, &str))> = items
        .iter()
        .enumerate()
        .filter_map(|(idx, item)| score(query, item.1).map(|s| (s, idx, item)))
        .collect();

    // Sort by score descending, then by original index ascending for stability.
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored.into_iter().map(|(_, _, item)| item).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_query_matches_everything_with_equal_score() {
        assert_eq!(score("", "Toggle Fullscreen"), Some(0));
        assert_eq!(score("", "New Tab"), Some(0));
    }

    #[test]
    fn non_subsequence_does_not_match() {
        assert_eq!(score("zzz", "Toggle Fullscreen"), None);
        // Right letters, wrong order.
        assert_eq!(score("lgt", "Toggle"), None);
    }

    #[test]
    fn subsequence_matches_case_insensitively() {
        assert!(score("tf", "Toggle Fullscreen").is_some());
        assert!(score("TF", "toggle fullscreen").is_some());
    }

    #[test]
    fn word_initials_outrank_scattered_letters() {
        // "tf" as the initials of Toggle Fullscreen should beat "tf"
        // buried inside an unrelated label.
        let initials = score("tf", "Toggle Fullscreen").expect("matches");
        let scattered = score("tf", "Restore Default Font").expect("matches");
        assert!(
            initials > scattered,
            "initials {initials} should outrank scattered {scattered}"
        );
    }

    #[test]
    fn contiguous_run_outranks_gapped_match() {
        let contiguous = score("togg", "Toggle Search").expect("matches");
        let gapped = score("togg", "Terminal Options Get Going").expect("matches");
        assert!(
            contiguous > gapped,
            "contiguous {contiguous} should outrank gapped {gapped}"
        );
    }

    #[test]
    fn prefix_match_outranks_later_match() {
        let prefix = score("new", "New Tab").expect("matches");
        let later = score("new", "Open New Window").expect("matches");
        assert!(
            prefix > later,
            "prefix {prefix} should outrank later {later}"
        );
    }

    #[test]
    fn rank_filters_and_orders_best_first() {
        let items = [
            ("toggle_search", "Toggle Search"),
            ("new_tab", "New Tab"),
            ("quit", "Quit par-term"),
        ];
        let out = rank("ts", &items);
        assert_eq!(out.len(), 1, "only Toggle Search is a subsequence of 'ts'");
        assert_eq!(out[0].0, "toggle_search");
    }

    #[test]
    fn rank_is_stable_for_equal_scores() {
        let items = [("a_one", "Alpha"), ("a_two", "Alpha")];
        let out = rank("a", &items);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].0, "a_one", "equal scores keep input order");
        assert_eq!(out[1].0, "a_two");
    }
}
