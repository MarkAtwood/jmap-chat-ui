use super::{
    GetResponse, PresenceStatusPatch, PushSubscriptionCreateInput, PushSubscriptionSetResponse,
    SetResponse,
};

impl super::SessionClient<'_> {
    /// Fetch ReadPosition objects by IDs (JMAP Chat §5 ReadPosition/get).
    ///
    /// If `ids` is `None`, returns all ReadPosition records for the account.
    /// The server creates one ReadPosition per Chat automatically.
    pub async fn read_position_get(
        &self,
        ids: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::ReadPosition>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
        });
        let (call_id, req) = super::build_request("ReadPosition/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Update the read position for a Chat (JMAP Chat §5 ReadPosition/set).
    ///
    /// `read_position_id` is the server-assigned ReadPosition.id (from
    /// `read_position_get`). `last_read_message_id` is the Message.id of the
    /// most recent message read. The server updates `lastReadAt` and
    /// recomputes `Chat.unreadCount`.
    ///
    /// `create` and `destroy` are forbidden by the spec; only `update` is issued.
    pub async fn read_position_update(
        &self,
        read_position_id: &str,
        last_read_message_id: &str,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "update": {
                read_position_id: { "lastReadMessageId": last_read_message_id }
            },
        });
        let (call_id, req) = super::build_request("ReadPosition/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch the singleton PresenceStatus record (JMAP Chat §5 PresenceStatus/get).
    ///
    /// Per spec there is exactly one PresenceStatus per account; `ids: null`
    /// retrieves it.
    pub async fn presence_status_get(
        &self,
    ) -> Result<GetResponse<crate::types::PresenceStatus>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": None::<&[&str]>,
        });
        let (call_id, req) = super::build_request("PresenceStatus/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch changes to ReadPosition records since `since_state` (JMAP Chat §5 ReadPosition/changes).
    ///
    /// `max_changes` may be `None` to let the server choose the limit (RFC 8620 §5.2).
    pub async fn read_position_changes(
        &self,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<super::ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let (call_id, req) = super::build_request("ReadPosition/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Update the PresenceStatus record (JMAP Chat §5 PresenceStatus/set).
    ///
    /// Only `update` is issued; `create` and `destroy` are forbidden by the spec.
    /// Fields absent from `patch` (i.e. `Patch::Keep` or `None`) are omitted from
    /// the patch and left unchanged server-side.
    pub async fn presence_status_update(
        &self,
        id: &str,
        patch: &PresenceStatusPatch<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut patch_map = serde_json::Map::new();
        if let Some(p) = &patch.presence {
            patch_map.insert("presence".into(), serde_json::to_value(p)?);
        }
        if let Some(entry) = patch.status_text.map_entry()? {
            patch_map.insert("statusText".into(), entry);
        }
        if let Some(entry) = patch.status_emoji.map_entry()? {
            patch_map.insert("statusEmoji".into(), entry);
        }
        if let Some(entry) = patch.expires_at.map_entry()? {
            patch_map.insert("expiresAt".into(), entry);
        }
        if let Some(rs) = patch.receipt_sharing {
            patch_map.insert("receiptSharing".into(), rs.into());
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "update": { id: serde_json::Value::Object(patch_map) },
        });
        let (call_id, req) = super::build_request("PresenceStatus/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch changes to PresenceStatus records since `since_state` (JMAP Chat §5 PresenceStatus/changes).
    ///
    /// `max_changes` may be `None` to let the server choose the limit (RFC 8620 §5.2).
    pub async fn presence_status_changes(
        &self,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<super::ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let (call_id, req) = super::build_request("PresenceStatus/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create a PushSubscription with the optional `chatPush` extension
    /// (RFC 8620 §7.2 / draft-atwood-jmap-chat-push-00 §3).
    ///
    /// PushSubscriptions are account-independent: no `accountId` is included
    /// in the request (RFC 8620 §7.2). When `input.chat_push` is `Some`, the
    /// `using` array includes `urn:ietf:params:jmap:chat:push` (RFC 8620 §3.3:
    /// capabilities MUST only be declared when used); otherwise `urn:ietf:params:jmap:core`
    /// alone is used.
    ///
    /// **Scope**: this method issues a `create` operation only. RFC 8620 §7.2
    /// also defines `update` (e.g., extending `expires`) and `destroy` (unsubscribe);
    /// those are not yet implemented.
    ///
    /// When `input.client_id` is `None`, a ULID is generated automatically.
    pub async fn push_subscription_set(
        &self,
        input: &PushSubscriptionCreateInput<'_>,
    ) -> Result<PushSubscriptionSetResponse, crate::error::ClientError> {
        let api_url = self.api_url();
        let mut buf = String::new();
        let client_id = super::resolve_client_id(input.client_id, &mut buf);
        let mut create_obj = serde_json::json!({
            "deviceClientId": input.device_client_id,
            "url": input.url,
        });
        if let Some(exp) = input.expires {
            create_obj["expires"] = exp.as_str().into();
        }
        if let Some(types) = input.types {
            create_obj["types"] = serde_json::Value::Array(
                types
                    .iter()
                    .map(|t| serde_json::Value::String((*t).to_owned()))
                    .collect(),
            );
        }
        let has_chat_push = input.chat_push.is_some();
        if let Some(cp) = input.chat_push {
            let mut seen = std::collections::HashSet::new();
            for (account_id, _) in cp {
                if !seen.insert(*account_id) {
                    return Err(crate::error::ClientError::InvalidArgument(format!(
                        "push_subscription_set: duplicate accountId '{}' in chat_push",
                        account_id
                    )));
                }
            }
            let mut chat_push_map = serde_json::Map::new();
            for (account_id, config) in cp {
                chat_push_map.insert((*account_id).to_owned(), serde_json::to_value(config)?);
            }
            create_obj["chatPush"] = serde_json::Value::Object(chat_push_map);
        }
        let args = serde_json::json!({
            "create": { client_id: create_obj }
        });
        // RFC 8620 §3.3: only declare the chatPush capability when it is actually used.
        let mut using = vec!["urn:ietf:params:jmap:core".to_string()];
        if has_chat_push {
            using.push("urn:ietf:params:jmap:chat:push".to_string());
        }
        let req = crate::jmap::JmapRequest {
            using,
            method_calls: vec![("PushSubscription/set".to_string(), args, "r1".to_string())],
        };
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, "r1")
    }
}
