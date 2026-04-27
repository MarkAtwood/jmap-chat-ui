// Typed JMAP Chat method wrappers (Step 8)
//
// Response types mirror RFC 8620 standard shapes (§5.1 /get, §5.5 /query,
// §5.2 /changes, §5.3 /set). Method implementations live on JmapChatClient
// and are the primary public API for callers that already hold a Session.

use std::collections::HashMap;

use serde::Deserialize;

pub mod chat;
pub mod contact;
pub mod custom_emoji;
pub mod message;
pub mod misc;
pub mod space_ban;
pub mod space_invite;

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

/// RFC 8620 §5.1 — /get response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetResponse<T> {
    pub account_id: String,
    pub state: String,
    pub list: Vec<T>,
    pub not_found: Option<Vec<String>>,
}

/// RFC 8620 §5.5 — /query response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryResponse {
    pub account_id: String,
    pub query_state: String,
    pub can_calculate_changes: bool,
    pub position: u64,
    pub ids: Vec<String>,
    pub total: Option<u64>,
    pub limit: Option<u64>,
}

/// RFC 8620 §5.2 — /changes response.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChangesResponse {
    pub account_id: String,
    pub old_state: String,
    pub new_state: String,
    pub has_more_changes: bool,
    pub created: Vec<String>,
    pub updated: Vec<String>,
    pub destroyed: Vec<String>,
}

/// RFC 8620 §5.3 — /set response.
///
/// Used for create (`message_create`, `custom_emoji_set`, `space_ban_set`,
/// `space_invite_set`), update (`read_position_set`, `presence_status_set`),
/// and destroy operations. All optional maps are `None` when absent in the
/// server response.
///
/// The type parameter `T` is the shape of each created/updated object.
/// Defaults to `serde_json::Value` so callers that don't need typed objects
/// can use `SetResponse` without a type argument.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(bound(deserialize = "T: serde::de::DeserializeOwned"))]
pub struct SetResponse<T = serde_json::Value> {
    pub account_id: String,
    pub old_state: Option<String>,
    pub new_state: String,
    pub created: Option<HashMap<String, T>>,
    pub updated: Option<HashMap<String, T>>,
    pub destroyed: Option<Vec<String>>,
    pub not_created: Option<HashMap<String, SetError>>,
    pub not_updated: Option<HashMap<String, SetError>>,
    pub not_destroyed: Option<HashMap<String, SetError>>,
}

/// A /set operation failure for a single object (RFC 8620 §5.3).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetError {
    #[serde(rename = "type")]
    pub error_type: String,
    pub description: Option<String>,
}

/// RFC 8620 §5.6 — /queryChanges response.
///
/// Reports which IDs were removed from and added to a query result set since
/// `old_query_state`. Used by `custom_emoji_query_changes` and any future
/// /queryChanges implementations.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QueryChangesResponse {
    pub account_id: String,
    pub old_query_state: String,
    pub new_query_state: String,
    pub total: Option<u64>,
    pub removed: Vec<String>,
    pub added: Vec<AddedItem>,
}

/// A single item added to a query result set (RFC 8620 §5.6).
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddedItem {
    pub id: String,
    pub index: u64,
}

// ---------------------------------------------------------------------------
// Input types for methods with many optional parameters
// ---------------------------------------------------------------------------

/// Input parameters for [`JmapChatClient::chat_query`].
#[derive(Debug, Default)]
pub struct ChatQueryInput {
    pub filter_kind: Option<crate::types::ChatKind>,
    pub filter_muted: Option<bool>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
}

/// Input parameters for [`JmapChatClient::message_query`].
#[derive(Debug, Default)]
pub struct MessageQueryInput<'a> {
    pub chat_id: Option<&'a str>,
    pub has_mention: Option<bool>,
    pub has_attachment: Option<bool>,
    pub text: Option<&'a str>,
    pub thread_root_id: Option<&'a str>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
    /// Sort by `sentAt` ascending (oldest first) when `true`.
    /// Defaults to `false` (descending, newest first), so `position:0, limit:N`
    /// returns the N most recent messages.
    pub sort_ascending: bool,
}

