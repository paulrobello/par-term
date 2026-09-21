//! Format-specific finalization methods for [`SessionLogger`].
//!
//! Contains the HTML header/footer writers and the asciicast event serializer.
//! These are split from `core.rs` to keep each file under 500 lines.

use anyhow::{Context, Result};
use par_term_emu_core_rust::terminal::RecordingEventType;

use super::core::SessionLogger;
use super::writers::html_escape;
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
    ///
    /// SEC-009: the session title is derived from the tab title, which a remote
    /// process controls through OSC 0/2. It is HTML-escaped here for the same
    /// reason the log body is — an unescaped `</title><script>…` would execute
    /// when the log is opened in a browser.
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
            html_escape(self.title.as_deref().unwrap_or("Terminal Session"))
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

    /// Write the recording as asciicast v3.
    ///
    /// When a v3 exporter is wired (see `SessionLogger::set_v3_exporter`),
    /// the core lib's graphics-aware export serializes the session: its `g`
    /// events carry base64 pixel data and derive graphic times from unix-ms
    /// `added_at` stamps minus the recording's epoch start, which `start()`
    /// re-anchors to the monotonic timeline's origin. Without an exporter the
    /// recording serializes text-only, which is still a valid v3 stream.
    pub(super) fn write_asciicast_v3(&mut self) -> Result<()> {
        use std::io::Write;

        if let (Some(exporter), Some(recording)) = (&self.v3_exporter, &self.recording)
            && let Some(output) = exporter(recording)
        {
            if let Some(ref mut writer) = self.writer {
                writer.write_all(output.as_bytes()).with_context(|| {
                    format!("Failed to write asciicast v3 to {:?}", self.output_path)
                })?;
            }
            return Ok(());
        }

        // Text-only fallback for loggers without a wired exporter.
        if let (Some(recording), Some(writer)) = (&self.recording, &mut self.writer) {
            let header = serde_json::json!({
                "version": 3,
                "term": {
                    "cols": recording.initial_size.0,
                    "rows": recording.initial_size.1,
                },
                "timestamp": recording.created_at / 1000,
                "title": &recording.title,
                "env": recording.env,
            });
            writeln!(writer, "{}", header).with_context(|| {
                format!(
                    "Failed to write asciicast v3 header to {:?}",
                    self.output_path
                )
            })?;

            // Text events with v3's relative intervals; graphics markers
            // cannot be conveyed without a terminal, so markers/metadata
            // events are dropped as in the core lib's own text mapping.
            let mut prev_ms: u64 = 0;
            for event in &recording.events {
                let t_ms = event.timestamp;
                let mut line = match event.event_type {
                    RecordingEventType::Output => {
                        serde_json::json!([0.0, "o", String::from_utf8_lossy(&event.data)])
                    }
                    RecordingEventType::Input => {
                        serde_json::json!([0.0, "i", String::from_utf8_lossy(&event.data)])
                    }
                    RecordingEventType::Resize => {
                        if let Some((cols, rows)) = event.metadata {
                            serde_json::json!([0.0, "r", format!("{}x{}", cols, rows)])
                        } else {
                            continue;
                        }
                    }
                    _ => continue,
                };
                let interval = t_ms.saturating_sub(prev_ms) as f64 / 1_000.0;
                line[0] = serde_json::json!(interval);
                writeln!(writer, "{}", line)?;
                prev_ms = t_ms;
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
        SessionLogFormat::AsciicastV3 => logger.write_asciicast_v3(),
    }
}
