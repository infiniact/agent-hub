use serde_json::json;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;
use tokio::time::{Duration, Instant};
use tokio_util::sync::CancellationToken;

use crate::acp::{discovery, manager as acp_manager, provisioner, transport};
use crate::db::{agent_repo, chat_tool_repo, task_run_repo};
use crate::error::{AppError, AppResult};
use crate::models::agent::AgentConfig;
use crate::models::chat_tool::{BridgeCommand, BridgeEvent};
use crate::state::AppState;

use super::manager::{self as chat_manager, check_process_alive, send_bridge_command};

/// What the event loop should do after handling an event.
enum EventAction {
    /// Continue processing events normally.
    Continue,
    /// A fatal error occurred — the bridge should be restarted.
    Restart { reason: String },
}

pub async fn run_bridge_event_loop(
    app: tauri::AppHandle,
    state: AppState,
    chat_tool_id: String,
    stdout: ChildStdout,
    cancel_token: CancellationToken,
) {
    let reader = BufReader::new(stdout);
    let mut lines = reader.lines();

    let mut last_event_time = Instant::now();
    let mut check_interval = tokio::time::interval(Duration::from_secs(45));
    // Consume the first immediate tick
    check_interval.tick().await;

    log::info!("[Bridge:{}] Event loop started", chat_tool_id);

    let mut restart_reason: Option<String> = None;

    loop {
        tokio::select! {
            _ = cancel_token.cancelled() => {
                log::info!("[Bridge:{}] Event loop cancelled", chat_tool_id);
                break;
            }
            line_result = lines.next_line() => {
                match line_result {
                    Ok(Some(line)) => {
                        let trimmed = line.trim();
                        if trimmed.is_empty() {
                            continue;
                        }
                        // Any valid line from the bridge counts as activity
                        last_event_time = Instant::now();
                        match serde_json::from_str::<BridgeEvent>(trimmed) {
                            Ok(event) => {
                                match handle_bridge_event(
                                    &app,
                                    &state,
                                    &chat_tool_id,
                                    event,
                                ).await {
                                    Ok(EventAction::Continue) => {}
                                    Ok(EventAction::Restart { reason }) => {
                                        restart_reason = Some(reason);
                                        break;
                                    }
                                    Err(e) => {
                                        log::error!("[Bridge:{}] Error handling event: {}", chat_tool_id, e);
                                    }
                                }
                            }
                            Err(e) => {
                                log::warn!("[Bridge:{}] Failed to parse NDJSON: '{}', error: {}", chat_tool_id, trimmed, e);
                            }
                        }
                    }
                    Ok(None) => {
                        log::info!("[Bridge:{}] stdout closed, ending event loop", chat_tool_id);
                        let state_clone = state.clone();
                        let id = chat_tool_id.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            chat_tool_repo::update_chat_tool_status(&state_clone, &id, "stopped", Some("Bridge process exited"))
                        }).await;
                        let _ = app.emit("chat_tool:status_changed", json!({
                            "chatToolId": chat_tool_id,
                            "status": "stopped",
                            "message": "Bridge process exited"
                        }));
                        break;
                    }
                    Err(e) => {
                        log::error!("[Bridge:{}] Error reading stdout: {}", chat_tool_id, e);
                        break;
                    }
                }
            }
            _ = check_interval.tick() => {
                let elapsed = last_event_time.elapsed();
                if elapsed > Duration::from_secs(90) {
                    log::warn!(
                        "[Bridge:{}] No activity for {:.0}s, sending ping",
                        chat_tool_id,
                        elapsed.as_secs_f64()
                    );

                    // Send a Ping command to the bridge
                    let ts = chrono::Utc::now().timestamp();
                    let ping_cmd = BridgeCommand::Ping { ts };
                    let ping_sent = {
                        let processes = state.chat_tool_processes.lock().await;
                        if let Some(process) = processes.get(&chat_tool_id) {
                            send_bridge_command(process, &ping_cmd).await.is_ok()
                        } else {
                            false
                        }
                    };

                    if !ping_sent {
                        log::error!("[Bridge:{}] Failed to send ping, ending event loop", chat_tool_id);
                        break;
                    }

                    // Wait up to 10 seconds for a Pong (any line from stdout)
                    let pong_received = tokio::time::timeout(
                        Duration::from_secs(10),
                        wait_for_pong(&mut lines, &app, &state, &chat_tool_id),
                    )
                    .await;

                    match pong_received {
                        Ok(WaitResult::Pong) => {
                            log::info!("[Bridge:{}] Pong received, bridge is alive", chat_tool_id);
                            last_event_time = Instant::now();
                        }
                        Ok(WaitResult::StreamClosed) => {
                            log::info!("[Bridge:{}] stdout closed while waiting for pong", chat_tool_id);
                            let state_clone = state.clone();
                            let id = chat_tool_id.clone();
                            let _ = tokio::task::spawn_blocking(move || {
                                chat_tool_repo::update_chat_tool_status(
                                    &state_clone, &id, "stopped",
                                    Some("Bridge process exited"),
                                )
                            }).await;
                            let _ = app.emit("chat_tool:status_changed", json!({
                                "chatToolId": chat_tool_id,
                                "status": "stopped",
                                "message": "Bridge process exited"
                            }));
                            break;
                        }
                        Ok(WaitResult::Error) | Err(_) => {
                            // Timeout or read error — check if process is actually alive
                            let process_alive = {
                                let mut processes = state.chat_tool_processes.lock().await;
                                if let Some(process) = processes.get_mut(&chat_tool_id) {
                                    check_process_alive(process)
                                } else {
                                    false
                                }
                            };

                            if !process_alive {
                                log::warn!("[Bridge:{}] Process already exited", chat_tool_id);
                                let state_clone = state.clone();
                                let id = chat_tool_id.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    chat_tool_repo::update_chat_tool_status(
                                        &state_clone, &id, "stopped",
                                        Some("Bridge process exited unexpectedly"),
                                    )
                                }).await;
                                let _ = app.emit("chat_tool:status_changed", json!({
                                    "chatToolId": chat_tool_id,
                                    "status": "stopped",
                                    "message": "Bridge process exited unexpectedly"
                                }));
                            } else {
                                log::error!("[Bridge:{}] Bridge unresponsive, killing process", chat_tool_id);
                                let state_clone = state.clone();
                                let id = chat_tool_id.clone();
                                let _ = tokio::task::spawn_blocking(move || {
                                    chat_tool_repo::update_chat_tool_status(
                                        &state_clone, &id, "error",
                                        Some("Bridge unresponsive"),
                                    )
                                }).await;
                                let _ = app.emit("chat_tool:status_changed", json!({
                                    "chatToolId": chat_tool_id,
                                    "status": "error",
                                    "message": "Bridge unresponsive"
                                }));

                                // Kill the unresponsive process
                                let mut processes = state.chat_tool_processes.lock().await;
                                if let Some(process) = processes.get_mut(&chat_tool_id) {
                                    let _ = process.child.kill().await;
                                }
                            }
                            break;
                        }
                    }
                }
            }
        }
    }

    log::info!("[Bridge:{}] Event loop ended", chat_tool_id);

    // Auto-restart if needed
    if let Some(reason) = restart_reason {
        if cancel_token.is_cancelled() {
            log::info!("[Bridge:{}] Skip restart — already cancelled", chat_tool_id);
            return;
        }

        log::info!(
            "[Bridge:{}] Auto-restarting bridge (reason: {})",
            chat_tool_id, reason
        );

        // 1. Kill old bridge process properly
        {
            let mut processes = state.chat_tool_processes.lock().await;
            if let Some(mut process) = processes.remove(&chat_tool_id) {
                let _ = chat_manager::stop_bridge_process(&mut process).await;
            }
        }

        // 2. Update status to "restarting"
        {
            let state_clone = state.clone();
            let id = chat_tool_id.clone();
            let r = reason.clone();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(
                    &state_clone, &id, "starting",
                    Some(&format!("Restarting: {}", r)),
                )
            })
            .await;
        }
        let _ = app.emit("chat_tool:status_changed", json!({
            "chatToolId": chat_tool_id,
            "status": "starting",
            "message": format!("Restarting: {}", reason)
        }));

        // 3. Wait a bit before restarting (allow logout to complete)
        tokio::time::sleep(Duration::from_secs(3)).await;

        if cancel_token.is_cancelled() {
            return;
        }

        // 4. Fetch latest chat tool config from DB
        let chat_tool = {
            let state_clone = state.clone();
            let id = chat_tool_id.clone();
            tokio::task::spawn_blocking(move || {
                chat_tool_repo::get_chat_tool(&state_clone, &id)
            })
            .await
        };

        let chat_tool = match chat_tool {
            Ok(Ok(ct)) => ct,
            _ => {
                log::error!("[Bridge:{}] Failed to fetch chat tool config for restart", chat_tool_id);
                return;
            }
        };

        // 5. Spawn new bridge process
        let spawn_result = chat_manager::spawn_bridge(
            &chat_tool_id, &chat_tool.plugin_type, &chat_tool.config_json,
        ).await;

        match spawn_result {
            Ok((process, new_stdout)) => {
                {
                    let mut processes = state.chat_tool_processes.lock().await;
                    processes.insert(chat_tool_id.clone(), process);
                }

                log::info!("[Bridge:{}] Bridge restarted successfully", chat_tool_id);

                // 6. Run new event loop recursively (tail call)
                Box::pin(run_bridge_event_loop(
                    app, state, chat_tool_id, new_stdout, cancel_token,
                )).await;
            }
            Err(e) => {
                log::error!("[Bridge:{}] Failed to restart bridge: {}", chat_tool_id, e);
                let state_clone = state.clone();
                let id = chat_tool_id.clone();
                let err = e.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    chat_tool_repo::update_chat_tool_status(
                        &state_clone, &id, "error", Some(&err),
                    )
                })
                .await;
                let _ = app.emit("chat_tool:status_changed", json!({
                    "chatToolId": chat_tool_id,
                    "status": "error",
                    "message": e.to_string()
                }));
            }
        }
    }
}

