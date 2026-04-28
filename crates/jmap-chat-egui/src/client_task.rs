// Background async task that drives all JMAP network I/O for the egui UI.
//
// Architecture:
// - `run` is the entry point, called from a `tokio::runtime::Runtime` spawned on
//   a dedicated thread outside the egui paint loop.
// - A bridge task converts the blocking `std::sync::mpsc::Receiver<AppCommand>`
//   into a tokio-friendly channel so it can participate in `select!`. This holds
//   one thread-pool thread for the process lifetime, which is acceptable because
//   there will never be more than one such bridge per process. The alternative —
//   polling `try_recv()` in a select! arm — would busy-loop between commands.
// - A separate SSE subtask drives the push stream and forwards state-change
//   notifications back via an internal tokio channel. On disconnect, the main
//   loop aborts the old task, recreates the channel, and re-spawns with
//   exponential backoff to avoid hammering a failing server.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use eframe::egui;
use futures::StreamExt;

use jmap_chat::{
    ChatQueryInput, ChatStreamDataType, ChatStreamEnable, ClientError, ContactPresence,
    JmapChatClient, MessageCreateInput, MessageQueryInput, SseEvent, WsFrame,
};

use crate::event::{AppCommand, AppEvent, ConnectionStatus};

// Unbounded sender so that send() in async context never blocks or fails due
// to backpressure. The UI drains via try_recv() each frame and is never the
// bottleneck; events are small and the rate is bounded by the server's SSE
// stream.
type EventSender = tokio::sync::mpsc::UnboundedSender<AppEvent>;

// ---------------------------------------------------------------------------
// Internal notification types
// ---------------------------------------------------------------------------

/// What the SSE subtask reports back to the command loop.
enum SseNotification {
    /// A "state" event arrived; the inner map is accountId → (typeName → state).
    StateChange(HashMap<String, HashMap<String, String>>),
    /// Normal stream end (network close or error). Carries the last `id:` value
    /// seen in the stream so the reconnect can send `Last-Event-ID` per RFC 8620 §7.3.
    StreamEnded { last_event_id: Option<String> },
    /// Permanent auth failure from `subscribe_events` — do not reconnect.
    AuthFailed,
}

