//! Permission request dispatch and approval logic for ACP.
//!
//! This module contains:
//! - [`is_safe_write_path`]: checks whether a write tool call targets a
//!   directory that can be auto-approved without user confirmation.
//! - [`handle_permission_request`]: the full `session/request_permission`
//!   handler, including auto-blocking of the `Skill` tool, auto-approval of
//!   read-only and safe-path write tools, and UI escalation for everything else.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::Ordering;

use super::agent::{AgentMessage, SafePaths};
use super::jsonrpc::{JsonRpcClient, RpcError};
use super::protocol::{PermissionOutcome, RequestPermissionParams, RequestPermissionResponse};
use tokio::sync::mpsc;

/// Extract the file path from a tool_call JSON and check if it is in a safe
/// directory that can be auto-approved for writes.
///
/// Safe directories include `/tmp`, the par-term shaders directory, and the
/// par-term config directory (for `.config-update.json`).
///
/// # TOCTOU (Time-of-Check / Time-of-Use) Risk
///
/// This function calls [`std::fs::canonicalize`] to resolve symlinks and `..` components
/// before checking whether the path falls within a safe root. This mitigates the most
/// common path-traversal vectors (e.g. `/tmp/../etc/passwd`).
///
/// However, a residual TOCTOU race remains: between the `canonicalize` call here and
/// the actual file write performed by the agent tool, a symlink could be created at the
/// target path that redirects the write to an unsafe location. This race is inherent to
/// any permission check that is separate from the I/O operation itself.
///
/// **Why accepted**: This is a standard limitation of filesystem-based access checks
/// in CLI tooling. The safe roots are locations the user already controls (`/tmp`,
/// their own config directory), and an attacker who can race a symlink creation in
/// those directories already has equivalent local access. The canonicalize step is
/// kept as a defense-in-depth measure against accidental traversal, not as a
/// security boundary against a local adversary.
pub fn is_safe_write_path(tool_call: &serde_json::Value, safe_paths: &SafePaths) -> bool {
    // Try to extract the path from various locations in the tool_call JSON.
    // Claude Code puts it in rawInput.file_path, rawInput.path, or the title
    // field as "Write /path/to/file".
    let path_str = tool_call
        .get("rawInput")
        .and_then(|ri| {
            ri.get("file_path")
                .or_else(|| ri.get("filePath"))
                .or_else(|| ri.get("path"))
                .and_then(|v| v.as_str())
        })
        .or_else(|| {
            // Fall back to extracting path from title: "Write /path/to/file"
            tool_call
                .get("title")
                .and_then(|v| v.as_str())
                .and_then(|t| t.split_whitespace().nth(1))
        });

    let Some(path_str) = path_str else {
        return false;
    };

    // Resolve the target path safely:
    // - existing paths are fully canonicalized
    // - non-existing paths resolve and canonicalize the parent, then append
    //   the final path component
    // This blocks prefix-based traversal tricks (`/tmp/../etc/...`) and
    // symlink escapes while still allowing new file creation in safe roots.
    let target = {
        let path = std::path::Path::new(path_str);
        if !path.is_absolute() {
            return false;
        }
        if path.exists() {
            match std::fs::canonicalize(path) {
                Ok(p) => p,
                Err(_) => return false,
            }
        } else {
            let Some(parent) = path.parent() else {
                return false;
            };
            let Ok(parent_real) = std::fs::canonicalize(parent) else {
                return false;
            };
            let Some(file_name) = path.file_name() else {
                return false;
            };
            parent_real.join(file_name)
        }
    };

    let mut safe_roots: Vec<PathBuf> = vec![
        PathBuf::from("/tmp"),
        PathBuf::from("/var/folders"),
        safe_paths.shaders_dir.clone(),
        safe_paths.config_dir.clone(),
    ];
    if let Ok(temp_dir) = std::env::var("TMPDIR") {
        safe_roots.push(PathBuf::from(temp_dir));
    }

    safe_roots.into_iter().any(|root| {
        std::fs::canonicalize(root)
            .map(|canonical_root| target.starts_with(canonical_root))
            .unwrap_or(false)
    })
}