enum WaitResult {
    Pong,
    StreamClosed,
    Error,
}

/// Read lines from stdout until we get a Pong event.
/// Any other valid events are dispatched normally.
async fn wait_for_pong(
    lines: &mut tokio::io::Lines<BufReader<ChildStdout>>,
    app: &tauri::AppHandle,
    state: &AppState,
    chat_tool_id: &str,
) -> WaitResult {
    loop {
        match lines.next_line().await {
            Ok(Some(line)) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<BridgeEvent>(trimmed) {
                    Ok(BridgeEvent::Pong { .. }) => {
                        return WaitResult::Pong;
                    }
                    Ok(event) => {
                        // Process other events normally while waiting
                        match handle_bridge_event(app, state, chat_tool_id, event).await {
                            Ok(EventAction::Restart { .. }) => {
                                // Treat restart-triggering errors as pong failure
                                return WaitResult::Error;
                            }
                            Err(e) => {
                                log::error!("[Bridge:{}] Error handling event during pong wait: {}", chat_tool_id, e);
                            }
                            _ => {}
                        }
                    }
                    Err(e) => {
                        log::warn!("[Bridge:{}] Failed to parse NDJSON during pong wait: '{}', error: {}", chat_tool_id, trimmed, e);
                    }
                }
            }
            Ok(None) => return WaitResult::StreamClosed,
            Err(_) => return WaitResult::Error,
        }
    }
}