/// What the WebSocket subtask reports back to the command loop.
enum WsNotification {
    /// A typing indicator event arrived.
    TypingIndicator {
        chat_id: String,
        sender_id: String,
        typing: bool,
    },
    /// A presence update event arrived.
    PresenceUpdate {
        contact_id: String,
        presence: ContactPresence,
    },
    /// Stream ended (disconnect or transport error) — reconnect with backoff.
    StreamEnded,
    /// Permanent auth failure during WebSocket handshake — do not reconnect.
    AuthFailed,
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
    tx: EventSender,
    rx: std::sync::mpsc::Receiver<AppCommand>,
    ctx: egui::Context,
) {
    let client = Arc::new(client);

    // Bridge the blocking std::mpsc::Receiver<AppCommand> to an async tokio channel.
    // Holds one thread-pool thread for the process lifetime — see module doc comment.
    let (cmd_tx, mut cmd_rx) = tokio::sync::mpsc::unbounded_channel::<AppCommand>();
    tokio::task::spawn_blocking(move || {
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
        None => {
            // Auth failed (not retriable); error already reported. Update status so
            // the UI does not remain stuck on "Connecting…" after the error dismisses.
            send_event(
                &tx,
                &ctx,
                AppEvent::StatusChanged(ConnectionStatus::Disconnected),
            );
            return;
        }
    };

    let event_source_url = session.event_source_url.clone();

    if session.chat_account_id().is_none() {
        send_event(
            &tx,
            &ctx,
            AppEvent::Error(
                "Server has no JMAP Chat account — \
                 check that the server supports the JMAP Chat extension"
                    .to_string(),
            ),
        );
        send_event(
            &tx,
            &ctx,
            AppEvent::StatusChanged(ConnectionStatus::Disconnected),
        );
        return;
    }

    // Phase 2b: Resolve WebSocket URL (and ephemeral capability) from session.
    //
    // Done here — before SessionReady — so `supports_ephemeral` is available for
    // the event. The WS task is spawned later once SSE is also set up.
    //
    // The WS stream carries ephemeral events (typing indicators, presence updates)
    // per draft-atwood-jmap-chat-wss-00. SSE remains the primary change-notification
    // transport; WS is additive.
    let ws_cap = match session.websocket_capability() {
        Ok(cap) => cap,
        Err(e) => {
            tracing::warn!(
                error = %e,
                "failed to parse WebSocket capability; \
                 server may have sent a malformed capability object — ephemeral events unavailable"
            );
            None
        }
    };
    let ws_url: Option<String> = match (ws_cap, session.supports_chat_websocket()) {
        (None, _) => {
            tracing::info!("server has no WebSocket capability; ephemeral events unavailable");
            None
        }
        (Some(_), false) => {
            tracing::warn!(
                "server advertises jmap:websocket but not jmap:chat:websocket; \
                 typing/presence unavailable — server may need upgrade"
            );
            None
        }
        (Some(cap), true) => {
            tracing::debug!(url = %cap.url, "ephemeral events via WebSocket");
            Some(cap.url.clone())
        }
    };

    send_event(
        &tx,
        &ctx,
        AppEvent::SessionReady {
            api_url: session.api_url.clone(),
            account_id: session.chat_account_id().unwrap_or_default().to_string(),
            supports_ephemeral: ws_url.is_some(),
        },
    );

    // Load all chats; track the server state string for future delta sync.
    // `None` means no baseline: delta sync will fall back to a full reload.
    let mut chat_state: Option<String> =
        match load_chats(Arc::clone(&client), &session, &tx, &ctx).await {
            Ok(state) => Some(state),
            Err(e) => {
                send_event(
                    &tx,
                    &ctx,
                    AppEvent::Error(format!("Failed to load chats: {e}")),
                );
                None
            }
        };

    // Load all read positions; store chat_id → read_position_id.
    //
    // Staleness invariant: read_positions is loaded once at bootstrap and is
    // never refreshed from the server via SSE (there is no ReadPosition state
    // change event). A chat created after startup will therefore be absent from
    // this map until the app restarts. try_mark_read and the MarkRead handler
    // both guard against the missing-key case: try_mark_read silently skips;
    // MarkRead reloads the full set and retries once before giving up.
    let mut read_positions: HashMap<String, String> =
        match load_read_positions(Arc::clone(&client), &session).await {
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
    let mut last_sse_event_id: Option<String> = None;
    let mut sse_handle =
        spawn_sse_task(Arc::clone(&client), event_source_url.clone(), sse_tx, None);

    let mut sse_backoff_idx = 0usize;

    // Spawn WebSocket subtask if the server supports it.
    //
    // A sender clone (_ws_tx_keepalive) is kept alive unconditionally so that
    // ws_notif_rx never closes in the select! when no WS task is running.

    let (ws_notif_tx, mut ws_notif_rx) = tokio::sync::mpsc::unbounded_channel::<WsNotification>();
    let mut ws_handle: Option<tokio::task::JoinHandle<()>> = None;
    let mut ws_backoff_idx = 0usize;
    let mut ws_consecutive_failures: usize = 0;
    const WS_MAX_FAILURES: usize = 5;

    if let Some(ref url) = ws_url {
        ws_handle = Some(spawn_ws_task(
            Arc::clone(&client),
            url.clone(),
            ws_notif_tx.clone(),
        ));
    }

    // Phase 3: Main command loop — multiplex commands, SSE notifications, and WS events.
    let mut current_chat: Option<String> = None;
    // `sse_needs_restart` and `ws_needs_restart` are set inside select! arms where we
    // cannot sleep directly; the actual reconnect happens at the top of the loop.
    let mut sse_needs_restart = false;
    let mut ws_needs_restart = false;

    loop {
        if sse_needs_restart {
            sse_needs_restart = false;
            let (new_handle, new_rx) = reconnect_sse(
                sse_handle,
                &mut sse_backoff_idx,
                Arc::clone(&client),
                &event_source_url,
                last_sse_event_id.clone(),
                &tx,
                &ctx,
            )
            .await;
            sse_handle = new_handle;
            sse_rx = new_rx;
        }

        if ws_needs_restart {
            ws_needs_restart = false;
            if let Some(ref url) = ws_url {
                ws_handle = Some(
                    reconnect_ws(
                        ws_handle,
                        &mut ws_backoff_idx,
                        Arc::clone(&client),
                        url,
                        ws_notif_tx.clone(),
                    )
                    .await,
                );
            }
        }

        tokio::select! {
            // WebSocket notification from the ephemeral-events subtask.
            ws_msg = ws_notif_rx.recv() => {
                match ws_msg {
                    None => {} // ws_notif_tx in scope; channel cannot close normally
                    Some(WsNotification::StreamEnded) => {
                        ws_consecutive_failures += 1;
                        if ws_consecutive_failures >= WS_MAX_FAILURES {
                            tracing::warn!(
                                consecutive = ws_consecutive_failures,
                                "WebSocket connection failed {WS_MAX_FAILURES} times without a successful \
                                 chat-stream frame; disabling ephemeral events"
                            );
                            send_event(&tx, &ctx, AppEvent::EphemeralUnavailable);
                            // ws_needs_restart stays false — do not retry further
                        } else {
                            ws_needs_restart = true;
                        }
                    }
                    Some(WsNotification::AuthFailed) => {
                        send_event(
                            &tx,
                            &ctx,
                            AppEvent::Error(
                                "WebSocket authentication failed — ephemeral events disabled"
                                    .to_string(),
                            ),
                        );
                        // Auth failure is permanent; drop the handle and stop reconnecting.
                        if let Some(h) = ws_handle.take() {
                            h.abort();
                        }
                        send_event(&tx, &ctx, AppEvent::EphemeralUnavailable);
                    }
                    Some(WsNotification::TypingIndicator {
                        chat_id,
                        sender_id,
                        typing,
                    }) => {
                        ws_consecutive_failures = 0;
                        ws_backoff_idx = 0;
                        send_event(
                            &tx,
                            &ctx,
                            AppEvent::TypingIndicator { chat_id, sender_id, typing },
                        );
                    }
                    Some(WsNotification::PresenceUpdate {
                        contact_id,
                        presence,
                    }) => {
                        ws_consecutive_failures = 0;
                        ws_backoff_idx = 0;
                        send_event(
                            &tx,
                            &ctx,
                            AppEvent::PresenceUpdate { contact_id, presence },
                        );
                    }
                }
            }

            // SSE notification from the push stream subtask.
            sse_msg = sse_rx.recv() => {
                match sse_msg {
                    None => {
                        // sse_tx was dropped — should not happen normally.
                        sse_needs_restart = true;
                        continue;
                    }
                    Some(SseNotification::StreamEnded { last_event_id }) => {
                        if last_event_id.is_some() {
                            last_sse_event_id = last_event_id;
                        }
                        sse_needs_restart = true;
                        continue;
                    }
                    Some(SseNotification::AuthFailed) => {
                        send_event(
                            &tx,
                            &ctx,
                            AppEvent::Error("SSE authentication failed — check credentials".to_string()),
                        );
                        send_event(
                            &tx,
                            &ctx,
                            AppEvent::StatusChanged(ConnectionStatus::Disconnected),
                        );
                        // Do not reconnect after auth failure.
                        continue;
                    }
                    Some(SseNotification::StateChange(changed)) => {
                        sse_backoff_idx = 0; // reset backoff on successful event
                        handle_state_change(
                            Arc::clone(&client),
                            &session,
                            &changed,
                            &current_chat,
                            &mut chat_state,
                            &tx,
                            &ctx,
                        )
                        .await;
                        send_event(&tx, &ctx, AppEvent::StatusChanged(ConnectionStatus::Connected));
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
                        match load_messages_for_chat(
                            Arc::clone(&client),
                            &session,
                            &chat_id,
                            &tx,
                            &ctx,
                        )
                        .await
                        {
                            Ok(last_msg_id) => {
                                // Auto-mark the last loaded message as read when the user
                                // opens a chat — they are presumed to have seen all visible
                                // messages at that point.
                                try_mark_read(
                                    &client,
                                    &session,
                                    &chat_id,
                                    last_msg_id,
                                    &read_positions,
                                    &tx,
                                    &ctx,
                                )
                                .await;
                            }
                            Err(e) => {
                                send_event(
                                    &tx,
                                    &ctx,
                                    AppEvent::Error(format!("Failed to load messages: {e}")),
                                );
                            }
                        }
                    }

                    Some(AppCommand::SendMessage { chat_id, body }) => {
                        let client_id = ulid::Ulid::new().to_string();
                        let sent_at =
                            jmap_chat::UTCDate::from_raw(now_rfc3339());
                        match client
                            .with_session(&session)
                            .message_create(
                                &MessageCreateInput::new(
                                    &chat_id,
                                    &body,
                                    jmap_chat::BodyType::Plain,
                                    &sent_at,
                                )
                                .with_client_id(client_id.as_str()),
                            )
                            .await
                        {
                            Ok(_set_resp) => {
                                if current_chat.as_deref() == Some(&chat_id) {
                                    match load_messages_for_chat(
                                        Arc::clone(&client),
                                        &session,
                                        &chat_id,
                                        &tx,
                                        &ctx,
                                    )
                                    .await
                                    {
                                        Ok(last_msg_id) => {
                                            // After sending, mark the thread as read — the
                                            // sender has obviously seen all messages.
                                            try_mark_read(
                                                &client,
                                                &session,
                                                &chat_id,
                                                last_msg_id,
                                                &read_positions,
                                                &tx,
                                                &ctx,
                                            )
                                            .await;
                                        }
                                        Err(e) => {
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
                            if let Err(e) = client
                                .with_session(&session)
                                .read_position_update(rp_id, &message_id)
                                .await
                            {
                                send_event(
                                    &tx,
                                    &ctx,
                                    AppEvent::Error(format!(
                                        "Failed to update read position: {e}"
                                    )),
                                );
                            }
                        } else {
                            // No read-position record yet; refresh the full set then retry once.
                            match load_read_positions(
                                Arc::clone(&client),
                                &session,
                            )
                            .await
                            {
                                Ok(new_map) => {
                                    read_positions = new_map;
                                    if let Some(rp_id) = read_positions.get(&chat_id) {
                                        if let Err(e) = client
                                            .with_session(&session)
                                            .read_position_update(
                                                rp_id,
                                                &message_id,
                                            )
                                            .await
                                        {
                                            send_event(
                                                &tx,
                                                &ctx,
                                                AppEvent::Error(format!(
                                                    "Failed to update read position: {e}"
                                                )),
                                            );
                                        }
                                    } else {
                                        // Chat has no read-position record on the server
                                        // even after re-fetching. This is a valid server
                                        // state (e.g. new chat with no read marker yet);
                                        // silently skip rather than reporting an error.
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
    tx: &EventSender,
    ctx: &egui::Context,
) -> Option<jmap_chat::Session> {
    let backoff_secs: &[u64] = &[1, 2, 4, 8, 16];
    let mut attempt = 0usize;

    loop {
        match client.fetch_session().await {
            Ok(session) => return Some(session),
            Err(ClientError::AuthFailed(code)) => {
                send_event(
                    tx,
                    ctx,
                    AppEvent::Error(format!(
                        "Authentication failed (HTTP {code}): \
                         check --bearer-token or --basic-user/--basic-pass"
                    )),
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

/// Load all chats via Chat/query + Chat/get.
///
/// Chat/get is always called — even when the query returns no IDs — so that
/// we receive the server's current `state` string. Without a valid state we
/// cannot perform delta sync (Chat/changes) and would fall back to a full
/// reload on every SSE StateChange event.
///
/// **Server requirement**: the server must accept `ids: []` in Chat/get and
/// return an empty list plus a valid `state` token (per RFC 8620 §5.1). This
/// is spec-compliant behaviour. A server that rejects empty `ids` arrays will
/// cause load_chats to return an error on empty accounts, leaving `chat_state`
/// as `None` and triggering a full reload on each Chat StateChange until at
/// least one chat exists. Known target servers (kith) accept `ids: []`.
///
/// Returns the server `state` string from the get response.
async fn load_chats(
    client: Arc<JmapChatClient>,
    session: &jmap_chat::Session,
    tx: &EventSender,
    ctx: &egui::Context,
) -> Result<String, ClientError> {
    const PAGE: u64 = 200;
    let mut all_ids: Vec<jmap_chat::Id> = Vec::new();
    let mut position = 0u64;
    loop {
        let query = client
            .with_session(session)
            .chat_query(&{
                let mut q = ChatQueryInput::default();
                q.limit = Some(PAGE);
                q.position = Some(position);
                q
            })
            .await?;
        let fetched = query.ids.len() as u64;
        let total = query.total;
        all_ids.extend(query.ids);
        position += fetched;
        if fetched < PAGE || total.is_some_and(|t| position >= t) {
            break;
        }
    }

    // Always call chat_get even when ids is empty, so we get the current state
    // string and can use Chat/changes for future delta sync.
    let id_refs: Vec<&str> = all_ids.iter().map(|id| id.as_str()).collect();
    let resp = client
        .with_session(session)
        .chat_get(Some(&id_refs), None)
        .await?;
    let state = resp.state.clone();
    send_event(tx, ctx, AppEvent::ChatsLoaded(resp.list));
    Ok(state)
}

// ---------------------------------------------------------------------------
// Message loading
// ---------------------------------------------------------------------------

async fn load_messages_for_chat(
    client: Arc<JmapChatClient>,
    session: &jmap_chat::Session,
    chat_id: &str,
    tx: &EventSender,
    ctx: &egui::Context,
) -> Result<Option<String>, ClientError> {
    let query = client
        .with_session(session)
        .message_query(&{
            let mut q = MessageQueryInput::default();
            q.chat_id = Some(chat_id);
            q.limit = Some(100);
            q
        })
        .await?;

    let mut messages = if query.ids.is_empty() {
        Vec::new()
    } else {
        let id_refs: Vec<&str> = query.ids.iter().map(|id| id.as_str()).collect();
        client
            .with_session(session)
            .message_get(&id_refs, None)
            .await?
            .list
    };

    // message_query returns IDs newest-first; after /get the order is
    // undefined. Sort ascending by parsed UTC instant so messages display
    // chronologically regardless of RFC 3339 offset format in sent_at.
    // messages.last() is then the most recent (required for auto-mark-read).
    // None (parse failure) sorts before Some, placing unparseable entries first.
    messages.sort_by_cached_key(|m| m.sent_at.parse().ok());

    // Capture the last message ID before moving `messages` into the event.
    let last_msg_id: Option<String> = messages.last().map(|m| m.id.to_string());
    send_event(
        tx,
        ctx,
        AppEvent::MessagesLoaded {
            chat_id: chat_id.to_string(),
            messages,
        },
    );
    Ok(last_msg_id)
}

// ---------------------------------------------------------------------------
// Read position loading
// ---------------------------------------------------------------------------

/// Load all ReadPosition records and return a map from chat_id to
/// read_position_id.
async fn load_read_positions(
    client: Arc<JmapChatClient>,
    session: &jmap_chat::Session,
) -> Result<HashMap<String, String>, ClientError> {
    let resp = client.with_session(session).read_position_get(None).await?;
    let mut map = HashMap::with_capacity(resp.list.len());
    for rp in resp.list {
        map.insert(rp.chat_id.to_string(), rp.id.to_string());
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Read-position helpers
// ---------------------------------------------------------------------------

/// Mark `last_msg_id` as read for `chat_id`, if both the message ID and a
/// read-position record for the chat are available.
///
/// Silently skips when either is absent: `last_msg_id` is `None` for an empty
/// chat, and a missing read-position record is a valid server state for a
/// newly-created chat that has never been marked read. This is the "best
/// effort" path used after auto-loading messages (SelectChat, SendMessage).
///
/// The explicit `MarkRead` command handler uses a different path with retry
/// logic (reload all read positions and try once more), because that is a
/// deliberate user action rather than a background heuristic.
async fn try_mark_read(
    client: &JmapChatClient,
    session: &jmap_chat::Session,
    chat_id: &str,
    last_msg_id: Option<String>,
    read_positions: &HashMap<String, String>,
    tx: &EventSender,
    ctx: &egui::Context,
) {
    if let Some(msg_id) = last_msg_id {
        if let Some(rp_id) = read_positions.get(chat_id) {
            if let Err(e) = client
                .with_session(session)
                .read_position_update(rp_id, &msg_id)
                .await
            {
                send_event(
                    tx,
                    ctx,
                    AppEvent::Error(format!("Failed to mark read: {e}")),
                );
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SSE subtask
// ---------------------------------------------------------------------------

/// Abort the current SSE task, wait out exponential backoff, recreate the
/// notification channel, and spawn a fresh SSE task.
///
/// The channel is recreated (not reused) so that any `StreamEnded` or stale
/// state-change messages queued by the aborted task are discarded. The old
/// `sse_rx` is dropped implicitly when the returned receiver replaces it.
///
/// Returns `(new_handle, new_rx)`. The caller **must** replace both `sse_handle`
/// and `sse_rx` with the returned values before the next `select!` iteration so
/// that the stale variables are not polled.
async fn reconnect_sse(
    old_handle: tokio::task::JoinHandle<()>,
    backoff_idx: &mut usize,
    client: Arc<JmapChatClient>,
    event_source_url: &str,
    last_event_id: Option<String>,
    tx: &EventSender,
    ctx: &egui::Context,
) -> (
    tokio::task::JoinHandle<()>,
    tokio::sync::mpsc::UnboundedReceiver<SseNotification>,
) {
    const BACKOFF: &[u64] = &[1, 2, 4, 8, 16, 30];
    old_handle.abort();
    let delay = BACKOFF
        .get(*backoff_idx)
        .copied()
        .unwrap_or(*BACKOFF.last().unwrap());
    *backoff_idx = backoff_idx.saturating_add(1);
    send_event(
        tx,
        ctx,
        AppEvent::StatusChanged(ConnectionStatus::Reconnecting),
    );
    tokio::time::sleep(Duration::from_secs(delay)).await;
    let (new_tx, new_rx) = tokio::sync::mpsc::unbounded_channel::<SseNotification>();
    let new_handle = spawn_sse_task(client, event_source_url.to_string(), new_tx, last_event_id);
    (new_handle, new_rx)
}

/// Spawn a tokio task that drives the SSE stream and forwards notifications.
///
/// Returns the `JoinHandle` so the caller can abort the task before re-spawning
/// on reconnect. Aborting is required: without it, the old task could deliver a
/// stale `StreamEnded` on the new channel, triggering a double reconnect.
fn spawn_sse_task(
    client: Arc<JmapChatClient>,
    event_source_url: String,
    sse_tx: tokio::sync::mpsc::UnboundedSender<SseNotification>,
    last_event_id: Option<String>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // run_sse_stream sends all notifications (StreamEnded or AuthFailed) itself.
        run_sse_stream(client, &event_source_url, &sse_tx, last_event_id).await;
    })
}

async fn run_sse_stream(
    client: Arc<JmapChatClient>,
    event_source_url: &str,
    sse_tx: &tokio::sync::mpsc::UnboundedSender<SseNotification>,
    last_event_id: Option<String>,
) {
    let stream = match client
        .subscribe_events(event_source_url, last_event_id.as_deref())
        .await
    {
        Ok(s) => s,
        Err(ClientError::AuthFailed(_)) => {
            let _ = sse_tx.send(SseNotification::AuthFailed);
            return; // do not send StreamEnded — main loop must not reconnect after auth failure
        }
        Err(_) => {
            let _ = sse_tx.send(SseNotification::StreamEnded { last_event_id });
            return;
        }
    };

    let mut current_event_id = last_event_id;

    tokio::pin!(stream);

    while let Some(item) = stream.next().await {
        match item {
            Err(_) => {
                let _ = sse_tx.send(SseNotification::StreamEnded {
                    last_event_id: current_event_id,
                });
                return;
            }
            Ok(frame) => {
                if let Some(id) = frame.id {
                    current_event_id = Some(id);
                }
                if let SseEvent::StateChange { changed } = frame.event {
                    if sse_tx.send(SseNotification::StateChange(changed)).is_err() {
                        return; // receiver dropped; no point sending StreamEnded
                    }
                }
            }
        }
    }

    let _ = sse_tx.send(SseNotification::StreamEnded {
        last_event_id: current_event_id,
    });
}

// ---------------------------------------------------------------------------
// State-change handler
// ---------------------------------------------------------------------------

// The argument count reflects that this function is the single integration
// point between the SSE stream and all three data-refresh paths (chats,
// messages, state tracking). Splitting it would scatter the flow across helper
// structs without reducing complexity.
async fn handle_state_change(
    client: Arc<JmapChatClient>,
    session: &jmap_chat::Session,
    changed: &HashMap<String, HashMap<String, String>>,
    current_chat: &Option<String>,
    chat_state: &mut Option<String>,
    tx: &EventSender,
    ctx: &egui::Context,
) {
    let account_id = match session.chat_account_id() {
        Some(id) => id,
        None => return, // session has no chat account; nothing to process
    };
    if let Some(type_map) = changed.get(account_id) {
        if type_map.contains_key("Chat") {
            if let Some(new_state) =
                chat_delta_sync(Arc::clone(&client), session, chat_state.as_deref(), tx, ctx).await
            {
                *chat_state = Some(new_state);
            }
        }

        if type_map.contains_key("Message") {
            // A new message also changes Chat.lastMessageAt. Sync chat metadata so the
            // chat list stays current when the server sends only a Message StateChange.
            if let Some(new_state) =
                chat_delta_sync(Arc::clone(&client), session, chat_state.as_deref(), tx, ctx).await
            {
                *chat_state = Some(new_state);
            }

            if let Some(chat_id) = current_chat {
                if let Err(e) =
                    load_messages_for_chat(Arc::clone(&client), session, chat_id, tx, ctx)
                        .await
                        .map(|_| ())
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

/// Call `load_chats` and convert the result to `Option<String>`, reporting any
/// error to the UI. Used as the shared fallback path in `chat_delta_sync`.
///
/// Use this instead of calling `load_chats` directly when the caller wants
/// errors sent to the UI and `None` returned on failure. Use `load_chats`
/// directly when the caller needs to handle the `ClientError` itself (e.g.
/// the bootstrap path in `run`, where `None` signals to leave `chat_state`
/// unchanged on the next sync attempt).
async fn full_reload_chats(
    client: Arc<JmapChatClient>,
    session: &jmap_chat::Session,
    tx: &EventSender,
    ctx: &egui::Context,
) -> Option<String> {
    match load_chats(client, session, tx, ctx).await {
        Ok(s) => Some(s),
        Err(e) => {
            send_event(
                tx,
                ctx,
                AppEvent::Error(format!("Failed to load chats: {e}")),
            );
            None
        }
    }
}

/// Sync chat changes since `chat_state` using Chat/changes + Chat/get.
///
/// `chat_state`:
/// - `None`: no baseline (initial load failed); falls back to a full reload.
/// - `Some(state)`: known server state; attempts delta sync via Chat/changes.
///
/// Falls back to a full Chat/query + Chat/get reload when:
/// - `has_more_changes` is true (too many changes to enumerate safely)
/// - server returns `cannotCalculateChanges` (state too old; history expired)
/// - any network call fails
///
/// Returns the new state string on success, or `None` on failure (`chat_state`
/// is left unchanged so the next attempt retries with the same baseline).
async fn chat_delta_sync(
    client: Arc<JmapChatClient>,
    session: &jmap_chat::Session,
    chat_state: Option<&str>,
    tx: &EventSender,
    ctx: &egui::Context,
) -> Option<String> {
    let state = match chat_state {
        None => return full_reload_chats(Arc::clone(&client), session, tx, ctx).await,
        Some(s) => s,
    };

    let changes = match client
        .with_session(session)
        .chat_changes(state, Some(500))
        .await
    {
        Ok(c) => c,
        Err(ClientError::MethodError { ref error_type, .. })
            if error_type == "cannotCalculateChanges" =>
        {
            // Server's change history has expired; fall back to a full reload.
            return full_reload_chats(Arc::clone(&client), session, tx, ctx).await;
        }
        Err(e) => {
            send_event(
                tx,
                ctx,
                AppEvent::Error(format!("Failed to get chat changes: {e}")),
            );
            return None;
        }
    };

    if changes.has_more_changes {
        return full_reload_chats(Arc::clone(&client), session, tx, ctx).await;
    }

    // Build the fetch list without cloning the ID strings.
    let id_refs: Vec<&str> = changes
        .created
        .iter()
        .chain(changes.updated.iter())
        .map(|id| id.as_str())
        .collect();

    let updated_chats = if id_refs.is_empty() {
        Vec::new()
    } else {
        match client
            .with_session(session)
            .chat_get(Some(&id_refs), None)
            .await
        {
            Ok(resp) => resp.list,
            Err(e) => {
                send_event(
                    tx,
                    ctx,
                    AppEvent::Error(format!("Failed to fetch updated chats: {e}")),
                );
                return None;
            }
        }
    };

    if !updated_chats.is_empty() || !changes.destroyed.is_empty() {
        send_event(
            tx,
            ctx,
            AppEvent::ChatsDelta {
                created_or_updated: updated_chats,
                destroyed: changes
                    .destroyed
                    .into_iter()
                    .map(|id| id.to_string())
                    .collect(),
            },
        );
    }

    Some(changes.new_state)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Send an event to the UI and request a repaint.
///
/// Uses an unbounded tokio channel so `send()` is always non-blocking and
/// never fails (except when the receiver is dropped, which means the UI has
/// shut down). Critical events such as MessagesLoaded and ChatsLoaded are
/// therefore never silently dropped due to backpressure.
fn send_event(tx: &EventSender, ctx: &egui::Context, event: AppEvent) {
    tx.send(event).ok();
    ctx.request_repaint();
}

/// Current UTC time formatted as RFC 3339 with second precision.
fn now_rfc3339() -> String {
    chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string()
}

// ---------------------------------------------------------------------------
// WebSocket subtask
// ---------------------------------------------------------------------------

/// Abort the current WS handle (if any), wait out exponential backoff, and
/// spawn a fresh WS task.
async fn reconnect_ws(
    old_handle: Option<tokio::task::JoinHandle<()>>,
    backoff_idx: &mut usize,
    client: Arc<JmapChatClient>,
    ws_url: &str,
    ws_tx: tokio::sync::mpsc::UnboundedSender<WsNotification>,
) -> tokio::task::JoinHandle<()> {
    const BACKOFF: &[u64] = &[1, 2, 4, 8, 16, 30];
    if let Some(h) = old_handle {
        h.abort();
    }
    let delay = BACKOFF
        .get(*backoff_idx)
        .copied()
        .unwrap_or(*BACKOFF.last().unwrap());
    *backoff_idx = backoff_idx.saturating_add(1);
    tokio::time::sleep(Duration::from_secs(delay)).await;
    spawn_ws_task(client, ws_url.to_string(), ws_tx)
}

/// Spawn a tokio task that drives the WebSocket stream and forwards notifications.
fn spawn_ws_task(
    client: Arc<JmapChatClient>,
    ws_url: String,
    ws_tx: tokio::sync::mpsc::UnboundedSender<WsNotification>,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        run_ws_stream(client, &ws_url, &ws_tx).await;
    })
}

async fn run_ws_stream(
    client: Arc<JmapChatClient>,
    ws_url: &str,
    ws_tx: &tokio::sync::mpsc::UnboundedSender<WsNotification>,
) {
    let mut session = match client.connect_ws(ws_url).await {
        Ok(s) => s,
        Err(ClientError::AuthFailed(_)) => {
            let _ = ws_tx.send(WsNotification::AuthFailed);
            return;
        }
        Err(_) => {
            let _ = ws_tx.send(WsNotification::StreamEnded);
            return;
        }
    };

    // Subscribe to all typing and presence events for all chats and contacts.
    let enable = ChatStreamEnable::new(
        &[ChatStreamDataType::Typing, ChatStreamDataType::Presence],
        None, // all chats
        None, // all contacts
    );
    if session.send_stream_enable(&enable).await.is_err() {
        let _ = ws_tx.send(WsNotification::StreamEnded);
        return;
    }

    loop {
        match session.next_frame().await {
            None => {
                let _ = ws_tx.send(WsNotification::StreamEnded);
                return;
            }
            Some(Err(_)) => {
                let _ = ws_tx.send(WsNotification::StreamEnded);
                return;
            }
            Some(Ok(WsFrame::ChatTyping(evt))) => {
                let _ = ws_tx.send(WsNotification::TypingIndicator {
                    chat_id: evt.chat_id.to_string(),
                    sender_id: evt.sender_id.to_string(),
                    typing: evt.typing,
                });
            }
            Some(Ok(WsFrame::ChatPresence(evt))) => {
                let _ = ws_tx.send(WsNotification::PresenceUpdate {
                    contact_id: evt.contact_id.to_string(),
                    presence: evt.presence,
                });
            }
            Some(Ok(_)) => {
                // StateChange, Response, Unknown — SSE handles state changes;
                // unknown types are silently ignored per forward-compat rules.
            }
        }
    }
}
