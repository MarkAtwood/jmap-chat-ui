use super::{
    ChangesResponse, GetResponse, MessageCreateInput, MessageQueryInput, MessageUpdateInput,
    QueryChangesResponse, QueryResponse, ReactionChange, SetResponse,
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
        if let Some(t) = input.text {
            filter.insert("text".into(), t.into());
        }
        if let Some(tid) = input.thread_root_id {
            filter.insert("threadRootId".into(), tid.into());
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
    /// For update and destroy operations see [`Self::message_set_update`] and
    /// [`Self::message_set_destroy`].
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

    /// Update Message properties (RFC 8620 §5.3 / JMAP Chat §4.5 Message/set).
    ///
    /// Issues an `update` operation patching only the fields present in
    /// `input`. Supports body edits (author-only), reaction changes (JSON
    /// Pointer patch on `reactions` map), read-receipt updates (`readAt`),
    /// and chat-level deletion (`deletedAt` / `deletedForAll`).
    ///
    /// If all optional fields are `None` and `reaction_changes` is empty, an
    /// empty patch object is sent. RFC 8620 §5.3 permits this; the server
    /// treats it as a no-op but still returns the object in `updated`.
    pub async fn message_set_update(
        &self,
        session: &crate::jmap::Session,
        input: &MessageUpdateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut patch = serde_json::Map::new();
        if let Some(b) = input.body {
            patch.insert("body".into(), b.into());
        }
        if let Some(bt) = input.body_type {
            patch.insert("bodyType".into(), bt.into());
        }
        if let Some(ra) = input.read_at {
            patch.insert("readAt".into(), ra.as_str().into());
        }
        if let Some(da) = input.deleted_at {
            patch.insert("deletedAt".into(), da.as_str().into());
        }
        if let Some(dfa) = input.deleted_for_all {
            patch.insert("deletedForAll".into(), dfa.into());
        }
        for change in input.reaction_changes {
            match change {
                ReactionChange::Add {
                    sender_reaction_id,
                    emoji,
                    sent_at,
                } => {
                    patch.insert(
                        format!("reactions/{sender_reaction_id}"),
                        serde_json::json!({"emoji": emoji, "sentAt": sent_at.as_str()}),
                    );
                }
                ReactionChange::Remove { sender_reaction_id } => {
                    patch.insert(
                        format!("reactions/{sender_reaction_id}"),
                        serde_json::Value::Null,
                    );
                }
            }
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "update": { input.id: serde_json::Value::Object(patch) },
        });
        let (call_id, req) = super::build_request("Message/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Destroy Message objects (RFC 8620 §5.3 / Message/set destroy).
    ///
    /// Permanently removes the listed message IDs from the account.
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn message_set_destroy(
        &self,
        session: &crate::jmap::Session,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_set_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let (call_id, req) = super::build_request("Message/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch query-result changes for Message since `since_query_state`
    /// (RFC 8620 §5.6 / Message/queryChanges).
    ///
    /// Returns which message IDs were removed from or added to the query
    /// result set since the given state. `max_changes` may be `None`.
    pub async fn message_query_changes(
        &self,
        session: &crate::jmap::Session,
        since_query_state: &str,
        max_changes: Option<u64>,
    ) -> Result<QueryChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceQueryState": since_query_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let (call_id, req) = super::build_request("Message/queryChanges", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
