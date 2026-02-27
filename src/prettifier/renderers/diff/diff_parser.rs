//! Unified diff parsing for the diff renderer.
//!
//! Parses unified diff format (`diff -u` / `diff --git`) into structured
//! [`DiffFile`] objects containing [`DiffHunk`]s with typed [`DiffLine`]
//! variants.  The main entry point is [`parse_unified_diff`].

// ---------------------------------------------------------------------------
// Parsed diff types
// ---------------------------------------------------------------------------

/// A parsed diff covering one or more files.
#[derive(Debug, Clone)]
pub(super) struct DiffFile {
    pub(super) old_path: String,
    pub(super) new_path: String,
    pub(super) hunks: Vec<DiffHunk>,
    /// Extra header lines (e.g. `index ...`, `mode ...`).
    pub(super) header_lines: Vec<String>,
}

/// A single hunk within a diff file.
#[derive(Debug, Clone)]
pub(super) struct DiffHunk {
    pub(super) old_start: usize,
    pub(super) old_count: usize,
    pub(super) new_start: usize,
    pub(super) new_count: usize,
    /// Optional function/context text after the @@ markers.
    pub(super) header_text: String,
    pub(super) lines: Vec<DiffLine>,
}

/// A single line within a diff hunk.
#[derive(Debug, Clone)]
pub(super) enum DiffLine {
    Context(String),
    Added(String),
    Removed(String),
}

// ---------------------------------------------------------------------------
// Parsing functions
// ---------------------------------------------------------------------------

/// Parse unified diff content into structured `DiffFile`s.
pub(super) fn parse_unified_diff(lines: &[String]) -> Vec<DiffFile> {
    let mut files: Vec<DiffFile> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        let line = &lines[i];

        // Start of a new file diff: `diff --git a/... b/...`
        if line.starts_with("diff --git ") {
            let (old_path, new_path) = parse_git_diff_header(line);
            let mut header_lines = vec![line.clone()];

            i += 1;
            // Collect header lines until we hit --- or another diff/hunk
            while i < lines.len()
                && !lines[i].starts_with("--- ")
                && !lines[i].starts_with("diff --git ")
                && !lines[i].starts_with("@@ ")
            {
                header_lines.push(lines[i].clone());
                i += 1;
            }

            // Parse --- / +++ if present
            let mut final_old = old_path;
            let mut final_new = new_path;
            if i < lines.len() && lines[i].starts_with("--- ") {
                final_old = lines[i][4..].trim().to_string();
                i += 1;
                if i < lines.len() && lines[i].starts_with("+++ ") {
                    final_new = lines[i][4..].trim().to_string();
                    i += 1;
                }
            }

            // Parse hunks
            let mut hunks = Vec::new();
            while i < lines.len() && !lines[i].starts_with("diff --git ") {
                if lines[i].starts_with("@@ ") {
                    let (hunk, next_i) = parse_hunk(lines, i);
                    hunks.push(hunk);
                    i = next_i;
                } else {
                    i += 1;
                }
            }

            files.push(DiffFile {
                old_path: final_old,
                new_path: final_new,
                hunks,
                header_lines,
            });
        } else if line.starts_with("--- ")
            && i + 1 < lines.len()
            && lines[i + 1].starts_with("+++ ")
        {
            // Non-git unified diff (e.g., `diff -u`)
            let old_path = lines[i][4..].trim().to_string();
            let new_path = lines[i + 1][4..].trim().to_string();
            i += 2;

            let mut hunks = Vec::new();
            while i < lines.len()
                && !lines[i].starts_with("--- ")
                && !lines[i].starts_with("diff --git ")
            {
                if lines[i].starts_with("@@ ") {
                    let (hunk, next_i) = parse_hunk(lines, i);
                    hunks.push(hunk);
                    i = next_i;
                } else {
                    i += 1;
                }
            }

            files.push(DiffFile {
                old_path,
                new_path,
                hunks,
                header_lines: Vec::new(),
            });
        } else {
            i += 1;
        }
    }

    files
}

/// Parse `diff --git a/path b/path` into (old_path, new_path).
pub(super) fn parse_git_diff_header(line: &str) -> (String, String) {
    let rest = line.strip_prefix("diff --git ").unwrap_or(line);
    // Split on " b/" — handles `a/path b/path`
    if let Some(idx) = rest.find(" b/") {
        let old = rest[..idx].to_string();
        let new = rest[idx + 1..].to_string();
        (old, new)
    } else {
        // Fallback: split on space
        let parts: Vec<&str> = rest.splitn(2, ' ').collect();
        if parts.len() == 2 {
            (parts[0].to_string(), parts[1].to_string())
        } else {
            (rest.to_string(), rest.to_string())
        }
    }
}

/// Parse a hunk starting at `@@ -old_start,old_count +new_start,new_count @@ ...`.
/// Returns the parsed hunk and the index of the next line after the hunk.
pub(super) fn parse_hunk(lines: &[String], start: usize) -> (DiffHunk, usize) {
    let header = &lines[start];
    let (old_start, old_count, new_start, new_count, header_text) = parse_hunk_header(header);

    let mut hunk_lines = Vec::new();
    let mut i = start + 1;

    while i < lines.len() {
        let line = &lines[i];
        if line.starts_with("@@ ") || line.starts_with("diff --git ") || line.starts_with("--- ") {
            break;
        }

        if let Some(rest) = line.strip_prefix('+') {
            hunk_lines.push(DiffLine::Added(rest.to_string()));
        } else if let Some(rest) = line.strip_prefix('-') {
            hunk_lines.push(DiffLine::Removed(rest.to_string()));
        } else if let Some(rest) = line.strip_prefix(' ') {
            hunk_lines.push(DiffLine::Context(rest.to_string()));
        } else {
            // No-newline-at-end-of-file marker or other, treat as context
            hunk_lines.push(DiffLine::Context(line.to_string()));
        }
        i += 1;
    }

    (
        DiffHunk {
            old_start,
            old_count,
            new_start,
            new_count,
            header_text,
            lines: hunk_lines,
        },
        i,
    )
}

/// Parse `@@ -old_start,old_count +new_start,new_count @@ optional_text`.
pub(super) fn parse_hunk_header(header: &str) -> (usize, usize, usize, usize, String) {
    let mut old_start = 1;
    let mut old_count = 1;
    let mut new_start = 1;
    let mut new_count = 1;
    let mut header_text = String::new();

    // Find the range between the @@ markers
    if let Some(rest) = header.strip_prefix("@@ ")
        && let Some(end_idx) = rest.find(" @@")
    {
        let range_part = &rest[..end_idx];
        header_text = rest[end_idx + 3..].trim().to_string();

        // Parse -old_start,old_count +new_start,new_count
        let parts: Vec<&str> = range_part.split_whitespace().collect();
        for part in parts {
            if let Some(old_part) = part.strip_prefix('-') {
                let nums: Vec<&str> = old_part.split(',').collect();
                old_start = nums[0].parse().unwrap_or(1);
                if nums.len() > 1 {
                    old_count = nums[1].parse().unwrap_or(1);
                }
            } else if let Some(new_part) = part.strip_prefix('+') {
                let nums: Vec<&str> = new_part.split(',').collect();
                new_start = nums[0].parse().unwrap_or(1);
                if nums.len() > 1 {
                    new_count = nums[1].parse().unwrap_or(1);
                }
            }
        }
    }

    (old_start, old_count, new_start, new_count, header_text)
}