async fn handle_bridge_event(
    app: &tauri::AppHandle,
    state: &AppState,
    chat_tool_id: &str,
    event: BridgeEvent,
) -> AppResult<EventAction> {
    match event {
        BridgeEvent::Status { status } => {
            log::info!("[Bridge:{}] Status: {}", chat_tool_id, status);
            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let s = status.clone();
            tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(&state_clone, &id, &s, None)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            let _ = app.emit(
                "chat_tool:status_changed",
                json!({
                    "chatToolId": chat_tool_id,
                    "status": status,
                    "message": serde_json::Value::Null
                }),
            );
        }

        BridgeEvent::Qrcode { url, image_base64 } => {
            log::info!("[Bridge:{}] QR code received", chat_tool_id);

            {
                let mut qr_codes = state.chat_tool_qr_codes.lock().await;
                qr_codes.insert(chat_tool_id.to_string(), image_base64.clone());
            }

            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(
                    &state_clone,
                    &id,
                    "login_required",
                    Some("Scan QR code to login"),
                )
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            let _ = app.emit(
                "chat_tool:qr_code",
                json!({
                    "chatToolId": chat_tool_id,
                    "url": url,
                    "imageBase64": image_base64
                }),
            );
        }

        BridgeEvent::Login { user_id, user_name } => {
            log::info!(
                "[Bridge:{}] Login: user_id={}, user_name={}",
                chat_tool_id,
                user_id,
                user_name
            );

            {
                let mut qr_codes = state.chat_tool_qr_codes.lock().await;
                qr_codes.remove(chat_tool_id);
            }

            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let name = user_name.clone();
            tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(
                    &state_clone,
                    &id,
                    "running",
                    Some(&format!("Logged in as {}", name)),
                )
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            let _ = app.emit(
                "chat_tool:login",
                json!({
                    "chatToolId": chat_tool_id,
                    "userId": user_id,
                    "userName": user_name
                }),
            );
        }

        BridgeEvent::Logout => {
            log::info!("[Bridge:{}] Logout", chat_tool_id);

            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(
                    &state_clone,
                    &id,
                    "stopped",
                    Some("Logged out"),
                )
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            let _ = app.emit(
                "chat_tool:logout",
                json!({ "chatToolId": chat_tool_id }),
            );
        }

        BridgeEvent::Message {
            message_id: _,
            sender_id,
            sender_name,
            content,
            content_type,
        } => {
            log::info!(
                "[Bridge:{}] Message from {} ({})",
                chat_tool_id,
                sender_name,
                sender_id,
            );

            // Check if the sender is blocked
            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let sid = sender_id.clone();
            let is_blocked = tokio::task::spawn_blocking(move || -> bool {
                let db = match state_clone.db.lock() {
                    Ok(db) => db,
                    Err(_) => return false,
                };
                db.query_row(
                    "SELECT is_blocked FROM chat_tool_contacts WHERE chat_tool_id = ?1 AND external_id = ?2",
                    rusqlite::params![id, sid],
                    |row| row.get::<_, i32>(0),
                )
                .map(|v| v != 0)
                .unwrap_or(false)
            })
            .await
            .unwrap_or(false);

            if is_blocked {
                log::info!("[Bridge:{}] Message from blocked contact {}, skipping", chat_tool_id, sender_id);
                return Ok(EventAction::Continue);
            }

            // Save message to DB (is_processed defaults to false)
            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let sid = sender_id.clone();
            let sname = sender_name.clone();
            let c = content.clone();
            let ct = content_type;
            let message = tokio::task::spawn_blocking(move || {
                chat_tool_repo::save_chat_tool_message(
                    &state_clone, &id, "incoming",
                    Some(&sid), Some(&sname), &c, &ct,
                )
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            // Increment received count
            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::increment_message_count(&state_clone, &id, "incoming")
            })
            .await;

            let _ = app.emit(
                "chat_tool:message_received",
                json!({
                    "chatToolId": chat_tool_id,
                    "message": message
                }),
            );

            // Check auto-reply mode
            let state_clone = state.clone();
            let ct_id = chat_tool_id.to_string();
            let chat_tool = tokio::task::spawn_blocking(move || {
                chat_tool_repo::get_chat_tool(&state_clone, &ct_id)
            })
            .await
            .map_err(|e| AppError::Internal(e.to_string()))??;

            if chat_tool.auto_reply_mode == "none" {
                return Ok(EventAction::Continue);
            }

            // Check if this chat tool is already processing a message
            {
                let processing = state.chat_tool_processing.lock().await;
                if processing.contains(chat_tool_id) {
                    log::info!(
                        "[Bridge:{}] Already processing, queuing message and sending busy reply",
                        chat_tool_id
                    );
                    // Send busy reply (release processing lock first)
                    drop(processing);
                    let processes = state.chat_tool_processes.lock().await;
                    if let Some(process) = processes.get(chat_tool_id) {
                        let cmd = BridgeCommand::SendMessage {
                            to_id: sender_id.clone(),
                            content: "正在处理前面的消息，请稍候".to_string(),
                            content_type: "text".into(),
                        };
                        let _ = send_bridge_command(process, &cmd).await;
                    }
                    return Ok(EventAction::Continue);
                }
            }

            // Mark as processing
            {
                let mut processing = state.chat_tool_processing.lock().await;
                processing.insert(chat_tool_id.to_string());
            }

            // Spawn message processing as a background task so the event loop
            // continues reading bridge events (heartbeats, new messages, etc.).
            // If we awaited here, the bridge's stdout pipe buffer would fill up
            // and the bridge process would block, unable to receive new messages.
            let bg_app = app.clone();
            let bg_state = state.clone();
            let bg_id = chat_tool_id.to_string();
            let bg_name = chat_tool.name.clone();
            let bg_ws = chat_tool.workspace_id.clone();
            tokio::spawn(async move {
                process_message_queue(
                    &bg_app, &bg_state, &bg_id, &bg_name, bg_ws.as_deref(),
                )
                .await;

                // Done processing — remove from processing set
                let mut processing = bg_state.chat_tool_processing.lock().await;
                processing.remove(&bg_id);
            });
        }

        BridgeEvent::Contacts { contacts } => {
            log::info!(
                "[Bridge:{}] Received {} contacts",
                chat_tool_id,
                contacts.len()
            );

            let contact_data: Vec<(String, String, Option<String>, String)> = contacts
                .into_iter()
                .map(|c| (c.id, c.name, c.avatar_url, c.contact_type))
                .collect();

            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::upsert_contacts(&state_clone, &id, &contact_data)
            })
            .await;
        }

        BridgeEvent::Error { error } => {
            log::error!("[Bridge:{}] Error: {}", chat_tool_id, error);

            // Detect fatal errors that require a full restart
            let is_fatal = error.contains("timeout")
                || error.contains("超时")
                || error.contains("must logout first")
                || error.contains("尝试重启")
                || error.contains("Unhandled rejection")
                || error.contains("ECONNRESET")
                || error.contains("socket hang up");

            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let err = error.clone();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_chat_tool_status(&state_clone, &id, "error", Some(&err))
            })
            .await;

            let _ = app.emit(
                "chat_tool:error",
                json!({
                    "chatToolId": chat_tool_id,
                    "error": error
                }),
            );

            if is_fatal {
                return Ok(EventAction::Restart { reason: error });
            }
        }

        BridgeEvent::Heartbeat => {
            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_last_active(&state_clone, &id)
            })
            .await;
        }

        BridgeEvent::Pong { ts } => {
            log::debug!("[Bridge:{}] Pong received (ts={})", chat_tool_id, ts);
            // last_event_time is updated at the line-read level; nothing else needed here.
            let state_clone = state.clone();
            let id = chat_tool_id.to_string();
            let _ = tokio::task::spawn_blocking(move || {
                chat_tool_repo::update_last_active(&state_clone, &id)
            })
            .await;
        }
    }

    Ok(EventAction::Continue)
}

