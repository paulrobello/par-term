//! Format-specific finalization methods for [`SessionLogger`].
//!
//! Contains the HTML header/footer writers and the asciicast event serializer.
//! These are split from `core.rs` to keep each file under 500 lines.

use anyhow::{Context, Result};
use par_term_emu_core_rust::terminal::RecordingEventType;

use super::core::SessionLogger;
use crate::config::SessionLogFormat;

impl SessionLogger {
    /// Write a plain-text redaction limitation warning at the start of a Plain log file.
    ///
    /// SEC-004: This comment informs readers of session logs that credential
    /// redaction is heuristic and may miss secrets that are not expressed in
    /// well-known `KEY=value` or prompt patterns. Users should disable session
    /// logging when working with credentials directly in the terminal.
    pub(super) fn write_plain_redaction_warning(&mut self) -> Result<()> {
        use std::io::Write;
        let warning = "\
# par-term session log
# WARNING: Credential redaction is heuristic and has known limitations.
# Patterns matched: common KEY=value exports, password/passphrase prompts,
#   Bearer tokens, PEM private key blocks, CI tokens (GITHUB_TOKEN, HEROKU_API_KEY, etc.)
# Known gaps: secrets printed without a recognisable variable name, base64-encoded
#   tokens, secrets embedded in JSON/YAML output, or any novel credential format.
# RECOMMENDATION: Disable session logging (Settings > Logging) before working with
#   credentials, API keys, or other sensitive values in the terminal.
\n";
        if let Some(ref mut writer) = self.writer {
            writer.write_all(warning.as_bytes()).with_context(|| {
                format!(
                    "Failed to write redaction warning to {:?}",
                    self.output_path
                )
            })?;
        }
        Ok(())
    }

    /// Write the HTML document header to the log file.
    pub(super) fn write_html_header(&mut self) -> Result<()> {
        use std::io::Write;
        let header = format!(
            r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="UTF-8">
    <title>{}</title>
    <style>
        body {{
            background-color: #1e1e1e;
            color: #d4d4d4;
            font-family: 'JetBrains Mono', 'Fira Code', 'Consolas', monospace;
            font-size: 14px;
            padding: 20px;
            white-space: pre-wrap;
            word-wrap: break-word;
        }}
        .timestamp {{
            color: #808080;
            font-size: 10px;
        }}
    </style>
</head>
<body>
<pre>
"#,
            self.title.as_deref().unwrap_or("Terminal Session")
        );

        if let Some(ref mut writer) = self.writer {
            writer.write_all(header.as_bytes()).with_context(|| {
                format!("Failed to write HTML header to {:?}", self.output_path)
            })?;
        }
        Ok(())
    }

    /// Write the HTML document footer to the log file.
    pub(super) fn write_html_footer(&mut self) -> Result<()> {
        use std::io::Write;
        let footer = r#"
</pre>
</body>
</html>
"#;
        if let Some(ref mut writer) = self.writer {
            writer.write_all(footer.as_bytes()).with_context(|| {
                format!("Failed to write HTML footer to {:?}", self.output_path)
            })?;
        }
        Ok(())
    }

    /// Serialize the recording session to asciicast v2 format and write it to the log file.
    pub(super) fn write_asciicast(&mut self) -> Result<()> {
        use std::io::Write;
        if let Some(ref recording) = self.recording {
            // Write asciicast v2 format
            // Header line (JSON object)
            let header = serde_json::json!({
                "version": 2,
                "width": recording.initial_size.0,
                "height": recording.initial_size.1,
                "timestamp": recording.created_at / 1000, // Convert to seconds
                "title": &recording.title,
                "env": recording.env,
            });

            if let Some(ref mut writer) = self.writer {
                writeln!(writer, "{}", header).with_context(|| {
                    format!("Failed to write asciicast header to {:?}", self.output_path)
                })?;

                // Event lines (JSON arrays)
                for event in &recording.events {
                    let time_seconds = event.timestamp as f64 / 1000.0;

                    match event.event_type {
                        RecordingEventType::Output => {
                            let data_str = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "o", data_str]);
                            writeln!(writer, "{}", line)?;
                        }
                        RecordingEventType::Input => {
                            let data_str = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "i", data_str]);
                            writeln!(writer, "{}", line)?;
                        }
                        RecordingEventType::Resize => {
                            if let Some((cols, rows)) = event.metadata {
                                let line = serde_json::json!([
                                    time_seconds,
                                    "r",
                                    format!("{}x{}", cols, rows)
                                ]);
                                writeln!(writer, "{}", line)?;
                            }
                        }
                        RecordingEventType::Marker => {
                            let label = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "m", label]);
                            writeln!(writer, "{}", line)?;
                        }
                        RecordingEventType::Metadata => {
                            // Metadata events store key-value pairs; emit as asciicast marker
                            let data_str = String::from_utf8_lossy(&event.data);
                            let line = serde_json::json!([time_seconds, "m", data_str]);
                            writeln!(writer, "{}", line)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

/// Helper used by `SessionLogger::stop()` to dispatch the right finalization method.
pub(super) fn finalize_format(logger: &mut SessionLogger) -> Result<()> {
    match logger.format {
        SessionLogFormat::Plain => Ok(()),
        SessionLogFormat::Html => logger.write_html_footer(),
        SessionLogFormat::Asciicast => logger.write_asciicast(),
    }
}
