// Typed JMAP Chat method wrappers (Step 8)
//
// Response types mirror RFC 8620 standard shapes (§5.1 /get, §5.5 /query,
// §5.2 /changes, §5.3 /set). Method implementations live on JmapChatClient
// and are the primary public API for callers that already hold a Session.

use std::collections::HashMap;

use serde::Deserialize;

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
}

/// Input parameters for [`JmapChatClient::message_create`].
#[derive(Debug)]
pub struct MessageCreateInput<'a> {
    pub client_id: &'a str,
    pub chat_id: &'a str,
    pub body: &'a str,
    pub body_type: &'a str,
    /// RFC 3339 timestamp (e.g. from `chrono::Utc::now().to_rfc3339()`).
    pub sent_at: &'a crate::jmap::UTCDate,
    pub reply_to: Option<&'a str>,
}

// ---------------------------------------------------------------------------
// Method implementations on JmapChatClient
// ---------------------------------------------------------------------------

impl crate::client::JmapChatClient {
    /// Extract the API URL and chat account ID from a session.
    ///
    /// Returns `Err(InvalidSession)` if the session has no primary account for
    /// `urn:ietf:params:jmap:chat`. All method wrappers call this as their first
    /// step; currently mirrors: build_client, auth_header.
    fn session_parts(
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

    /// Fetch Chat objects by IDs (RFC 8620 §5.1 / JMAP Chat §5 Chat/get).
    ///
    /// If `ids` is `None`, the server returns all Chats for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn chat_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::Chat>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let req = build_request("Chat/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Query Chat IDs with optional filter (RFC 8620 §5.5 / JMAP Chat §5 Chat/query).
    ///
    /// Only keys that are `Some` in `input` are included in the filter object;
    /// an empty filter object is sent as JSON `null`.
    pub async fn chat_query(
        &self,
        session: &crate::jmap::Session,
        input: &ChatQueryInput,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut filter = serde_json::Map::new();
        if let Some(ref k) = input.filter_kind {
            let kind_str = serde_json::to_value(k).map_err(crate::error::ClientError::Serialize)?;
            filter.insert("kind".into(), kind_str);
        }
        if let Some(m) = input.filter_muted {
            filter.insert("muted".into(), m.into());
        }
        let filter_val = if filter.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::Object(filter)
        };
        let args = serde_json::json!({
            "accountId": account_id,
            "filter": filter_val,
            "position": input.position,
            "limit": input.limit,
        });
        let req = build_request("Chat/query", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Fetch changes to Chat objects since `since_state` (RFC 8620 §5.2 / Chat/changes).
    ///
    /// If `has_more_changes` is true in the response, call again with `new_state`
    /// as `since_state` until the flag is false.
    pub async fn chat_changes(
        &self,
        session: &crate::jmap::Session,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
            "maxChanges": max_changes,
        });
        let req = build_request("Chat/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Fetch Message objects by IDs (RFC 8620 §5.1 / JMAP Chat §5 Message/get).
    ///
    /// `ids` is required (non-empty); fetching all messages is impractical.
    /// Pass `properties: None` to return all fields.
    pub async fn message_get(
        &self,
        session: &crate::jmap::Session,
        ids: &[&str],
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::Message>, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_get: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let req = build_request("Message/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Query Message IDs within a Chat (RFC 8620 §5.5 / JMAP Chat §5 Message/query).
    ///
    /// Per spec, either `chat_id` or `has_mention: Some(true)` must be provided.
    /// Servers MUST return `unsupportedFilter` if neither condition holds.
    ///
    /// Results are sorted by `sentAt` descending (RFC 8620 §5.5 Comparator).
    /// Descending order ensures that `position:0, limit:N` returns the N most
    /// recent message IDs. Callers that display messages chronologically must
    /// reverse or re-sort ascending after fetching. Without an explicit sort,
    /// result order is undefined by the spec.
    pub async fn message_query(
        &self,
        session: &crate::jmap::Session,
        input: &MessageQueryInput<'_>,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        if input.chat_id.is_none() && input.has_mention != Some(true) {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_query: chat_id or has_mention=true must be provided".into(),
            ));
        }
        if let Some(id) = input.chat_id {
            if id.is_empty() {
                return Err(crate::error::ClientError::InvalidArgument(
                    "chat_id must not be empty".to_string(),
                ));
            }
        }
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut filter = serde_json::Map::new();
        if let Some(id) = input.chat_id {
            filter.insert("chatId".into(), id.into());
        }
        if let Some(m) = input.has_mention {
            filter.insert("hasMention".into(), m.into());
        }
        if let Some(a) = input.has_attachment {
            filter.insert("hasAttachment".into(), a.into());
        }
        let filter_val = if filter.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::Object(filter)
        };
        let args = serde_json::json!({
            "accountId": account_id,
            "filter": filter_val,
            "sort": [{"property": "sentAt", "isAscending": false}],
            "position": input.position,
            "limit": input.limit,
        });
        let req = build_request("Message/query", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Fetch changes to Message objects since `since_state` (RFC 8620 §5.2 / Message/changes).
    pub async fn message_changes(
        &self,
        session: &crate::jmap::Session,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
            "maxChanges": max_changes,
        });
        let req = build_request("Message/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Create (send) a new Message (RFC 8620 §5.3 / JMAP Chat §5 Message/set).
    ///
    /// `client_id` is a caller-supplied ULID used as the creation key. The server
    /// maps it to the server-assigned Message id in `SetResponse.created`.
    /// Only the `create` operation is implemented here; update/destroy are Phase 4.
    pub async fn message_create(
        &self,
        session: &crate::jmap::Session,
        input: &MessageCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut create_obj = serde_json::json!({
            "chatId": input.chat_id,
            "body": input.body,
            "bodyType": input.body_type,
            "sentAt": input.sent_at.as_str(),
        });
        if let Some(rt) = input.reply_to {
            create_obj["replyTo"] = rt.into();
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { input.client_id: create_obj },
        });
        let req = build_request("Message/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Fetch ChatContact objects by IDs (JMAP Chat §5 ChatContact/get).
    ///
    /// If `ids` is `None`, returns all ChatContacts for the account.
    pub async fn chat_contact_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::ChatContact>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let req = build_request("ChatContact/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Fetch ReadPosition objects by IDs (JMAP Chat §5 ReadPosition/get).
    ///
    /// If `ids` is `None`, returns all ReadPosition records for the account.
    /// The server creates one ReadPosition per Chat automatically.
    pub async fn read_position_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::ReadPosition>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
        });
        let req = build_request("ReadPosition/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Update the read position for a Chat (JMAP Chat §5 ReadPosition/set).
    ///
    /// `read_position_id` is the server-assigned ReadPosition.id (from
    /// `read_position_get`). `last_read_message_id` is the Message.id of the
    /// most recent message read. The server updates `lastReadAt` and
    /// recomputes `Chat.unreadCount`.
    ///
    /// `create` and `destroy` are forbidden by the spec; only `update` is issued.
    pub async fn read_position_set(
        &self,
        session: &crate::jmap::Session,
        read_position_id: &str,
        last_read_message_id: &str,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "update": {
                read_position_id: { "lastReadMessageId": last_read_message_id }
            },
        });
        let req = build_request("ReadPosition/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }

    /// Fetch the singleton PresenceStatus record (JMAP Chat §5 PresenceStatus/get).
    ///
    /// Per spec there is exactly one PresenceStatus per account; `ids: null`
    /// retrieves it.
    pub async fn presence_status_get(
        &self,
        session: &crate::jmap::Session,
    ) -> Result<GetResponse<crate::types::PresenceStatus>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": serde_json::Value::Null,
        });
        let req = build_request("PresenceStatus/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, CALL_ID)
    }
}

// ---------------------------------------------------------------------------
// Internal helper
// ---------------------------------------------------------------------------

/// The call-id used in every single-method JMAP request built by this module.
/// `extract_response` searches for this id. Do not change without updating both.
const CALL_ID: &str = "r1";

// Each request contains exactly one method call, identified by CALL_ID.
// extract_response() relies on this. Do not add multi-call batching here
// without updating extract_response() to accept a call-id parameter.
fn build_request(method_name: &str, args: serde_json::Value) -> crate::jmap::JmapRequest {
    crate::jmap::JmapRequest {
        using: chat_using().to_vec(),
        method_calls: vec![(method_name.to_string(), args, CALL_ID.to_string())],
    }
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
