// Background async task that drives all JMAP network I/O for the egui UI.
//
// Architecture:
// - `run` is the entry point, called from a `tokio::runtime::Runtime` outside
//   the egui paint loop.
// - A bridge task converts the blocking `std::sync::mpsc::Receiver<AppCommand>`
//   into a tokio-friendly channel so it can participate in `select!`.
// - A separate SSE subtask drives the push stream and forwards state-change
//   notifications back via an unbounded tokio channel.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use futures::StreamExt;

use jmap_chat::client::JmapChatClient;
use jmap_chat::error::ClientError;
use jmap_chat::sse::SseEvent;

use crate::event::{AppCommand, AppEvent, ConnectionStatus};

// ---------------------------------------------------------------------------
// Internal SSE notification type
// ---------------------------------------------------------------------------

/// What the SSE subtask reports back to the command loop.
enum SseNotification {
    /// A "state" event arrived; the inner map is accountId → (typeName → state).
    StateChange(HashMap<String, HashMap<String, String>>),
    /// The SSE stream ended or hit an error; reconnect is needed.
    StreamEnded,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Run the background JMAP client task.
///
/// Drives all network I/O: session bootstrap, chat list load, message fetch on
/// chat selection, message send, and SSE real-time updates. Communicates with
/// the egui UI via mpsc channels. `ctx` is used to request UI repaints after
/// sending any event.
pub async fn run(
    client: JmapChatClient,
    tx: std::sync::mpsc::SyncSender<AppEvent>,
    rx: std::sync::mpsc::Receiver<AppCommand>,
    ctx: egui::Context,
) {
    let client = Arc::new(client);

    // Bridge the blocking std receiver to an async tokio channel.
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<AppCommand>();
    tokio::task::spawn_blocking(move || {
        // Loop until the sender side is dropped (UI shut down) or the bridge
        // receiver is dropped (client_task returned).
        while let Ok(cmd) = rx.recv() {
            if cmd_tx.send(cmd).is_err() {
                break;
            }
        }
    });

    // Phase 1: Bootstrap — session fetch with exponential backoff.
    send_event(
        &tx,
        &ctx,
        AppEvent::StatusChanged(ConnectionStatus::Connecting),
    );

    let session = match bootstrap_session(Arc::clone(&client), &tx, &ctx).await {
        Some(s) => s,
        None => return, // AuthFailed or tx dropped; already reported
    };

    let api_url = session.api_url.clone();
    let event_source_url = session.event_source_url.clone();

    let account_id = match session.chat_account_id() {
        Some(id) => id.to_string(),
        None => {
            send_event(
                &tx,
                &ctx,
                AppEvent::Error("Server session has no JMAP Chat account".to_string()),
            );
            return;
        }
    };

    send_event(
        &tx,
        &ctx,
        AppEvent::SessionReady {
            api_url: api_url.clone(),
            account_id: account_id.clone(),
        },
    );

    // Load all chats.
    if let Err(e) = load_chats(Arc::clone(&client), &api_url, &account_id, &tx, &ctx).await {
        send_event(
            &tx,
            &ctx,
            AppEvent::Error(format!("Failed to load chats: {e}")),
        );
    }

    // Load all read positions; store chat_id → read_position_id.
    let mut read_positions: HashMap<String, String> =
        match load_read_positions(Arc::clone(&client), &api_url, &account_id).await {
            Ok(map) => map,
            Err(e) => {
                send_event(
                    &tx,
                    &ctx,
                    AppEvent::Error(format!("Failed to load read positions: {e}")),
                );
                HashMap::new()
            }
        };

    send_event(
        &tx,
        &ctx,
        AppEvent::StatusChanged(ConnectionStatus::Connected),
    );

    // Phase 2: Spawn SSE subtask.
    let (sse_tx, mut sse_rx) = tokio::sync::mpsc::unbounded_channel::<SseNotification>();
    spawn_sse_task(
        Arc::clone(&client),
        event_source_url.clone(),
        sse_tx.clone(),
    );

    // Phase 3: Main command loop — multiplex commands and SSE notifications.
    let mut current_chat: Option<String> = None;
    let mut sse_needs_restart = false;

    loop {
        if sse_needs_restart {
            sse_needs_restart = false;
            send_event(
                &tx,
                &ctx,
                AppEvent::StatusChanged(ConnectionStatus::Reconnecting),
            );
            spawn_sse_task(
                Arc::clone(&client),
                event_source_url.clone(),
                sse_tx.clone(),
            );
        }

        tokio::select! {
            // SSE notification from the push stream subtask.
            sse_msg = sse_rx.recv() => {
                match sse_msg {
                    None => {
                        // sse_tx dropped — should not happen; treat as stream end.
                        sse_needs_restart = true;
                        continue;
                    }
                    Some(SseNotification::StreamEnded) => {
                        sse_needs_restart = true;
                        continue;
                    }
                    Some(SseNotification::StateChange(changed)) => {
                        handle_state_change(
                            Arc::clone(&client),
                            &api_url,
                            &account_id,
                            &changed,
                            &current_chat,
                            &tx,
                            &ctx,
                        )
                        .await;
                        send_event(
                            &tx,
                            &ctx,
                            AppEvent::StatusChanged(ConnectionStatus::Connected),
                        );
                    }
                }
            }

            // Command from the UI.
            cmd = cmd_rx.recv() => {
                match cmd {
                    None => {
                        // UI dropped the sender; we're done.
                        return;
                    }
                    Some(AppCommand::SelectChat(chat_id)) => {
                        current_chat = Some(chat_id.clone());
                        if let Err(e) = load_messages_for_chat(
                            Arc::clone(&client),
                            &api_url,
                            &account_id,
                            &chat_id,
                            &tx,
                            &ctx,
                        )
                        .await
                        {
                            send_event(
                                &tx,
                                &ctx,
                                AppEvent::Error(format!("Failed to load messages: {e}")),
                            );
                        }

                        // Update read position to last message if we have one.
                        // We re-fetch messages here; mark last as read after load.
                        // (The actual mark-read on load is done via MarkRead from UI.)
                    }

                    Some(AppCommand::SendMessage { chat_id, body }) => {
                        let client_id = ulid::Ulid::new().to_string();
                        let sent_at = now_rfc3339();
                        match client
                            .message_create(
                                &api_url,
                                &account_id,
                                &client_id,
                                &chat_id,
                                &body,
                                "text/plain",
                                &sent_at,
                                None,
                            )
                            .await
                        {
                            Ok(_set_resp) => {
                                // Refresh messages for the current chat if it matches.
                                if current_chat.as_deref() == Some(&chat_id) {
                                    if let Err(e) = load_messages_for_chat(
                                        Arc::clone(&client),
                                        &api_url,
                                        &account_id,
                                        &chat_id,
                                        &tx,
                                        &ctx,
                                    )
                                    .await
                                    {
                                        send_event(
                                            &tx,
                                            &ctx,
                                            AppEvent::Error(format!(
                                                "Failed to refresh messages: {e}"
                                            )),
                                        );
                                    }
                                }
                            }
                            Err(e) => {
                                send_event(
                                    &tx,
                                    &ctx,
                                    AppEvent::Error(format!("Failed to send message: {e}")),
                                );
                            }
                        }
                    }

                    Some(AppCommand::MarkRead { chat_id, message_id }) => {
                        if let Some(rp_id) = read_positions.get(&chat_id) {
                            match client
                                .read_position_set(&api_url, &account_id, rp_id, &message_id)
                                .await
                            {
                                Ok(_) => {}
                                Err(e) => {
                                    send_event(
                                        &tx,
                                        &ctx,
                                        AppEvent::Error(format!(
                                            "Failed to update read position: {e}"
                                        )),
                                    );
                                    // Continue — not fatal.
                                }
                            }
                        } else {
                            // No read position record for this chat yet; try fetching
                            // positions again and update the local map.
                            match load_read_positions(
                                Arc::clone(&client),
                                &api_url,
                                &account_id,
                            )
                            .await
                            {
                                Ok(new_map) => {
                                    read_positions = new_map;
                                    if let Some(rp_id) = read_positions.get(&chat_id) {
                                        let _ = client
                                            .read_position_set(
                                                &api_url,
                                                &account_id,
                                                rp_id,
                                                &message_id,
                                            )
                                            .await;
                                    }
                                }
                                Err(e) => {
                                    send_event(
                                        &tx,
                                        &ctx,
                                        AppEvent::Error(format!(
                                            "Failed to update read position: {e}"
                                        )),
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Bootstrap helpers
// ---------------------------------------------------------------------------

/// Attempt to fetch the JMAP session with exponential backoff.
///
/// Returns `Some(session)` on success, `None` if auth failed (not retriable)
/// or the event channel was dropped.
async fn bootstrap_session(
    client: Arc<JmapChatClient>,
    tx: &std::sync::mpsc::SyncSender<AppEvent>,
    ctx: &egui::Context,
) -> Option<jmap_chat::jmap::Session> {
    let backoff_secs: &[u64] = &[1, 2, 4, 8, 16];
    let mut attempt = 0usize;

    loop {
        match client.fetch_session().await {
            Ok(session) => return Some(session),
            Err(ClientError::AuthFailed(code)) => {
                send_event(
                    tx,
                    ctx,
                    AppEvent::Error(format!("Authentication failed (HTTP {code})")),
                );
                return None;
            }
            Err(e) => {
                let delay = backoff_secs
                    .get(attempt)
                    .copied()
                    .unwrap_or(*backoff_secs.last().unwrap());
                send_event(
                    tx,
                    ctx,
                    AppEvent::Error(format!("Session fetch failed: {e}; retrying in {delay}s")),
                );
                tokio::time::sleep(Duration::from_secs(delay)).await;
                attempt = attempt.saturating_add(1);
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Chat loading
// ---------------------------------------------------------------------------

async fn load_chats(
    client: Arc<JmapChatClient>,
    api_url: &str,
    account_id: &str,
    tx: &std::sync::mpsc::SyncSender<AppEvent>,
    ctx: &egui::Context,
) -> Result<(), ClientError> {
    let query = client
        .chat_query(api_url, account_id, None, None, None, Some(200))
        .await?;

    let chats = if query.ids.is_empty() {
        Vec::new()
    } else {
        let id_refs: Vec<&str> = query.ids.iter().map(String::as_str).collect();
        client
            .chat_get(api_url, account_id, Some(&id_refs), None)
            .await?
            .list
    };

    send_event(tx, ctx, AppEvent::ChatsLoaded(chats));
    Ok(())
}

// ---------------------------------------------------------------------------
// Message loading
// ---------------------------------------------------------------------------

async fn load_messages_for_chat(
    client: Arc<JmapChatClient>,
    api_url: &str,
    account_id: &str,
    chat_id: &str,
    tx: &std::sync::mpsc::SyncSender<AppEvent>,
    ctx: &egui::Context,
) -> Result<(), ClientError> {
    let query = client
        .message_query(
            api_url,
            account_id,
            Some(chat_id),
            None,
            None,
            None,
            Some(100),
        )
        .await?;

    let messages = if query.ids.is_empty() {
        Vec::new()
    } else {
        let id_refs: Vec<&str> = query.ids.iter().map(String::as_str).collect();
        client
            .message_get(api_url, account_id, &id_refs, None)
            .await?
            .list
    };

    send_event(
        tx,
        ctx,
        AppEvent::MessagesLoaded {
            chat_id: chat_id.to_string(),
            messages,
        },
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Read position loading
// ---------------------------------------------------------------------------

/// Load all ReadPosition records and return a map from chat_id to
/// read_position_id.
async fn load_read_positions(
    client: Arc<JmapChatClient>,
    api_url: &str,
    account_id: &str,
) -> Result<HashMap<String, String>, ClientError> {
    let resp = client.read_position_get(api_url, account_id, None).await?;
    let mut map = HashMap::with_capacity(resp.list.len());
    for rp in resp.list {
        map.insert(rp.chat_id.to_string(), rp.id.to_string());
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// SSE subtask
// ---------------------------------------------------------------------------

/// Spawn a tokio task that drives the SSE stream and forwards notifications.
///
/// On any stream error or end-of-stream, sends `SseNotification::StreamEnded`
/// and exits. The caller is responsible for re-spawning with backoff.
fn spawn_sse_task(
    client: Arc<JmapChatClient>,
    event_source_url: String,
    sse_tx: tokio::sync::mpsc::UnboundedSender<SseNotification>,
) {
    tokio::spawn(async move {
        run_sse_stream(client, &event_source_url, &sse_tx).await;
        // Notify the main loop regardless of why we exited.
        let _ = sse_tx.send(SseNotification::StreamEnded);
    });
}

async fn run_sse_stream(
    client: Arc<JmapChatClient>,
    event_source_url: &str,
    sse_tx: &tokio::sync::mpsc::UnboundedSender<SseNotification>,
) {
    let stream = match client.subscribe_events(event_source_url, None).await {
        Ok(s) => s,
        Err(ClientError::AuthFailed(_)) => {
            // Auth failure is not retriable; let StreamEnded propagate but
            // the main loop will keep retrying. In practice the UI should
            // detect this via the Error events from bootstrap.
            return;
        }
        Err(_) => return,
    };

    tokio::pin!(stream);

    while let Some(item) = stream.next().await {
        match item {
            Err(_) => return,
            Ok(frame) => {
                if let SseEvent::StateChange { changed } = frame.event {
                    if sse_tx.send(SseNotification::StateChange(changed)).is_err() {
                        return;
                    }
                }
                // Unknown / Typing / Presence frames are silently ignored.
            }
        }
    }
}

// ---------------------------------------------------------------------------
// State-change handler
// ---------------------------------------------------------------------------

async fn handle_state_change(
    client: Arc<JmapChatClient>,
    api_url: &str,
    account_id: &str,
    changed: &HashMap<String, HashMap<String, String>>,
    current_chat: &Option<String>,
    tx: &std::sync::mpsc::SyncSender<AppEvent>,
    ctx: &egui::Context,
) {
    // changed is accountId → { TypeName → newState }.
    // We look at the local account's changes only.
    if let Some(type_map) = changed.get(account_id) {
        let chat_changed = type_map.contains_key("Chat");
        let message_changed = type_map.contains_key("Message");

        if chat_changed {
            if let Err(e) = load_chats(Arc::clone(&client), api_url, account_id, tx, ctx).await {
                send_event(
                    tx,
                    ctx,
                    AppEvent::Error(format!("Failed to refresh chats: {e}")),
                );
            }
        }

        if message_changed {
            if let Some(chat_id) = current_chat {
                if let Err(e) = load_messages_for_chat(
                    Arc::clone(&client),
                    api_url,
                    account_id,
                    chat_id,
                    tx,
                    ctx,
                )
                .await
                {
                    send_event(
                        tx,
                        ctx,
                        AppEvent::Error(format!("Failed to refresh messages: {e}")),
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Send an event to the UI and request a repaint.
///
/// Silently ignores send failures (receiver dropped means the UI shut down).
fn send_event(tx: &std::sync::mpsc::SyncSender<AppEvent>, ctx: &egui::Context, event: AppEvent) {
    tx.send(event).ok();
    ctx.request_repaint();
}

/// Current UTC time formatted as RFC 3339 with second precision.
fn now_rfc3339() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Decompose Unix timestamp into date/time components.
    let s = secs % 60;
    let m = (secs / 60) % 60;
    let h = (secs / 3600) % 24;
    let days = secs / 86400;

    // Days since 1970-01-01 → Gregorian calendar.
    let (year, month, day) = days_to_ymd(days);

    format!("{year:04}-{month:02}-{day:02}T{h:02}:{m:02}:{s:02}Z")
}

/// Convert days since Unix epoch to (year, month, day).
///
/// Algorithm: civil calendar from Howard Hinnant's date library.
/// Reference: <https://howardhinnant.github.io/date_algorithms.html#civil_from_days>
fn days_to_ymd(z: u64) -> (u64, u64, u64) {
    // Shift epoch to 1 Mar 0000 for simpler leap-year math.
    let z = z as i64 + 719_468;
    let era: i64 = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64; // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y as u64, m, d)
}