/// Process the queue of unprocessed messages for a chat tool.
///
/// Loops until no more unprocessed messages remain:
/// 1. Fetch all unprocessed incoming messages
/// 2. Merge them into a single prompt (grouped by sender)
/// 3. Send to Control Hub for processing
/// 4. Mark the batch as processed
/// 5. Reply to each sender
/// 6. Check for newly arrived messages and repeat if any
async fn process_message_queue(
    app: &tauri::AppHandle,
    state: &AppState,
    chat_tool_id: &str,
    chat_tool_name: &str,
    workspace_id: Option<&str>,
) {
    loop {
        // 1. Fetch unprocessed messages
        let state_clone = state.clone();
        let ct_id = chat_tool_id.to_string();
        let unprocessed = tokio::task::spawn_blocking(move || {
            chat_tool_repo::list_unprocessed_messages(&state_clone, &ct_id)
        })
        .await;

        let messages = match unprocessed {
            Ok(Ok(msgs)) if !msgs.is_empty() => msgs,
            _ => {
                log::info!("[Bridge:{}] No more unprocessed messages", chat_tool_id);
                break;
            }
        };

        log::info!(
            "[Bridge:{}] Processing batch of {} unprocessed messages",
            chat_tool_id,
            messages.len()
        );

        // 2. Merge messages into a single prompt
        let mut prompt_parts: Vec<String> = Vec::new();
        let mut sender_ids: Vec<String> = Vec::new(); // track unique senders for reply
        let mut message_ids: Vec<String> = Vec::new();

        for msg in &messages {
            let sender = msg
                .external_sender_name
                .as_deref()
                .unwrap_or("Unknown");
            prompt_parts.push(format!("[Message from {}]: {}", sender, msg.content));
            message_ids.push(msg.id.clone());

            if let Some(sid) = &msg.external_sender_id {
                if !sender_ids.contains(sid) {
                    sender_ids.push(sid.clone());
                }
            }
        }

        let merged_prompt = prompt_parts.join("\n\n");

        // 3. Send to Control Hub
        let agent_reply = forward_to_control_hub(
            app,
            state,
            chat_tool_id,
            chat_tool_name,
            workspace_id,
            &merged_prompt,
        )
        .await;

        match agent_reply {
            Ok(Some(reply)) => {
                // 4. Mark batch as processed
                let state_clone = state.clone();
                let mids = message_ids.clone();
                let r = reply.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    chat_tool_repo::mark_messages_processed_batch(&state_clone, &mids, &r)
                })
                .await;

                // 5. Send reply to each sender through bridge
                for sid in &sender_ids {
                    let processes = state.chat_tool_processes.lock().await;
                    if let Some(process) = processes.get(chat_tool_id) {
                        let cmd = BridgeCommand::SendMessage {
                            to_id: sid.clone(),
                            content: reply.clone(),
                            content_type: "text".into(),
                        };
                        if let Err(e) = send_bridge_command(process, &cmd).await {
                            log::error!(
                                "[Bridge:{}] Failed to send reply to {}: {}",
                                chat_tool_id, sid, e
                            );
                        }
                    }
                }

                // Increment sent count
                let state_clone = state.clone();
                let id = chat_tool_id.to_string();
                let _ = tokio::task::spawn_blocking(move || {
                    chat_tool_repo::increment_message_count(&state_clone, &id, "outgoing")
                })
                .await;

                // Save outgoing message
                let state_clone = state.clone();
                let id = chat_tool_id.to_string();
                let first_sender = sender_ids.first().cloned();
                let r2 = reply.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    chat_tool_repo::save_chat_tool_message(
                        &state_clone,
                        &id,
                        "outgoing",
                        first_sender.as_deref(),
                        None,
                        &r2,
                        "text",
                    )
                })
                .await;

                // Emit processed events for each message in batch
                for mid in &message_ids {
                    let _ = app.emit(
                        "chat_tool:message_processed",
                        json!({
                            "chatToolId": chat_tool_id,
                            "messageId": mid,
                            "agentResponse": reply
                        }),
                    );
                }
            }
            Ok(None) => {
                // No Control Hub available — skip, messages stay unprocessed
                log::info!("[Bridge:{}] No Control Hub, skipping batch", chat_tool_id);
                break;
            }
            Err(e) => {
                log::error!(
                    "[Bridge:{}] Control Hub reply failed for batch: {}",
                    chat_tool_id, e
                );
                // Mark all messages with error
                for mid in &message_ids {
                    let state_clone = state.clone();
                    let mid_clone = mid.clone();
                    let err = e.to_string();
                    let _ = tokio::task::spawn_blocking(move || {
                        chat_tool_repo::mark_message_error(&state_clone, &mid_clone, &err)
                    })
                    .await;
                }
                break;
            }
        }

        // 6. Loop back to check for new unprocessed messages that arrived during processing
    }
}

