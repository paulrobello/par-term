//! Side-by-side diff rendering helpers.

use super::diff_parser::DiffLine;

/// A row in side-by-side mode with left (old) and right (new) columns.
pub(super) struct SbsRow {
    pub left: SbsCell,
    pub right: SbsCell,
}

/// A cell in a side-by-side row.
pub(super) enum SbsCell {
    Context(usize, String),
    Removed(usize, String),
    Empty,
}

/// Build side-by-side rows from hunk lines.
pub(super) fn build_side_by_side_rows(
    hunk_lines: &[DiffLine],
    old_start: usize,
    new_start: usize,
) -> Vec<SbsRow> {
    let mut rows = Vec::new();
    let mut old_line = old_start;
    let mut new_line = new_start;
    let mut i = 0;

    while i < hunk_lines.len() {
        match &hunk_lines[i] {
            DiffLine::Context(text) => {
                rows.push(SbsRow {
                    left: SbsCell::Context(old_line, text.clone()),
                    right: SbsCell::Context(new_line, text.clone()),
                });
                old_line += 1;
                new_line += 1;
                i += 1;
            }
            DiffLine::Removed(_) => {
                // Collect consecutive removed/added for pairing
                let remove_start = i;
                while i < hunk_lines.len() && matches!(&hunk_lines[i], DiffLine::Removed(_)) {
                    i += 1;
                }
                let add_start = i;
                while i < hunk_lines.len() && matches!(&hunk_lines[i], DiffLine::Added(_)) {
                    i += 1;
                }

                let removed: Vec<_> = hunk_lines[remove_start..add_start]
                    .iter()
                    .map(|l| match l {
                        DiffLine::Removed(t) => t.clone(),
                        _ => String::new(),
                    })
                    .collect();
                let added: Vec<_> = hunk_lines[add_start..i]
                    .iter()
                    .map(|l| match l {
                        DiffLine::Added(t) => t.clone(),
                        _ => String::new(),
                    })
                    .collect();

                let max_len = removed.len().max(added.len());
                for j in 0..max_len {
                    let left = if j < removed.len() {
                        let ln = old_line;
                        old_line += 1;
                        SbsCell::Removed(ln, removed[j].clone())
                    } else {
                        SbsCell::Empty
                    };
                    let right = if j < added.len() {
                        let ln = new_line;
                        new_line += 1;
                        // Reuse Removed variant for added (displayed with + on right side)
                        SbsCell::Removed(ln, added[j].clone())
                    } else {
                        SbsCell::Empty
                    };
                    rows.push(SbsRow { left, right });
                }
            }
            DiffLine::Added(text) => {
                rows.push(SbsRow {
                    left: SbsCell::Empty,
                    right: SbsCell::Removed(new_line, text.clone()),
                });
                new_line += 1;
                i += 1;
            }
        }
    }

    rows
}