/// Input parameters for [`JmapChatClient::message_create`].
#[derive(Debug)]
pub struct MessageCreateInput<'a> {
    pub client_id: &'a str,
    pub chat_id: &'a str,
    pub body: &'a str,
    /// MIME type for the message body. Spec-defined values: `"text/plain"`,
    /// `"text/markdown"`. An unrecognized value will produce a server
    /// `MethodError`; no client-side validation is performed.
    pub body_type: &'a str,
    /// RFC 3339 timestamp (e.g. from `chrono::Utc::now().to_rfc3339()`).
    pub sent_at: &'a crate::jmap::UTCDate,
    pub reply_to: Option<&'a str>,
}

/// A single reaction change in a Message/set patch (JMAP Chat §4.5).
///
/// The patch key is `reactions/<senderReactionId>` (JSON Pointer).
/// `senderReactionId` is a caller-generated ID (e.g. ULID) that uniquely
/// identifies this reaction slot for the sending user in this message.
#[non_exhaustive]
#[derive(Debug)]
pub enum ReactionChange<'a> {
    /// Add a reaction. Patch value: `{emoji, sentAt}`.
    Add {
        sender_reaction_id: &'a str,
        emoji: &'a str,
        sent_at: &'a crate::jmap::UTCDate,
    },
    /// Remove a reaction. Patch value: null.
    Remove { sender_reaction_id: &'a str },
}

/// Input parameters for [`JmapChatClient::message_set_update`].
///
/// All fields except `id` are optional; absent fields are not included in the
/// patch (the server leaves them unchanged). For chat-level deletion, set
/// `deleted_at` (soft-delete) and optionally `deleted_for_all: Some(true)`
/// (hard-delete, propagated to all participants).
#[derive(Debug)]
pub struct MessageUpdateInput<'a> {
    /// `Message.id` to update.
    pub id: &'a str,
    /// New message body text (author-only edit).
    pub body: Option<&'a str>,
    /// MIME type for `body` — `"text/plain"` or `"text/markdown"`.
    pub body_type: Option<&'a str>,
    /// Reaction changes to apply in this update. Pass `&[]` when only other
    /// fields are being patched. Example: `reaction_changes: &changes[..]`
    /// where `changes: Vec<ReactionChange<'_>>` is built by the caller.
    pub reaction_changes: &'a [ReactionChange<'a>],
    /// Set the read-receipt timestamp (`Message.readAt`).
    pub read_at: Option<&'a crate::jmap::UTCDate>,
    /// Set the deletion timestamp for soft/hard delete.
    pub deleted_at: Option<&'a crate::jmap::UTCDate>,
    /// When `Some(true)` and `deleted_at` is also set, deletes for all
    /// participants (server sends `Peer/retract`).
    pub deleted_for_all: Option<bool>,
}

/// Input parameters for [`JmapChatClient::presence_status_set`].
///
/// All fields except `id` are optional. A field that is `None` is omitted from
/// the patch, leaving the server value unchanged. For nullable spec fields
/// (`status_text`, `status_emoji`, `expires_at`) use `Some(None)` to clear
/// the field and `Some(Some(value))` to set it.
///
/// `Default` is intentionally not derived: `id` has no safe default value and
/// an empty-string id would produce an invalid `/set` patch key.
#[derive(Debug)]
pub struct PresenceStatusSetInput<'a> {
    /// The PresenceStatus.id to update (from `presence_status_get`).
    pub id: &'a str,
    pub presence: Option<crate::types::OwnerPresence>,
    pub status_text: Option<Option<&'a str>>,
    pub status_emoji: Option<Option<&'a str>>,
    /// Set or clear the auto-clear deadline. `Some(None)` removes any deadline.
    pub expires_at: Option<Option<&'a crate::jmap::UTCDate>>,
    pub receipt_sharing: Option<bool>,
}