/// Handle a `session/request_permission` RPC call from the agent.
///
/// This function:
/// 1. Parses the permission parameters.
/// 2. Auto-blocks the `Skill` tool (can produce malformed output with non-Claude backends).
/// 3. Auto-approves read-only tools and writes to safe directories.
/// 4. Escalates everything else to the UI via [`AgentMessage::PermissionRequest`].
pub async fn handle_permission_request(
    request_id: u64,
    params: Option<&serde_json::Value>,
    client: Arc<JsonRpcClient>,
    ui_tx: &mpsc::UnboundedSender<AgentMessage>,
    auto_approve: &std::sync::atomic::AtomicBool,
    safe_paths: &SafePaths,
) {
    if let Some(params) = params {
        match serde_json::from_value::<RequestPermissionParams>(params.clone()) {
            Ok(perm_params) => {
                // Identify the tool from the tool_call JSON.
                // Claude Code ACP puts the tool name in the "title"
                // field as "ToolName /path/..." rather than in a
                // dedicated "tool" or "name" field.
                let tool_name = perm_params
                    .tool_call
                    .get("tool")
                    .and_then(|v| v.as_str())
                    .or_else(|| perm_params.tool_call.get("name").and_then(|v| v.as_str()))
                    .or_else(|| {
                        perm_params
                            .tool_call
                            .get("toolName")
                            .and_then(|v| v.as_str())
                    })
                    .or_else(|| {
                        // Extract first word from "title" field
                        // e.g. "Write /path/to/file" → "Write"
                        perm_params
                            .tool_call
                            .get("title")
                            .and_then(|v| v.as_str())
                            .and_then(|t| t.split_whitespace().next())
                    })
                    .unwrap_or("");

                log::info!(
                    "ACP permission request: id={request_id} tool={tool_name} \
                     tool_call={}",
                    perm_params.tool_call
                );

                // The Skill tool can produce malformed raw function-tag
                // output with non-Claude backends (e.g. Ollama models).
                // Block it at the host permission layer and let the
                // conversation continue with normal chat text.
                let lower_tool = tool_name.to_lowercase();
                if lower_tool == "skill" {
                    let deny_option_id = perm_params
                        .options
                        .iter()
                        .find(|o| {
                            matches!(
                                o.kind.as_deref(),
                                Some("deny") | Some("reject") | Some("cancel") | Some("disallow")
                            ) || o.name.to_lowercase().contains("deny")
                                || o.name.to_lowercase().contains("reject")
                                || o.name.to_lowercase().contains("cancel")
                        })
                        .or_else(|| perm_params.options.first())
                        .map(|o| o.option_id.clone());

                    log::info!(
                        "ACP: auto-blocking tool={tool_name} id={request_id} \
                         chosen_option={deny_option_id:?}"
                    );

                    let outcome = RequestPermissionResponse {
                        outcome: PermissionOutcome {
                            outcome: "selected".to_string(),
                            option_id: deny_option_id,
                        },
                    };
                    let response_json = serde_json::to_value(&outcome).unwrap_or_default();
                    if let Err(e) = client.respond(request_id, Some(response_json), None).await {
                        log::error!("Failed to auto-block Skill permission: {e}");
                    }
                    return;
                }

                // Auto-approve read-only tools and config updates.
                // Write/edit tools require approval unless writing
                // to a temp directory (shaders dir, /tmp, etc.).
                let lower = tool_name.to_lowercase();
                let is_par_term_screenshot_tool = lower
                    .contains("par-term-config__terminal_screenshot")
                    || lower == "terminal_screenshot";
                let is_safe_fs_tool = {
                    let is_read_only = matches!(
                        lower.as_str(),
                        "read"
                            | "read_file"
                            | "readfile"
                            | "readtextfile"
                            | "glob"
                            | "grep"
                            | "find"
                            | "list_directory"
                            | "listdirectory"
                            | "toolsearch"
                            | "tool_search"
                            | "notebookedit"
                            | "notebook_edit"
                            | "config"
                            | "config_update"
                            | "configupdate"
                    ) || (lower.contains("par-term-config")
                        && !is_par_term_screenshot_tool);

                    let is_write_tool = matches!(
                        lower.as_str(),
                        "write" | "write_file" | "writefile" | "writetextfile" | "edit"
                    );

                    if is_read_only {
                        true
                    } else if is_write_tool {
                        // Only auto-approve writes to safe directories
                        is_safe_write_path(&perm_params.tool_call, safe_paths)
                    } else {
                        false
                    }
                };

                // Log all options for debugging.
                for (i, opt) in perm_params.options.iter().enumerate() {
                    log::info!(
                        "ACP permission option[{i}]: id={} name={} kind={:?}",
                        opt.option_id,
                        opt.name,
                        opt.kind
                    );
                }

                if (auto_approve.load(Ordering::Relaxed) && !is_par_term_screenshot_tool)
                    || is_safe_fs_tool
                {
                    // Auto-approve: pick the first "allow" option, or just
                    // the first option available.
                    let option_id = perm_params
                        .options
                        .iter()
                        .find(|o| {
                            o.kind.as_deref() == Some("allow")
                                || o.kind.as_deref() == Some("allowOnce")
                                || o.name.to_lowercase().contains("allow")
                        })
                        .or_else(|| perm_params.options.first())
                        .map(|o| o.option_id.clone());

                    log::info!(
                        "ACP: auto-approving tool={tool_name} id={request_id} \
                         chosen_option={option_id:?}"
                    );

                    // Notify the UI about the auto-approval
                    let description = perm_params
                        .tool_call
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or(tool_name)
                        .to_string();
                    let _ = ui_tx.send(AgentMessage::AutoApproved(description));

                    let outcome = RequestPermissionResponse {
                        outcome: PermissionOutcome {
                            outcome: "selected".to_string(),
                            option_id,
                        },
                    };
                    let response_json = serde_json::to_value(&outcome).unwrap_or_default();
                    log::info!("ACP: sending permission response: {response_json}");
                    if let Err(e) = client.respond(request_id, Some(response_json), None).await {
                        log::error!("Failed to auto-approve permission: {e}");
                    }
                } else {
                    let _ = ui_tx.send(AgentMessage::PermissionRequest {
                        request_id,
                        tool_call: perm_params.tool_call,
                        options: perm_params.options,
                    });
                }
            }
            Err(e) => {
                log::error!("Failed to parse permission params: {e}");
                let _ = client
                    .respond(
                        request_id,
                        None,
                        Some(RpcError {
                            code: -32602,
                            message: "Invalid params".to_string(),
                            data: None,
                        }),
                    )
                    .await;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_safe_paths() -> SafePaths {
        let base = std::env::temp_dir().join(format!(
            "par-term-acp-permissions-tests-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        let config_dir = base.join("config");
        let shaders_dir = base.join("shaders");
        std::fs::create_dir_all(&config_dir).expect("create config dir");
        std::fs::create_dir_all(&shaders_dir).expect("create shaders dir");

        SafePaths {
            config_dir,
            shaders_dir,
        }
    }

    #[test]
    fn test_safe_write_path_tmp() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/tmp/test.glsl"},
            "title": "Write /tmp/test.glsl"
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_shaders_dir() {
        let safe_paths = make_safe_paths();
        let path = safe_paths.shaders_dir.join("crt.glsl");
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": path.to_string_lossy()},
            "title": format!("Write {}", path.display())
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_config_dir() {
        let safe_paths = make_safe_paths();
        let path = safe_paths.config_dir.join(".config-update.json");
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": path.to_string_lossy()},
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_unsafe_write_path_home() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/Users/someone/.bashrc"},
            "title": "Write /Users/someone/.bashrc"
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_unsafe_write_path_system() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/etc/passwd"},
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_from_title_fallback() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "title": "Write /tmp/shader.glsl"
        });
        assert!(is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_safe_write_path_no_path() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "title": "Write"
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }

    #[test]
    fn test_unsafe_write_path_tmp_traversal() {
        let safe_paths = make_safe_paths();
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": "/tmp/../etc/passwd"},
            "title": "Write /tmp/../etc/passwd"
        });
        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }

    #[cfg(unix)]
    #[test]
    fn test_unsafe_write_path_tmp_symlink_escape() {
        use std::os::unix::fs::symlink;

        let base = std::env::temp_dir().join(format!(
            "par-term-acp-permissions-symlink-tests-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock should be after epoch")
                .as_nanos()
        ));
        let safe_root = base.join("safe");
        let config_dir = base.join("config");
        std::fs::create_dir_all(&safe_root).expect("create safe root");
        std::fs::create_dir_all(&config_dir).expect("create config root");
        symlink("/etc", safe_root.join("escape")).expect("create symlink");

        let safe_paths = SafePaths {
            shaders_dir: safe_root.clone(),
            config_dir,
        };
        let escaped_path = safe_root.join("escape").join("leak.glsl");
        let tool_call = serde_json::json!({
            "rawInput": {"file_path": escaped_path.to_string_lossy()},
            "title": format!("Write {}", escaped_path.display())
        });

        assert!(!is_safe_write_path(&tool_call, &safe_paths));
    }
}