/// Forward a message to the workspace's Control Hub agent and collect the full text response.
///
/// Returns `Ok(None)` if no Control Hub is configured or it is not running — the caller
/// should silently skip auto-reply in that case.
///
/// Maintains a persistent ACP session per chat_tool_id so follow-up messages
/// share context. If the session becomes invalid, a new one is created automatically.
/// Reuses a single TaskRun per chat tool to track all message processing.
async fn forward_to_control_hub(
    app: &tauri::AppHandle,
    state: &AppState,
    chat_tool_id: &str,
    chat_tool_name: &str,
    workspace_id: Option<&str>,
    prompt_text: &str,
) -> AppResult<Option<String>> {
    use crate::acp::transport;

    // 1. Find the Control Hub agent for this workspace
    let state_clone = state.clone();
    let ws_id = workspace_id.map(|s| s.to_string());
    let hub = tokio::task::spawn_blocking(move || {
        agent_repo::get_control_hub(&state_clone, ws_id.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    let hub = match hub {
        Some(h) => h,
        None => {
            log::info!(
                "[Bridge:{}] No Control Hub configured for workspace {:?}, skipping auto-reply",
                chat_tool_id, workspace_id
            );
            return Ok(None);
        }
    };
    let agent_id = hub.id.clone();

    // 2. Ensure the Control Hub process is running (auto-start if needed)
    if let Err(e) = ensure_control_hub_running(app, state, &hub).await {
        log::warn!(
            "[Bridge:{}] Cannot start Control Hub agent {}: {}",
            chat_tool_id, agent_id, e
        );
        return Ok(None);
    }

    // 3. Get or create a persistent TaskRun for this chat tool (reuse across messages)
    let task_run_id = get_or_create_task_run(app, state, chat_tool_id, chat_tool_name, &hub.id, workspace_id, prompt_text).await?;

    // Update task run: set prompt to latest merged content
    {
        let state_clone = state.clone();
        let trid = task_run_id.clone();
        let pt = prompt_text.to_string();
        let _ = tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_prompt(&state_clone, &trid, &pt)
        })
        .await;
    }

    // Update task run status to running
    {
        let state_clone = state.clone();
        let trid = task_run_id.clone();
        let _ = tokio::task::spawn_blocking(move || {
            task_run_repo::update_task_run_status(&state_clone, &trid, "running")
        })
        .await;
    }

    // 4. Get or create an ACP session for this chat tool
    let acp_session_id = get_or_create_session(state, chat_tool_id, &agent_id).await?;

    // 5. Send prompt
    let request_id = chrono::Utc::now().timestamp_millis();
    let req = transport::build_request(
        request_id,
        "session/prompt",
        Some(json!({
            "sessionId": acp_session_id,
            "prompt": [{
                "type": "text",
                "text": prompt_text
            }]
        })),
    );

    {
        let mut processes = state.agent_processes.lock().await;
        let process = processes.get_mut(&agent_id).ok_or_else(|| {
            AppError::AgentNotRunning("Control Hub agent not running".into())
        })?;
        transport::send_message(process, &req).await?;
    }

    // 6. Collect response with timeout
    let collected_text = collect_response(state, &agent_id, request_id).await;

    match &collected_text {
        Ok(text) => {
            // Update task run as completed with summary
            let state_clone = state.clone();
            let trid = task_run_id.clone();
            let _ = tokio::task::spawn_blocking(move || {
                task_run_repo::update_task_run_status(&state_clone, &trid, "completed")
            })
            .await;

            let summary = text.clone();
            let state_clone = state.clone();
            let trid = task_run_id.clone();
            let _ = tokio::task::spawn_blocking(move || {
                task_run_repo::update_task_run_summary(&state_clone, &trid, &summary)
            })
            .await;

            let _ = app.emit("orchestration:task_run_updated", json!({
                "taskRunId": task_run_id,
                "status": "completed"
            }));
        }
        Err(e) => {
            // 7. If session error, clear old session and retry once
            let err_msg = e.to_string();
            if err_msg.contains("session") || err_msg.contains("timed out") || err_msg.contains("channel closed") {
                log::warn!(
                    "[Bridge:{}] Session may be stale ({}), clearing and retrying",
                    chat_tool_id, err_msg
                );
                // Clear the old session
                {
                    let mut sessions = state.chat_tool_acp_sessions.lock().await;
                    sessions.remove(chat_tool_id);
                }

                // Create a fresh session and retry
                let new_session_id = get_or_create_session(state, chat_tool_id, &agent_id).await?;
                let retry_req_id = chrono::Utc::now().timestamp_millis();
                let retry_req = transport::build_request(
                    retry_req_id,
                    "session/prompt",
                    Some(json!({
                        "sessionId": new_session_id,
                        "prompt": [{
                            "type": "text",
                            "text": prompt_text
                        }]
                    })),
                );

                {
                    let mut processes = state.agent_processes.lock().await;
                    let process = processes.get_mut(&agent_id).ok_or_else(|| {
                        AppError::AgentNotRunning("Control Hub agent not running".into())
                    })?;
                    transport::send_message(process, &retry_req).await?;
                }

                let retry_result = collect_response(state, &agent_id, retry_req_id).await;
                match &retry_result {
                    Ok(text) => {
                        let state_clone = state.clone();
                        let trid = task_run_id.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            task_run_repo::update_task_run_status(&state_clone, &trid, "completed")
                        })
                        .await;

                        let summary = text.clone();
                        let state_clone = state.clone();
                        let trid = task_run_id.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            task_run_repo::update_task_run_summary(&state_clone, &trid, &summary)
                        })
                        .await;

                        let _ = app.emit("orchestration:task_run_updated", json!({
                            "taskRunId": task_run_id,
                            "status": "completed"
                        }));
                    }
                    Err(_) => {
                        let state_clone = state.clone();
                        let trid = task_run_id.clone();
                        let _ = tokio::task::spawn_blocking(move || {
                            task_run_repo::update_task_run_status(&state_clone, &trid, "failed")
                        })
                        .await;

                        let _ = app.emit("orchestration:task_run_updated", json!({
                            "taskRunId": task_run_id,
                            "status": "failed"
                        }));
                    }
                }
                return retry_result.map(Some);
            } else {
                // Non-retryable error — mark task run as failed
                let state_clone = state.clone();
                let trid = task_run_id.clone();
                let _ = tokio::task::spawn_blocking(move || {
                    task_run_repo::update_task_run_status(&state_clone, &trid, "failed")
                })
                .await;

                let _ = app.emit("orchestration:task_run_updated", json!({
                    "taskRunId": task_run_id,
                    "status": "failed"
                }));
            }
        }
    }

    collected_text.map(Some)
}

