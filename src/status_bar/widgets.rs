//! Widget text generation and layout helpers for the status bar.
//!
//! Each [`WidgetId`] variant maps to a function that produces a display string
//! from the current [`WidgetContext`].  An optional format-override string
//! supports `\(variable)` interpolation.

use crate::badge::SessionVariables;
use crate::status_bar::config::{StatusBarSection, StatusBarWidgetConfig, WidgetId};
use crate::status_bar::system_monitor::{SystemMonitorData, format_bytes_per_sec, format_memory};

/// Runtime context passed to widget text generators.
#[derive(Debug, Clone)]
pub struct WidgetContext {
    /// Session variables (hostname, username, path, bell count, etc.)
    pub session_vars: SessionVariables,
    /// Latest system monitor snapshot
    pub system_data: SystemMonitorData,
    /// Current git branch (if known)
    pub git_branch: Option<String>,
}

/// Generate display text for a single widget.
///
/// If `format_override` is `Some`, the format string is interpolated instead
/// of the built-in formatting.
pub fn widget_text(id: &WidgetId, ctx: &WidgetContext, format_override: Option<&str>) -> String {
    if let Some(fmt) = format_override {
        return interpolate_format(fmt, ctx);
    }

    match id {
        WidgetId::Clock => chrono::Local::now().format("%H:%M:%S").to_string(),
        WidgetId::UsernameHostname => {
            format!(
                "{}@{}",
                ctx.session_vars.username, ctx.session_vars.hostname
            )
        }
        WidgetId::CurrentDirectory => ctx.session_vars.path.clone(),
        WidgetId::GitBranch => {
            if let Some(ref branch) = ctx.git_branch {
                format!("\u{e0a0} {}", branch)
            } else {
                String::new()
            }
        }
        WidgetId::CpuUsage => format!("CPU {:.1}%", ctx.system_data.cpu_usage),
        WidgetId::MemoryUsage => {
            format!(
                "MEM {}",
                format_memory(ctx.system_data.memory_used, ctx.system_data.memory_total)
            )
        }
        WidgetId::NetworkStatus => {
            format!(
                "\u{2193} {} \u{2191} {}",
                format_bytes_per_sec(ctx.system_data.network_rx_rate),
                format_bytes_per_sec(ctx.system_data.network_tx_rate)
            )
        }
        WidgetId::BellIndicator => {
            if ctx.session_vars.bell_count > 0 {
                format!("\u{1f514} {}", ctx.session_vars.bell_count)
            } else {
                String::new()
            }
        }
        WidgetId::CurrentCommand => ctx.session_vars.current_command.clone().unwrap_or_default(),
        WidgetId::Custom(_) => String::new(),
    }
}

/// Interpolate `\(variable)` placeholders in a format string.
///
/// Supported variables:
/// - `\(session.hostname)`, `\(session.username)`, `\(session.path)`, etc.
/// - `\(git.branch)`
/// - `\(system.cpu)`, `\(system.memory)`
pub fn interpolate_format(fmt: &str, ctx: &WidgetContext) -> String {
    let mut result = String::with_capacity(fmt.len());
    let mut chars = fmt.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\\' && chars.peek() == Some(&'(') {
            // Consume '('
            chars.next();
            // Collect variable name until ')'
            let mut var_name = String::new();
            let mut found_close = false;
            for c in chars.by_ref() {
                if c == ')' {
                    found_close = true;
                    break;
                }
                var_name.push(c);
            }
            if found_close {
                // Resolve variable
                let value = resolve_variable(&var_name, ctx);
                result.push_str(&value);
            } else {
                // Unterminated \( — output raw text
                result.push_str("\\(");
                result.push_str(&var_name);
            }
        } else {
            result.push(ch);
        }
    }

    result
}

/// Resolve a single variable name to its string value.
fn resolve_variable(name: &str, ctx: &WidgetContext) -> String {
    match name {
        // Session variables delegate to SessionVariables::get
        n if n.starts_with("session.") => ctx.session_vars.get(n).unwrap_or_default(),
        "git.branch" => ctx.git_branch.clone().unwrap_or_default(),
        "system.cpu" => format!("{:.1}%", ctx.system_data.cpu_usage),
        "system.memory" => format_memory(ctx.system_data.memory_used, ctx.system_data.memory_total),
        _ => String::new(),
    }
}