/// Input parameters for [`JmapChatClient::custom_emoji_query`].
#[derive(Debug, Default)]
pub struct CustomEmojiQueryInput<'a> {
    /// Filter to a specific Space's custom emojis. `None` returns all emojis
    /// visible to the account (Space-specific + server-global).
    pub filter_space_id: Option<&'a str>,
    pub position: Option<u64>,
    pub limit: Option<u64>,
}

/// Parameters for creating one CustomEmoji via [`JmapChatClient::custom_emoji_set`].
#[derive(Debug)]
pub struct CustomEmojiCreateInput<'a> {
    /// Caller-supplied ULID used as the creation key in the JMAP create map.
    pub client_id: &'a str,
    /// Shortcode name without colons (e.g., `catjam`).
    pub name: &'a str,
    /// blobId of the emoji image (already uploaded).
    pub blob_id: &'a str,
    /// If `Some`, limits the emoji to the given Space. `None` = server-global.
    pub space_id: Option<&'a str>,
}

/// Parameters for creating one SpaceInvite via [`JmapChatClient::space_invite_set`].
#[derive(Debug)]
pub struct SpaceInviteCreateInput<'a> {
    /// Caller-supplied ULID used as the creation key.
    pub client_id: &'a str,
    pub space_id: &'a str,
    pub default_channel_id: Option<&'a str>,
    pub expires_at: Option<&'a crate::jmap::UTCDate>,
    pub max_uses: Option<u64>,
}

/// Parameters for creating one SpaceBan via [`JmapChatClient::space_ban_set`].
#[derive(Debug)]
pub struct SpaceBanCreateInput<'a> {
    /// Caller-supplied ULID used as the creation key.
    pub client_id: &'a str,
    pub space_id: &'a str,
    /// ChatContact.id of the user to ban.
    pub user_id: &'a str,
    pub reason: Option<&'a str>,
    pub expires_at: Option<&'a crate::jmap::UTCDate>,
}

// ---------------------------------------------------------------------------
// Private helpers (accessible to child modules via super::)
// ---------------------------------------------------------------------------

/// The call-id embedded in every single-method JMAP request produced by
/// [`build_request`]. Returned alongside the request so callers pass it
/// directly to [`crate::client::extract_response`] — no separate import needed.
const CALL_ID: &str = "r1";

/// Build a single-method JMAP request.
///
/// Returns `(call_id, request)`. Pass `call_id` to
/// `crate::client::extract_response` so the pairing is explicit and
/// compiler-visible rather than via a shared constant.
fn build_request(
    method_name: &str,
    args: serde_json::Value,
) -> (&'static str, crate::jmap::JmapRequest) {
    let req = crate::jmap::JmapRequest {
        using: chat_using().to_vec(),
        method_calls: vec![(method_name.to_string(), args, CALL_ID.to_string())],
    };
    (CALL_ID, req)
}

fn chat_using() -> &'static [String] {
    static CHAT_USING_VEC: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();
    CHAT_USING_VEC.get_or_init(|| {
        vec![
            "urn:ietf:params:jmap:core".to_string(),
            "urn:ietf:params:jmap:chat".to_string(),
        ]
    })
}

// ---------------------------------------------------------------------------
// Private impl block — session helper, used by all child modules
// ---------------------------------------------------------------------------

impl crate::client::JmapChatClient {
    /// Extract the API URL and chat account ID from a session.
    ///
    /// Returns `Err(InvalidSession)` if the session has no primary account for
    /// `urn:ietf:params:jmap:chat`. All method wrappers call this as their first
    /// step.
    pub(super) fn session_parts(
        session: &crate::jmap::Session,
    ) -> Result<(&str, &str), crate::error::ClientError> {
        let api_url = session.api_url.as_str();
        let account_id = session.chat_account_id().ok_or_else(|| {
            crate::error::ClientError::InvalidSession(
                "no primary account for urn:ietf:params:jmap:chat in Session.primaryAccounts",
            )
        })?;
        Ok((api_url, account_id))
    }
}
