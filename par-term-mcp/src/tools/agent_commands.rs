//! Handlers for the `command_list`, `command_create`, and `command_delete`
//! MCP tools — the agent-authored command write path.
//!
//! These write directly into `<config_dir>/commands/` (validated, atomic,
//! provenance-enforced by `par_term_config::agent_commands`); the running
//! par-term app picks the change up through its directory watcher with no
//! restart, the same contract `config_update` established.

use crate::tools::tool_error;
use par_term_config::agent_commands::{self as fmt, AgentCommandFile, CommandAuthor};
use serde_json::Value;

/// Resolve the commands dir, honouring the test-overridable IPC env var so
/// tests never touch the real config directory.
fn commands_dir() -> std::path::PathBuf {
    if let Ok(root) = std::env::var("PAR_TERM_IPC_DIR") {
        return std::path::PathBuf::from(root).join("commands");
    }
    fmt::commands_dir()
}

/// Handle `command_list`: enumerate every valid command file.
pub fn handle_command_list(_params: &Value) -> Value {
    let dir = commands_dir();
    let commands = fmt::load_all_commands(&dir);

    let rows: Vec<Value> = commands
        .iter()
        .map(|c| {
            serde_json::json!({
                "id": c.file.id(),
                "title": c.file.title(),
                "kind": if c.file.is_script() { "script" } else { "macro" },
                "created_by": match c.file.created_by {
                    CommandAuthor::Agent => "agent",
                    CommandAuthor::User => "user",
                },
                "source_agent": c.file.source_agent,
            })
        })
        .collect();

    serde_json::json!({
        "content": [{
            "type": "text",
            "text": serde_json::to_string_pretty(&rows).unwrap_or_else(|_| "[]".into())
        }]
    })
}

/// Handle `command_create`: upsert one command.
///
/// Validation (SEC-005 class, per the design): id charset and length cap
/// inside the payload (the loader re-validates id == filename stem),
/// `source_agent` required for agent-authored files, payload size cap, and
/// refusal to overwrite a user-authored command (D4c). The atomic write is
/// `save_command_file`'s temp-file + rename, so the app's watcher only ever
/// sees complete files.
pub fn handle_command_create(params: &Value) -> Value {
    // MCP delivers the tool's inputs under params.arguments; that object IS
    // the command payload (created_by / source_agent / action), not a nested
    // "arguments" key inside it.
    let Some(arguments) = params.get("arguments") else {
        return tool_error("Missing 'arguments' in tools/call params");
    };

    let file: AgentCommandFile = match serde_json::from_value(arguments.clone()) {
        Ok(f) => f,
        Err(e) => {
            return tool_error(&format!(
                "Invalid command payload: {e}. Expected fields: created_by \
                 (\"agent\"|\"user\"), source_agent (required when agent), \
                 created_at (optional), action (one CustomActionConfig variant: \
                 shell_command | new_tab | insert_text | split_pane | \
                 key_sequence | sequence, each embedding id and title)"
            ));
        }
    };

    // Trust rule: agents writing through this tool author as agents. A
    // payload claiming user provenance from an MCP client is a lie by
    // construction — the user did not write it. Refuse it.
    if file.created_by != CommandAuthor::Agent {
        return tool_error(
            "command_create writes agent commands only: set created_by to \
             \"agent\" and provide source_agent. User commands are hand-edited \
             in the commands directory.",
        );
    }

    let dir = commands_dir();
    match fmt::save_command_file(&file, &dir) {
        Ok(path) => {
            eprintln!("[mcp-server] command_create: wrote {}", path.display());
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": format!(
                        "Created command {} ({}) at {} — the palette picks it up \
                         automatically; first run of a script command asks the \
                         user to confirm.",
                        file.id(),
                        file.title(),
                        path.display()
                    )
                }]
            })
        }
        Err(e) => tool_error(&format!("Failed to create command: {e:#}")),
    }
}

/// Handle `command_delete`: delete by id, refusing user-authored commands.
pub fn handle_command_delete(params: &Value) -> Value {
    let Some(arguments) = params.get("arguments") else {
        return tool_error("Missing 'arguments' in tools/call params");
    };
    let Some(id) = arguments.get("id").and_then(|v| v.as_str()) else {
        return tool_error("Missing 'id' (string) in tool arguments");
    };

    let dir = commands_dir();
    match fmt::delete_command_file(id, &dir) {
        Ok(()) => {
            eprintln!("[mcp-server] command_delete: removed {id}");
            serde_json::json!({
                "content": [{
                    "type": "text",
                    "text": format!("Deleted command {id}")
                }]
            })
        }
        Err(e) => tool_error(&format!("Failed to delete command: {e:#}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    fn with_temp_ipc_dir(f: impl FnOnce(&std::path::Path)) {
        let tmp = tempfile::tempdir().unwrap();
        // The commands dir resolves relative to the IPC root env var; set it
        // so load/save target the temp tree, and restore after. Mirrors the
        // unsafe set_var pattern of the config_update tests.
        let key = "PAR_TERM_IPC_DIR";
        let old = std::env::var_os(key);
        unsafe { std::env::set_var(key, tmp.path()) };
        f(tmp.path());
        unsafe {
            match old {
                Some(v) => std::env::set_var(key, v),
                None => std::env::remove_var(key),
            }
        }
    }

    #[test]
    #[serial]
    fn create_list_delete_roundtrip() {
        with_temp_ipc_dir(|_| {
            let create = serde_json::json!({
                "arguments": {
                    "created_by": "agent",
                    "source_agent": "claude-code",
                    "created_at": "2026-09-24T12:00:00Z",
                    "action": {
                        "type": "shell_command",
                        "id": "smoke-test",
                        "title": "Smoke",
                        "command": "echo",
                        "args": ["hello"]
                    }
                }
            });
            let out = handle_command_create(&create);
            assert!(out.get("isError").is_none(), "create failed: {out}");

            let list = handle_command_list(&serde_json::json!({}));
            let text = list["content"][0]["text"].as_str().unwrap();
            assert!(text.contains("smoke-test"), "list missing command: {text}");

            let del = serde_json::json!({"arguments": {"id": "smoke-test"}});
            let out = handle_command_delete(&del);
            assert!(out.get("isError").is_none(), "delete failed: {out}");

            let list = handle_command_list(&serde_json::json!({}));
            let text = list["content"][0]["text"].as_str().unwrap();
            assert_eq!(text.trim(), "[]");
        });
    }

    #[test]
    #[serial]
    fn create_refuses_user_provenance() {
        with_temp_ipc_dir(|_| {
            let create = serde_json::json!({
                "arguments": {
                    "created_by": "user",
                    "action": {
                        "type": "insert_text",
                        "id": "sneaky",
                        "title": "Sneaky",
                        "text": "x"
                    }
                }
            });
            let out = handle_command_create(&create);
            assert!(out.get("isError").is_some());
        });
    }
}