/// Return widgets for a given section, filtered by enabled, sorted by order.
pub fn sorted_widgets_for_section(
    widgets: &[StatusBarWidgetConfig],
    section: StatusBarSection,
) -> Vec<&StatusBarWidgetConfig> {
    let mut result: Vec<&StatusBarWidgetConfig> = widgets
        .iter()
        .filter(|w| w.enabled && w.section == section)
        .collect();
    result.sort_by_key(|w| w.order);
    result
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::status_bar::config::StatusBarSection;

    fn make_ctx() -> WidgetContext {
        let sv = SessionVariables {
            username: "alice".to_string(),
            hostname: "dev-box".to_string(),
            path: "/home/alice/project".to_string(),
            bell_count: 3,
            current_command: Some("cargo build".to_string()),
            ..Default::default()
        };

        WidgetContext {
            session_vars: sv,
            system_data: SystemMonitorData {
                cpu_usage: 42.5,
                memory_used: 4_294_967_296,   // 4 GB
                memory_total: 17_179_869_184, // 16 GB
                network_rx_rate: 1024,
                network_tx_rate: 2048,
                last_update: None,
            },
            git_branch: Some("main".to_string()),
        }
    }

    #[test]
    fn test_widget_text_clock() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::Clock, &ctx, None);
        // Should be HH:MM:SS format
        assert_eq!(text.len(), 8);
        assert_eq!(text.as_bytes()[2], b':');
        assert_eq!(text.as_bytes()[5], b':');
    }

    #[test]
    fn test_widget_text_username_hostname() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::UsernameHostname, &ctx, None);
        assert_eq!(text, "alice@dev-box");
    }

    #[test]
    fn test_widget_text_current_directory() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::CurrentDirectory, &ctx, None);
        assert_eq!(text, "/home/alice/project");
    }

    #[test]
    fn test_widget_text_git_branch() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::GitBranch, &ctx, None);
        assert_eq!(text, "\u{e0a0} main");

        // No branch
        let mut ctx2 = make_ctx();
        ctx2.git_branch = None;
        let text = widget_text(&WidgetId::GitBranch, &ctx2, None);
        assert!(text.is_empty());
    }

    #[test]
    fn test_widget_text_cpu_usage() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::CpuUsage, &ctx, None);
        assert_eq!(text, "CPU 42.5%");
    }

    #[test]
    fn test_widget_text_memory_usage() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::MemoryUsage, &ctx, None);
        assert_eq!(text, "MEM 4.0 GB / 16.0 GB");
    }

    #[test]
    fn test_widget_text_network_status() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::NetworkStatus, &ctx, None);
        assert_eq!(text, "\u{2193} 1.0 KB/s \u{2191} 2.0 KB/s");
    }

    #[test]
    fn test_widget_text_bell_indicator() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::BellIndicator, &ctx, None);
        assert_eq!(text, "\u{1f514} 3");

        let mut ctx2 = make_ctx();
        ctx2.session_vars.bell_count = 0;
        let text = widget_text(&WidgetId::BellIndicator, &ctx2, None);
        assert!(text.is_empty());
    }

    #[test]
    fn test_widget_text_current_command() {
        let ctx = make_ctx();
        let text = widget_text(&WidgetId::CurrentCommand, &ctx, None);
        assert_eq!(text, "cargo build");
    }

    #[test]
    fn test_widget_text_format_override() {
        let ctx = make_ctx();
        let text = widget_text(
            &WidgetId::UsernameHostname,
            &ctx,
            Some("Host: \\(session.hostname) CPU: \\(system.cpu)"),
        );
        assert_eq!(text, "Host: dev-box CPU: 42.5%");
    }

    #[test]
    fn test_interpolate_format() {
        let ctx = make_ctx();
        let result = interpolate_format(
            "\\(session.username)@\\(session.hostname) [\\(git.branch)]",
            &ctx,
        );
        assert_eq!(result, "alice@dev-box [main]");
    }

    #[test]
    fn test_sorted_widgets_for_section() {
        let widgets = vec![
            StatusBarWidgetConfig {
                id: WidgetId::Clock,
                enabled: true,
                section: StatusBarSection::Right,
                order: 2,
                format: None,
            },
            StatusBarWidgetConfig {
                id: WidgetId::CpuUsage,
                enabled: false,
                section: StatusBarSection::Right,
                order: 0,
                format: None,
            },
            StatusBarWidgetConfig {
                id: WidgetId::BellIndicator,
                enabled: true,
                section: StatusBarSection::Right,
                order: 1,
                format: None,
            },
            StatusBarWidgetConfig {
                id: WidgetId::UsernameHostname,
                enabled: true,
                section: StatusBarSection::Left,
                order: 0,
                format: None,
            },
        ];

        let right = sorted_widgets_for_section(&widgets, StatusBarSection::Right);
        assert_eq!(right.len(), 2); // CpuUsage is disabled
        assert_eq!(right[0].id, WidgetId::BellIndicator); // order 1
        assert_eq!(right[1].id, WidgetId::Clock); // order 2

        let left = sorted_widgets_for_section(&widgets, StatusBarSection::Left);
        assert_eq!(left.len(), 1);
        assert_eq!(left[0].id, WidgetId::UsernameHostname);

        let center = sorted_widgets_for_section(&widgets, StatusBarSection::Center);
        assert!(center.is_empty());
    }
}
