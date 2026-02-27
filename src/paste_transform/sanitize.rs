//! Paste content sanitization: strips dangerous terminal control characters.

/// Sanitize clipboard paste content by stripping dangerous control characters.
///
/// Removes characters that could inject terminal escape sequences when pasted:
/// - C0 control characters (0x00-0x1F) **except** Tab (0x09), Newline (0x0A),
///   and Carriage Return (0x0D) which are safe/expected in paste content
/// - ESC (0x1B) is explicitly stripped to prevent escape sequence injection
/// - C1 control characters (0x80-0x9F) including CSI (0x9B)
///
/// All normal printable ASCII, extended Latin, and Unicode text passes through
/// unchanged.
pub fn sanitize_paste_content(input: &str) -> String {
    input
        .chars()
        .filter(|&ch| {
            let code = ch as u32;
            // Allow safe C0 controls: Tab, Newline, Carriage Return
            if ch == '\t' || ch == '\n' || ch == '\r' {
                return true;
            }
            // Strip C0 control characters (0x00-0x1F) — includes ESC (0x1B)
            if code <= 0x1F {
                return false;
            }
            // Strip DEL (0x7F)
            if code == 0x7F {
                return false;
            }
            // Strip C1 control characters (0x80-0x9F) — includes CSI (0x9B)
            if (0x80..=0x9F).contains(&code) {
                return false;
            }
            true
        })
        .collect()
}