/// Ensure the Control Hub agent process is running. If not, spawn and initialize it.
///
/// Returns `Ok(())` if the process is already running or was successfully started.
/// Returns `Err` if the agent has no ACP command or fails to start.
async fn ensure_control_hub_running(
    app: &tauri::AppHandle,
    state: &AppState,
    agent: &AgentConfig,
) -> AppResult<()> {
    let agent_id = &agent.id;

    // Already running?
    {
        let processes = state.agent_processes.lock().await;
        if processes.contains_key(agent_id) {
            return Ok(());
        }
    }

    log::info!(
        "[Bridge] Control Hub agent {} not running, auto-starting...",
        agent_id
    );

    let acp_command = agent.acp_command.clone().ok_or_else(|| {
        AppError::Internal(format!(
            "Control Hub agent {} has no ACP command configured",
            agent_id
        ))
    })?;

    let args: Vec<String> = agent
        .acp_args_json
        .as_ref()
        .and_then(|j| serde_json::from_str(j).ok())
        .unwrap_or_default();

    // Resolve command via provisioner
    let resolved = provisioner::resolve_agent_command(&acp_command, &args).await?;

    log::info!(
        "[Bridge] Spawning Control Hub: command={}, args={:?}, agent_type={}",
        resolved.command, resolved.args, resolved.agent_type
    );

    // Build extra environment variables
    let mut extra_env = discovery::get_agent_env_for_command(&resolved.agent_type).await;
    {
        let discovered = state.discovered_agents.lock().await;
        let cmd_basename = std::path::Path::new(&acp_command)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(&acp_command);
        if let Some(matched) = discovered.iter().find(|d| {
            let d_basename = std::path::Path::new(&d.command)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&d.command);
            d_basename == cmd_basename
        }) {
            if let Ok(config_env) =
                serde_json::from_str::<std::collections::HashMap<String, String>>(&matched.env_json)
            {
                extra_env.extend(config_env);
            }
        }
    }

    // Spawn process
    let process = acp_manager::spawn_agent_process(
        agent_id,
        &resolved.command,
        &resolved.args,
        &extra_env,
        &resolved.agent_type,
    )
    .await?;

    let stdin_handle = process.stdin.clone();

    {
        let mut processes = state.agent_processes.lock().await;
        processes.insert(agent_id.to_string(), process);
    }
    {
        let mut stdins = state.agent_stdins.lock().await;
        stdins.insert(agent_id.to_string(), stdin_handle);
    }

    // Initialize ACP protocol
    let init_req = transport::build_request(
        1,
        "initialize",
        Some(json!({
            "protocolVersion": 1,
            "clientInfo": {
                "name": "IAAgentHub",
                "version": "0.1.0"
            },
            "clientCapabilities": {
                "fs": {
                    "readTextFile": false,
                    "writeTextFile": false
                },
                "terminal": false
            }
        })),
    );

    {
        let mut processes = state.agent_processes.lock().await;
        if let Some(process) = processes.get_mut(agent_id) {
            transport::send_message(process, &init_req).await?;
        }
    }

    // Wait for initialize response
    let init_deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
    let init_response = loop {
        let recv_result = {
            let mut processes = state.agent_processes.lock().await;
            match processes.get_mut(agent_id) {
                Some(process) => process.message_rx.try_recv(),
                None => {
                    return Err(AppError::Internal(format!(
                        "Control Hub agent {} disappeared during init",
                        agent_id
                    )));
                }
            }
        };
        match recv_result {
            Ok(msg) => {
                if let Some(msg_id) = msg.get("id") {
                    if msg_id == &serde_json::json!(1) {
                        break msg;
                    }
                }
                // Skip non-matching messages during init
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                if std::time::Instant::now() >= init_deadline {
                    return Err(AppError::Transport(
                        "Timeout (120s) waiting for Control Hub initialization".into(),
                    ));
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                return Err(AppError::Transport(
                    "Control Hub message channel disconnected during initialization".into(),
                ));
            }
        }
    };

    // Check for error in initialize response
    if let Some(error) = init_response.get("error") {
        let msg = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Unknown error");
        return Err(AppError::Internal(format!(
            "Control Hub initialize failed: {}",
            msg
        )));
    }

    log::info!(
        "[Bridge] Control Hub agent {} initialized successfully",
        agent_id
    );

    // Emit event so frontend shows the agent as running
    let _ = app.emit(
        "acp:agent_started",
        &json!({
            "agent_id": agent_id,
            "status": "Running"
        }),
    );

    Ok(())
}

