// Typed JMAP Chat method wrappers (Step 8)
//
// Response types mirror RFC 8620 standard shapes (§5.1 /get, §5.5 /query,
// §5.2 /changes, §5.3 /set). Method implementations live on JmapChatClient
// and are the primary public API for callers that already hold a Session.

use std::collections::HashMap;

use serde::Deserialize;

pub mod chat;
pub mod contact;
pub mod message;
pub mod misc;

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
/// Used for both create (`message_create`) and update (`read_position_set`)
/// operations. Only the fields relevant to those two operations are modelled
/// here; `destroy` is deferred to Phase 4.
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
fn build_request(method_name: &str, args: serde_json::Value) -> (&'static str, crate::jmap::JmapRequest) {
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
