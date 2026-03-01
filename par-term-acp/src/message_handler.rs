//! Background message handler for incoming JSON-RPC messages from the agent.
//!
//! This module contains [`handle_incoming_messages`], the async task that reads
//! incoming messages from the agent subprocess and routes them to the UI channel.
//! Separating this from [`super::agent`] makes the routing logic independently
//! testable without requiring a live agent process.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use tokio::sync::mpsc;

use super::agent::{AgentMessage};
use super::jsonrpc::{JsonRpcClient, IncomingMessage, RpcError};
use super::permissions::SafePaths;
use super::protocol::{ConfigUpdateParams, SessionUpdate, SessionUpdateParams};

/// Background task that reads incoming JSON-RPC messages from the agent and
/// routes them to the UI channel.
///
/// This function runs until the `incoming_rx` channel closes (i.e. the agent
/// subprocess exits or the JSON-RPC client is dropped).
///
/// # Routing
///
/// - `session/update` notifications → [`AgentMessage::SessionUpdate`]
/// - `session/request_permission` RPC calls → [`super::permissions::handle_permission_request`]
/// - `fs/*` RPC calls → [`super::fs_tools`] handlers
/// - `config/update` RPC calls → [`AgentMessage::ConfigUpdate`] (reply via oneshot)
/// - Unknown methods → JSON-RPC "Method not found" error response
pub async fn handle_incoming_messages(
    mut incoming_rx: mpsc::UnboundedReceiver<IncomingMessage>,
    client: Arc<JsonRpcClient>,
    ui_tx: mpsc::UnboundedSender<AgentMessage>,
    auto_approve: Arc<AtomicBool>,
    safe_paths: SafePaths,
) {
    while let Some(msg) = incoming_rx.recv().await {
        let method = match msg.method.as_deref() {
            Some(m) => m,
            None => continue,
        };

        if msg.is_notification() {
            // Handle notifications.
            match method {
                "session/update" => {
                    if let Some(params) = &msg.params {
                        // Parse the SessionUpdateParams to extract the update field.
                        if let Ok(update_params) =
                            serde_json::from_value::<SessionUpdateParams>(params.clone())
                        {
                            let update = SessionUpdate::from_value(&update_params.update);
                            let _ = ui_tx.send(AgentMessage::SessionUpdate(update));
                        } else {
                            log::error!("Failed to parse session/update params");
                        }
                    }
                }
                _ => {
                    log::error!("Unknown notification method: {method}");
                }
            }
        } else if msg.is_rpc_call() {
            // Handle RPC calls from the agent.
            let request_id = match msg.id {
                Some(id) => id,
                None => continue,
            };

            log::info!("ACP RPC call: method={method} id={request_id}");

            match method {
                "session/request_permission" => {
                    super::permissions::handle_permission_request(
                        request_id,
                        msg.params.as_ref(),
                        Arc::clone(&client),
                        &ui_tx,
                        &auto_approve,
                        &safe_paths,
                    )
                    .await;
                }
                "fs/read_text_file" | "fs/readTextFile" => {
                    super::fs_tools::handle_fs_read(
                        method,
                        request_id,
                        msg.params.clone(),
                        Arc::clone(&client),
                    )
                    .await;
                }
                "fs/write_text_file" | "fs/writeTextFile" => {
                    super::fs_tools::handle_fs_write(
                        method,
                        request_id,
                        msg.params.clone(),
                        Arc::clone(&client),
                    )
                    .await;
                }
                "fs/list_directory" | "fs/listDirectory" => {
                    super::fs_tools::handle_fs_list_directory(
                        method,
                        request_id,
                        msg.params.clone(),
                        Arc::clone(&client),
                    )
                    .await;
                }
                "fs/find" | "fs/glob" => {
                    super::fs_tools::handle_fs_find(
                        method,
                        request_id,
                        msg.params.clone(),
                        Arc::clone(&client),
                    )
                    .await;
                }
                "config/update" | "config/updateConfig" => {
                    match msg
                        .params
                        .as_ref()
                        .and_then(|p| serde_json::from_value::<ConfigUpdateParams>(p.clone()).ok())
                    {
                        Some(params) => {
                            log::info!(
                                "ACP RPC: config/update keys={:?}",
                                params.updates.keys().collect::<Vec<_>>()
                            );
                            let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
                            let _ = ui_tx.send(AgentMessage::ConfigUpdate {
                                updates: params.updates,
                                reply: reply_tx,
                            });
                            let c = Arc::clone(&client);
                            tokio::spawn(async move {
                                match reply_rx.await {
                                    Ok(Ok(())) => {
                                        log::info!("ACP config/update OK");
                                        let _ = c
                                            .respond(
                                                request_id,
                                                Some(serde_json::json!({"success": true})),
                                                None,
                                            )
                                            .await;
                                    }
                                    Ok(Err(e)) => {
                                        log::warn!("ACP config/update FAIL: {e}");
                                        let _ = c
                                            .respond(
                                                request_id,
                                                None,
                                                Some(RpcError {
                                                    code: -32000,
                                                    message: e,
                                                    data: None,
                                                }),
                                            )
                                            .await;
                                    }
                                    Err(_) => {
                                        let _ = c
                                            .respond(
                                                request_id,
                                                None,
                                                Some(RpcError {
                                                    code: -32003,
                                                    message: "Config update handler dropped"
                                                        .to_string(),
                                                    data: None,
                                                }),
                                            )
                                            .await;
                                    }
                                }
                            });
                        }
                        None => {
                            log::error!("ACP: failed to parse {method} params: {:?}", msg.params);
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
                _ => {
                    log::error!("Unknown RPC call method: {method}");
                    let _ = client
                        .respond(
                            request_id,
                            None,
                            Some(RpcError {
                                code: -32601,
                                message: format!("Method not found: {method}"),
                                data: None,
                            }),
                        )
                        .await;
                }
            }
        }
    }
}