/// Get the existing TaskRun for a chat tool, or create a new one.
/// Reuses a single TaskRun per chat tool so all messages share the same orchestration entry.
async fn get_or_create_task_run(
    app: &tauri::AppHandle,
    state: &AppState,
    chat_tool_id: &str,
    chat_tool_name: &str,
    agent_id: &str,
    workspace_id: Option<&str>,
    prompt_preview: &str,
) -> AppResult<String> {
    let existing = {
        let runs = state.chat_tool_task_runs.lock().await;
        runs.get(chat_tool_id).cloned()
    };

    if let Some(id) = existing {
        return Ok(id);
    }

    let new_id = uuid::Uuid::new_v4().to_string();
    let title = format!("Chat: {}", chat_tool_name);
    let state_clone = state.clone();
    let nid = new_id.clone();
    let aid = agent_id.to_string();
    let ws = workspace_id.map(|s| s.to_string());
    let t = title.clone();
    let pt = prompt_preview.to_string();
    let task_run = tokio::task::spawn_blocking(move || {
        task_run_repo::create_task_run(
            &state_clone, &nid, &t, &pt, &aid, "running", ws.as_deref(),
        )
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))??;

    {
        let mut runs = state.chat_tool_task_runs.lock().await;
        runs.insert(chat_tool_id.to_string(), new_id.clone());
    }

    let _ = app.emit(
        "orchestration:task_run_created",
        json!({ "taskRun": task_run }),
    );

    Ok(new_id)
}

