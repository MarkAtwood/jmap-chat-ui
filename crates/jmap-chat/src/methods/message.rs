use super::{
    ChangesResponse, GetResponse, MessageCreateInput, MessagePatch, MessageQueryInput,
    QueryChangesResponse, QueryResponse, ReactionChange, SetResponse,
};

impl super::SessionClient {
    /// Fetch Message objects by IDs (RFC 8620 §5.1 / JMAP Chat §5 Message/get).
    ///
    /// `ids` is required (non-empty); fetching all messages is impractical.
    /// Pass `properties: None` to return all fields.
    pub async fn message_get(
        &self,
        ids: &[&str],
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::Message>, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_get: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let req = super::build_request("Message/get", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
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
        let (api_url, account_id) = self.session_parts()?;
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
            if tid.is_empty() {
                return Err(crate::error::ClientError::InvalidArgument(
                    "message_query: thread_root_id may not be empty".into(),
                ));
            }
            filter.insert("threadRootId".into(), tid.into());
        }
        if let Some(a) = input.after {
            filter.insert("after".into(), a.as_str().into());
        }
        if let Some(b) = input.before {
            filter.insert("before".into(), b.as_str().into());
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
        let req = super::build_request("Message/query", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Fetch changes to Message objects since `since_state` (RFC 8620 §5.2 / Message/changes).
    pub async fn message_changes(
        &self,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<ChangesResponse, crate::error::ClientError> {
        if since_state.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_changes: since_state may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let req = super::build_request("Message/changes", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Create (send) a new Message (RFC 8620 §5.3 / JMAP Chat §5 Message/set).
    ///
    /// When `input.client_id` is `None`, a ULID is generated automatically.
    /// The server maps the creation key to the server-assigned Message id in
    /// `SetResponse.created`.
    pub async fn message_create(
        &self,
        input: &MessageCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        if input.chat_id.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_create: chat_id may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let client_id = super::resolve_client_id(input.client_id);
        // Used twice: as the json! key and for not_created.get() lookup — borrow from Cow to avoid double-move.
        let client_id_str: &str = &client_id;
        let mut create_obj = serde_json::json!({
            "chatId": input.chat_id,
            "body": input.body,
            "bodyType": serde_json::to_value(&input.body_type)?,
            "sentAt": input.sent_at.as_str(),
        });
        if let Some(rt) = input.reply_to {
            if rt.is_empty() {
                return Err(crate::error::ClientError::InvalidArgument(
                    "message_create: reply_to may not be empty".into(),
                ));
            }
            create_obj["replyTo"] = rt.into();
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { client_id_str: create_obj },
        });
        let req = super::build_request("Message/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        let set_resp: SetResponse = crate::client::extract_response(resp, super::CALL_ID)?;
        if let Some(not_created) = &set_resp.not_created {
            if let Some(err) = not_created.get(client_id_str) {
                if err.error_type == "rateLimited" {
                    let retry_after = err.server_retry_after.clone().ok_or_else(|| {
                        crate::error::ClientError::Parse(
                            "rateLimited SetError missing serverRetryAfter".into(),
                        )
                    })?;
                    return Err(crate::error::ClientError::RateLimited { retry_after });
                }
            }
        }
        Ok(set_resp)
    }

    /// Update Message properties (RFC 8620 §5.3 / JMAP Chat §4.5 Message/set).
    ///
    /// Issues an `update` operation patching only the fields present in `patch`.
    /// Supports body edits (author-only), reaction changes (JSON Pointer patch on
    /// `reactions` map), read-receipt updates (`readAt`), and chat-level deletion
    /// (`deletedAt` / `deletedForAll`).
    ///
    /// If all optional fields are `None`, an empty patch object is sent. RFC 8620
    /// §5.3 permits this; the server treats it as a no-op but still returns the
    /// object in `updated`.
    pub async fn message_update(
        &self,
        id: &str,
        patch: &MessagePatch<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        if id.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_update: id may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let mut patch_map = serde_json::Map::new();
        if let Some(b) = patch.body {
            patch_map.insert("body".into(), b.into());
        }
        if let Some(bt) = &patch.body_type {
            patch_map.insert("bodyType".into(), serde_json::to_value(bt)?);
        }
        if let Some(ra) = patch.read_at {
            patch_map.insert("readAt".into(), ra.as_str().into());
        }
        if let Some(da) = patch.deleted_at {
            patch_map.insert("deletedAt".into(), da.as_str().into());
        }
        if let Some(dfa) = patch.deleted_for_all {
            patch_map.insert("deletedForAll".into(), dfa.into());
        }
        for change in patch.reaction_changes.unwrap_or(&[]) {
            match change {
                ReactionChange::Add {
                    sender_reaction_id,
                    emoji,
                    sent_at,
                } => {
                    if sender_reaction_id.is_empty() {
                        return Err(crate::error::ClientError::InvalidArgument(
                            "message_update: sender_reaction_id may not be empty".into(),
                        ));
                    }
                    if sender_reaction_id.contains('/') || sender_reaction_id.contains('~') {
                        return Err(crate::error::ClientError::InvalidArgument(
                            "message_update: sender_reaction_id must not contain '/' or '~' \
                             (RFC 6901 JSON Pointer special characters)"
                                .into(),
                        ));
                    }
                    patch_map.insert(
                        format!("reactions/{sender_reaction_id}"),
                        serde_json::json!({"emoji": emoji, "sentAt": sent_at.as_str()}),
                    );
                }
                ReactionChange::Remove { sender_reaction_id } => {
                    if sender_reaction_id.is_empty() {
                        return Err(crate::error::ClientError::InvalidArgument(
                            "message_update: sender_reaction_id may not be empty".into(),
                        ));
                    }
                    if sender_reaction_id.contains('/') || sender_reaction_id.contains('~') {
                        return Err(crate::error::ClientError::InvalidArgument(
                            "message_update: sender_reaction_id must not contain '/' or '~' \
                             (RFC 6901 JSON Pointer special characters)"
                                .into(),
                        ));
                    }
                    patch_map.insert(
                        format!("reactions/{sender_reaction_id}"),
                        serde_json::Value::Null,
                    );
                }
            }
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "update": { id: serde_json::Value::Object(patch_map) },
        });
        let req = super::build_request("Message/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Destroy Message objects (RFC 8620 §5.3 / Message/set destroy).
    ///
    /// Permanently removes the listed message IDs from the account.
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn message_destroy(
        &self,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let req = super::build_request("Message/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Fetch query-result changes for Message since `since_query_state`
    /// (RFC 8620 §5.6 / Message/queryChanges).
    ///
    /// Returns which message IDs were removed from or added to the query
    /// result set since the given state. `max_changes` may be `None`.
    pub async fn message_query_changes(
        &self,
        since_query_state: &str,
        max_changes: Option<u64>,
    ) -> Result<QueryChangesResponse, crate::error::ClientError> {
        if since_query_state.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "message_query_changes: since_query_state may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceQueryState": since_query_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let req = super::build_request("Message/queryChanges", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }
}
