//! Text parsing utilities: code block extraction, inline tool parsing.

/// Truncate replay transcript text by Unicode scalar count and append an ASCII suffix.
pub(super) fn truncate_replay_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    if max_chars <= 3 {
        return ".".repeat(max_chars);
    }
    let mut out: String = text.chars().take(max_chars - 3).collect();
    out.push_str("...");
    out
}

/// A segment of agent message text for rendering.
#[derive(Debug, PartialEq)]
pub enum TextSegment {
    /// Regular (non-code) text.
    Plain(String),
    /// A fenced code block with optional language tag.
    CodeBlock { lang: String, code: String },
}

/// Parse agent message text into alternating plain-text and code-block segments.
///
/// Recognises fenced code blocks delimited by triple backticks, with an
/// optional language tag on the opening fence. Unclosed code blocks are
/// treated as extending to the end of the text.
pub fn parse_text_segments(text: &str) -> Vec<TextSegment> {
    let mut segments = Vec::new();
    let mut plain_lines: Vec<&str> = Vec::new();
    let mut in_block = false;
    let mut block_lang = String::new();
    let mut code_lines: Vec<&str> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_block {
                // End of code block
                let code = code_lines.join("\n");
                segments.push(TextSegment::CodeBlock {
                    lang: std::mem::take(&mut block_lang),
                    code,
                });
                code_lines.clear();
                in_block = false;
            } else {
                // Flush accumulated plain text
                if !plain_lines.is_empty() {
                    segments.push(TextSegment::Plain(plain_lines.join("\n")));
                    plain_lines.clear();
                }
                // Start code block — extract language tag
                block_lang = trimmed.trim_start_matches('`').trim().to_string();
                in_block = true;
            }
        } else if in_block {
            code_lines.push(line);
        } else {
            plain_lines.push(line);
        }
    }

    // Flush remaining content
    if in_block {
        let code = code_lines.join("\n");
        segments.push(TextSegment::CodeBlock {
            lang: block_lang,
            code,
        });
    } else if !plain_lines.is_empty() {
        segments.push(TextSegment::Plain(plain_lines.join("\n")));
    }

    segments
}

/// Extract shell commands from fenced code blocks in text.
///
/// Looks for code blocks tagged with `bash`, `sh`, `shell`, or `zsh`.
/// Supports additional metadata after the language tag (for example:
/// ` ```bash title=example`), and combines continuation lines ending with `\`.
/// Lines starting with `#` (comments) or empty lines are skipped.
pub(super) fn extract_code_block_commands(text: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut in_block = false;
    let mut is_shell_block = false;
    let mut continued: Vec<String> = Vec::new();

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("```") {
            if in_block {
                // End of block
                if !continued.is_empty() {
                    commands.push(continued.join(" "));
                    continued.clear();
                }
                in_block = false;
                is_shell_block = false;
            } else {
                // Start of block — check language tag
                let lang = trimmed
                    .trim_start_matches('`')
                    .split_whitespace()
                    .next()
                    .unwrap_or_default()
                    .to_ascii_lowercase();
                is_shell_block = lang == "bash" || lang == "sh" || lang == "shell" || lang == "zsh";
                in_block = true;
            }
            continue;
        }

        if in_block && is_shell_block {
            let cmd = trimmed.strip_prefix("$ ").unwrap_or(trimmed);
            if cmd.is_empty() || cmd.starts_with('#') {
                continue;
            }

            let continued_line = cmd.ends_with('\\');
            let segment = if continued_line {
                cmd.trim_end_matches('\\').trim_end()
            } else {
                cmd
            };

            if !segment.is_empty() {
                continued.push(segment.to_string());
            }

            if !continued_line && !continued.is_empty() {
                commands.push(continued.join(" "));
                continued.clear();
            }
        }
    }

    if !continued.is_empty() {
        commands.push(continued.join(" "));
    }

    commands
}

/// Extract a literal XML-style `config_update` tool call emitted as plain text.
///
/// Some local backends can emit `<function=...>` / `<parameter=...>` blocks
/// instead of a structured ACP tool call. This helper parses that fallback
/// format so the host can still apply the requested config update.
pub fn extract_inline_config_update(
    text: &str,
) -> Option<std::collections::HashMap<String, serde_json::Value>> {
    const FN_TAG: &str = "<function=mcp__par-term-config__config_update>";
    const PARAM_START: &str = "<parameter=updates>";
    const PARAM_END: &str = "</parameter>";

    let fn_idx = text.find(FN_TAG)?;
    let after_fn = &text[fn_idx + FN_TAG.len()..];
    let param_idx = after_fn.find(PARAM_START)?;
    let after_param = &after_fn[param_idx + PARAM_START.len()..];
    let end_idx = after_param.find(PARAM_END)?;
    let json_text = after_param[..end_idx].trim();
    if json_text.is_empty() {
        return None;
    }

    let parsed: serde_json::Value = serde_json::from_str(json_text).ok()?;
    match parsed {
        serde_json::Value::Object(mut map) => {
            if let Some(serde_json::Value::Object(updates)) = map.remove("updates") {
                Some(updates.into_iter().collect())
            } else {
                Some(map.into_iter().collect())
            }
        }
        _ => None,
    }
}

/// Extract the function name from XML-style inline tool markup emitted as
/// plain text, e.g. `<function=Write>` -> `Write`.
pub fn extract_inline_tool_function_name(text: &str) -> Option<String> {
    let start = text.find("<function=")?;
    let after = &text[start + "<function=".len()..];
    let end = after.find('>')?;
    let name = after[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}