/// Get the existing ACP session for a chat tool, or create a new one.
async fn get_or_create_session(
    state: &AppState,
    chat_tool_id: &str,
    agent_id: &str,
) -> AppResult<String> {
    use crate::acp::transport;

    // Check if we already have a session for this chat tool
    let existing = {
        let sessions = state.chat_tool_acp_sessions.lock().await;
        sessions.get(chat_tool_id).cloned()
    };

    if let Some(session_id) = existing {
        // Verify the session is still usable in acp_sessions
        let still_valid = {
            let acp_sessions = state.acp_sessions.lock().await;
            acp_sessions.values().any(|s| s.acp_session_id == session_id && s.is_usable())
        };
        if still_valid {
            return Ok(session_id);
        }
        // Session expired, remove it
        let mut sessions = state.chat_tool_acp_sessions.lock().await;
        sessions.remove(chat_tool_id);
    }

    // Create a new ACP session
    log::info!("[Bridge:{}] Creating new ACP session via Control Hub {}", chat_tool_id, agent_id);
    let request_id = chrono::Utc::now().timestamp_millis() + 1; // offset to avoid collision
    let req = transport::build_request(
        request_id,
        "session/new",
        Some(json!({
            "cwd": ".",
            "mcpServers": []
        })),
    );

    {
        let mut processes = state.agent_processes.lock().await;
        let process = processes.get_mut(agent_id).ok_or_else(|| {
            AppError::AgentNotRunning("Control Hub agent not running".into())
        })?;
        transport::send_message(process, &req).await?;
    }

    // Wait for session/new response
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(90);
    let response = loop {
        if std::time::Instant::now() >= deadline {
            return Err(AppError::Internal("Timeout waiting for session/new response".into()));
        }

        let recv_result = {
            let mut processes = state.agent_processes.lock().await;
            let process = processes.get_mut(agent_id).ok_or_else(|| {
                AppError::AgentNotRunning("Control Hub agent not running".into())
            })?;
            process.message_rx.try_recv()
        };

        match recv_result {
            Ok(msg) => {
                if let Some(id) = msg.get("id") {
                    if id.as_i64() == Some(request_id) {
                        break msg;
                    }
                }
                // Skip notifications
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                return Err(AppError::Internal("Agent channel closed during session creation".into()));
            }
        }
    };

    // Check for error
    if let Some(error) = response.get("error") {
        let msg = error.get("message").and_then(|m| m.as_str()).unwrap_or("Unknown error");
        return Err(AppError::Internal(format!("session/new failed: {}", msg)));
    }

    let result = response.get("result").ok_or_else(|| {
        AppError::Internal("No result in session/new response".into())
    })?;
    let session_id = result
        .get("sessionId")
        .and_then(|s| s.as_str())
        .ok_or_else(|| AppError::Internal("No sessionId in session/new response".into()))?
        .to_string();

    log::info!("[Bridge:{}] Created ACP session: {}", chat_tool_id, session_id);

    // Store the session mapping
    {
        let mut sessions = state.chat_tool_acp_sessions.lock().await;
        sessions.insert(chat_tool_id.to_string(), session_id.clone());
    }

    // Also register in acp_sessions for validation tracking
    {
        let mut acp_sessions = state.acp_sessions.lock().await;
        let info = crate::state::AcpSessionInfo::new(
            uuid::Uuid::new_v4().to_string(),
            agent_id.to_string(),
            session_id.clone(),
        );
        acp_sessions.insert(info.session_id.clone(), info);
    }

    Ok(session_id)
}

/// Collect text response from the agent's message_rx channel.
///
/// Uses non-blocking try_recv in a polling loop so the agent_processes lock
/// is released between polls, allowing other tasks to access the process map.
async fn collect_response(
    state: &AppState,
    agent_id: &str,
    request_id: i64,
) -> AppResult<String> {
    let mut collected_text = String::new();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);

    loop {
        if std::time::Instant::now() >= deadline {
            if collected_text.is_empty() {
                return Err(AppError::Internal("Agent response timed out".into()));
            }
            break;
        }

        // Non-blocking receive: lock briefly, try_recv, release immediately
        let recv_result = {
            let mut processes = state.agent_processes.lock().await;
            match processes.get_mut(agent_id) {
                Some(process) => process.message_rx.try_recv(),
                None => {
                    return Err(AppError::AgentNotRunning(format!(
                        "Agent {} not running",
                        agent_id
                    )));
                }
            }
        };
        // Lock released here

        match recv_result {
            Ok(value) => {
                // Check if this is a final response to our request
                if let Some(id) = value.get("id") {
                    if id.as_i64() == Some(request_id) {
                        // Extract content from final response
                        if let Some(result) = value.get("result") {
                            if let Some(content_arr) = result.get("content") {
                                if let Some(arr) = content_arr.as_array() {
                                    for item in arr {
                                        if item.get("type").and_then(|t| t.as_str())
                                            == Some("text")
                                        {
                                            if let Some(text) =
                                                item.get("text").and_then(|t| t.as_str())
                                            {
                                                collected_text.push_str(text);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        break;
                    }
                }

                // Check for streaming content notifications
                let method = value.get("method").and_then(|m| m.as_str()).unwrap_or("");
                if method == "session/update" {
                    let update_type = value
                        .get("params")
                        .and_then(|p| p.get("update"))
                        .and_then(|u| u.get("sessionUpdate"))
                        .and_then(|s| s.as_str())
                        .unwrap_or("");

                    if update_type == "agent_message_chunk"
                        || update_type == "user_message_chunk"
                    {
                        if let Some(text) = value
                            .get("params")
                            .and_then(|p| p.get("update"))
                            .and_then(|u| u.get("content"))
                            .and_then(|c| c.get("text"))
                            .and_then(|t| t.as_str())
                        {
                            collected_text.push_str(text);
                        }
                    }
                }
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => {
                // No message yet — yield briefly so other tasks can make progress
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                continue;
            }
            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                if collected_text.is_empty() {
                    return Err(AppError::Internal(
                        "Agent message channel closed".into(),
                    ));
                }
                break;
            }
        }
    }

    if collected_text.is_empty() {
        return Err(AppError::Internal(
            "Agent returned empty response".into(),
        ));
    }

    log::info!(
        "[Bridge] Collected response from agent {} ({} chars)",
        agent_id,
        collected_text.len()
    );

    Ok(collected_text)
}
