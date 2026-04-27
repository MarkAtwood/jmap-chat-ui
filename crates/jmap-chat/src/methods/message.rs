use super::{
    ChangesResponse, GetResponse, MessageCreateInput, MessageQueryInput, QueryResponse, SetResponse,
};

impl crate::client::JmapChatClient {
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
        let (call_id, req) = super::build_request("Message/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Query Message IDs within a Chat (RFC 8620 §5.5 / JMAP Chat §5 Message/query).
    ///
    /// Per spec, either `chat_id` or `has_mention: Some(true)` must be provided.
    /// Servers MUST return `unsupportedFilter` if neither condition holds.
    ///
    /// Sort order is controlled by `input.sort_ascending` (default `false` =
    /// newest first). With `position:0, limit:N` and `sort_ascending:false`, the
    /// server returns the N most recent message IDs. Callers displaying messages
    /// chronologically should set `sort_ascending:true` or reverse after fetching.
    pub async fn message_query(
        &self,
        session: &crate::jmap::Session,
        input: &MessageQueryInput<'_>,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        match (input.chat_id, input.has_mention) {
            (None, Some(true)) => {} // has_mention=true: no chat_id required
            (None, _) => {
                return Err(crate::error::ClientError::InvalidArgument(
                    "message_query: chat_id or has_mention=true must be provided".into(),
                ))
            }
            (Some(""), _) => {
                return Err(crate::error::ClientError::InvalidArgument(
                    "chat_id must not be empty".into(),
                ))
            }
            (Some(_), _) => {} // chat_id present and non-empty
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
        let mut args = serde_json::json!({
            "accountId": account_id,
            "filter": filter_val,
            "sort": [{"property": "sentAt", "isAscending": input.sort_ascending}],
        });
        if let Some(p) = input.position {
            args["position"] = p.into();
        }
        if let Some(l) = input.limit {
            args["limit"] = l.into();
        }
        let (call_id, req) = super::build_request("Message/query", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
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
        let (call_id, req) = super::build_request("Message/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
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
        let (call_id, req) = super::build_request("Message/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
