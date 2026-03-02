//! File-system tool handlers for ACP RPC calls.
//!
//! This module contains the async handler functions for `fs/read_text_file`,
//! `fs/write_text_file`, `fs/list_directory`, and `fs/find` / `fs/glob` RPC
//! methods received from the agent. Each handler is a standalone async function
//! that is called from the main message dispatcher in `agent.rs`.

use std::sync::Arc;

use super::jsonrpc::{JsonRpcClient, RpcError};
use super::protocol::{FsFindParams, FsListDirectoryParams, FsReadParams, FsWriteParams};

/// Handle an `fs/read_text_file` or `fs/readTextFile` RPC call.
///
/// Spawns a blocking task to read the file and responds on the JSON-RPC channel.
pub async fn handle_fs_read(
    method: &str,
    request_id: u64,
    params: Option<serde_json::Value>,
    client: Arc<JsonRpcClient>,
) {
    match params
        .as_ref()
        .and_then(|p| serde_json::from_value::<FsReadParams>(p.clone()).ok())
    {
        Some(fs_params) => {
            log::info!("ACP RPC: {method} path={}", fs_params.path);
            // Spawn independently so the handler continues processing other messages.
            tokio::spawn(async move {
                let path = fs_params.path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    super::fs_ops::read_file_with_range(
                        &fs_params.path,
                        fs_params.line,
                        fs_params.limit,
                    )
                })
                .await
                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                let (res, err) = match result {
                    Ok(text) => {
                        log::info!("ACP fs/read OK: {} ({} bytes)", path, text.len());
                        (Some(serde_json::json!({ "content": text })), None)
                    }
                    Err(e) => {
                        log::warn!("ACP fs/read FAIL: {} — {}", path, e);
                        (
                            None,
                            Some(RpcError {
                                code: -32000,
                                message: e,
                                data: None,
                            }),
                        )
                    }
                };
                let _ = client.respond(request_id, res, err).await;
            });
        }
        None => {
            log::error!("ACP: failed to parse {method} params: {:?}", params);
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

/// Handle an `fs/write_text_file` or `fs/writeTextFile` RPC call.
///
/// Spawns a blocking task to write the file and responds on the JSON-RPC channel.
pub async fn handle_fs_write(
    method: &str,
    request_id: u64,
    params: Option<serde_json::Value>,
    client: Arc<JsonRpcClient>,
) {
    match params
        .as_ref()
        .and_then(|p| serde_json::from_value::<FsWriteParams>(p.clone()).ok())
    {
        Some(fs_params) => {
            log::info!(
                "ACP RPC: {method} path={} ({} bytes)",
                fs_params.path,
                fs_params.content.len()
            );
            tokio::spawn(async move {
                let path = fs_params.path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    super::fs_ops::write_file_safe(&fs_params.path, &fs_params.content)
                })
                .await
                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                let (res, err) = match result {
                    Ok(()) => {
                        log::info!("ACP fs/write OK: {}", path);
                        (Some(serde_json::json!(null)), None)
                    }
                    Err(e) => {
                        log::warn!("ACP fs/write FAIL: {} — {}", path, e);
                        (
                            None,
                            Some(RpcError {
                                code: -32000,
                                message: e,
                                data: None,
                            }),
                        )
                    }
                };
                let _ = client.respond(request_id, res, err).await;
            });
        }
        None => {
            log::error!("ACP: failed to parse {method} params: {:?}", params);
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

/// Handle an `fs/list_directory` or `fs/listDirectory` RPC call.
///
/// Spawns a blocking task to list the directory and responds on the JSON-RPC channel.
pub async fn handle_fs_list_directory(
    method: &str,
    request_id: u64,
    params: Option<serde_json::Value>,
    client: Arc<JsonRpcClient>,
) {
    match params
        .as_ref()
        .and_then(|p| serde_json::from_value::<FsListDirectoryParams>(p.clone()).ok())
    {
        Some(fs_params) => {
            log::info!("ACP RPC: {method} path={}", fs_params.path);
            let pattern = fs_params.pattern.clone();
            tokio::spawn(async move {
                let path = fs_params.path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    super::fs_ops::list_directory_entries(&fs_params.path, pattern.as_deref())
                })
                .await
                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                let (res, err) = match result {
                    Ok(entries) => {
                        log::info!("ACP fs/list OK: {} ({} entries)", path, entries.len());
                        (Some(serde_json::json!({ "entries": entries })), None)
                    }
                    Err(e) => {
                        log::warn!("ACP fs/list FAIL: {} — {}", path, e);
                        (
                            None,
                            Some(RpcError {
                                code: -32000,
                                message: e,
                                data: None,
                            }),
                        )
                    }
                };
                let _ = client.respond(request_id, res, err).await;
            });
        }
        None => {
            log::error!("ACP: failed to parse {method} params: {:?}", params);
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

/// Handle an `fs/find` or `fs/glob` RPC call.
///
/// Spawns a blocking task to search for files and responds on the JSON-RPC channel.
pub async fn handle_fs_find(
    method: &str,
    request_id: u64,
    params: Option<serde_json::Value>,
    client: Arc<JsonRpcClient>,
) {
    match params
        .as_ref()
        .and_then(|p| serde_json::from_value::<FsFindParams>(p.clone()).ok())
    {
        Some(fs_params) => {
            log::info!("ACP RPC: {method} path={}", fs_params.path);
            tokio::spawn(async move {
                let path = fs_params.path.clone();
                let result = tokio::task::spawn_blocking(move || {
                    super::fs_ops::find_files_recursive(&fs_params.path, &fs_params.pattern)
                })
                .await
                .unwrap_or_else(|e| Err(format!("Internal error: {e}")));

                let (res, err) = match result {
                    Ok(files) => {
                        log::info!("ACP fs/find OK: {} ({} files)", path, files.len());
                        (Some(serde_json::json!({ "files": files })), None)
                    }
                    Err(e) => {
                        log::warn!("ACP fs/find FAIL: {} — {}", path, e);
                        (
                            None,
                            Some(RpcError {
                                code: -32000,
                                message: e,
                                data: None,
                            }),
                        )
                    }
                };
                let _ = client.respond(request_id, res, err).await;
            });
        }
        None => {
            log::error!("ACP: failed to parse {method} params: {:?}", params);
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
