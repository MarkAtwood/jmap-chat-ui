use super::{
    AddMemberInput, ChangesResponse, ChatCreateInput, ChatPatch, ChatQueryInput, GetResponse,
    QueryChangesResponse, QueryResponse, SetResponse, TypingResponse, UpdateMemberRoleInput,
};

impl super::SessionClient {
    /// Fetch Chat objects by IDs (RFC 8620 §5.1 / JMAP Chat §5 Chat/get).
    ///
    /// If `ids` is `None`, the server returns all Chats for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn chat_get(
        &self,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::Chat>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let req = super::build_request("Chat/get", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Query Chat IDs with optional filter (RFC 8620 §5.5 / JMAP Chat §5 Chat/query).
    ///
    /// Only keys that are `Some` in `input` are included in the filter object;
    /// an empty filter object is sent as JSON `null`.
    pub async fn chat_query(
        &self,
        input: &ChatQueryInput,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut filter = serde_json::Map::new();
        if let Some(ref k) = input.filter_kind {
            let kind_str = serde_json::to_value(k)?;
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
        let mut args = serde_json::json!({
            "accountId": account_id,
            "filter": filter_val,
        });
        if let Some(p) = input.position {
            args["position"] = p.into();
        }
        if let Some(l) = input.limit {
            args["limit"] = l.into();
        }
        let req = super::build_request("Chat/query", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Fetch changes to Chat objects since `since_state` (RFC 8620 §5.2 / Chat/changes).
    ///
    /// If `has_more_changes` is true in the response, call again with `new_state`
    /// as `since_state` until the flag is false.
    pub async fn chat_changes(
        &self,
        since_state: &str,
        max_changes: Option<u64>,
    ) -> Result<ChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceState": since_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let req = super::build_request("Chat/changes", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Send a typing indicator for a Chat (JMAP Chat §Chat/typing).
    ///
    /// Notifies other participants that the account is (or has stopped) typing.
    /// The server silently drops the event if `Chat.receiveTypingIndicators` is
    /// `false` for a recipient (direct/group chats); for channel chats the
    /// preference has no effect. The server SHOULD rate-limit to one call per
    /// account per chat per 3 seconds — excess calls MAY be silently discarded.
    /// Debouncing (send once per keypress, stop event on idle) is the caller's
    /// responsibility.
    pub async fn chat_typing(
        &self,
        chat_id: &str,
        typing: bool,
    ) -> Result<TypingResponse, crate::error::ClientError> {
        if chat_id.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "chat_typing: chat_id must not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "chatId": chat_id,
            "typing": typing,
        });
        let req = super::build_request("Chat/typing", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Fetch query-result changes for Chat since `since_query_state`
    /// (RFC 8620 §5.6 / Chat/queryChanges).
    ///
    /// Returns which Chat IDs were removed from or added to the query result set
    /// since the given state. `max_changes` may be `None`.
    pub async fn chat_query_changes(
        &self,
        since_query_state: &str,
        max_changes: Option<u64>,
    ) -> Result<QueryChangesResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
            "sinceQueryState": since_query_state,
        });
        if let Some(mc) = max_changes {
            args["maxChanges"] = mc.into();
        }
        let req = super::build_request("Chat/queryChanges", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Create a Chat (JMAP Chat §Chat/set create).
    ///
    /// Dispatches to the correct spec `kind` based on the `input` variant:
    /// `Direct`, `Group`, or `Channel`. When `client_id` inside the variant is
    /// `None`, a ULID is generated automatically.
    ///
    /// For `Direct` chats: if one already exists with the given `contact_id`,
    /// the server returns it in `SetResponse.updated` rather than `created`
    /// (dedup rule per spec).
    pub async fn chat_create(
        &self,
        input: &ChatCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let create_obj;
        let client_id_opt = match input {
            ChatCreateInput::Direct {
                client_id,
                contact_id,
            } => {
                if contact_id.is_empty() {
                    return Err(crate::error::ClientError::InvalidArgument(
                        "chat_create: contact_id may not be empty".into(),
                    ));
                }
                create_obj = serde_json::json!({
                    "kind": "direct",
                    "contactId": contact_id,
                });
                *client_id
            }
            ChatCreateInput::Group {
                client_id,
                name,
                member_ids,
                description,
                avatar_blob_id,
                message_expiry_seconds,
            } => {
                if name.is_empty() {
                    return Err(crate::error::ClientError::InvalidArgument(
                        "chat_create: name may not be empty".into(),
                    ));
                }
                let mut obj = serde_json::json!({
                    "kind": "group",
                    "name": name,
                    "memberIds": member_ids,
                });
                if let Some(d) = description {
                    obj["description"] = (*d).into();
                }
                if let Some(b) = avatar_blob_id {
                    obj["avatarBlobId"] = (*b).into();
                }
                if let Some(s) = message_expiry_seconds {
                    obj["messageExpirySeconds"] = (*s).into();
                }
                create_obj = obj;
                *client_id
            }
            ChatCreateInput::Channel {
                client_id,
                space_id,
                name,
                description,
            } => {
                if space_id.is_empty() {
                    return Err(crate::error::ClientError::InvalidArgument(
                        "chat_create: space_id may not be empty".into(),
                    ));
                }
                if name.is_empty() {
                    return Err(crate::error::ClientError::InvalidArgument(
                        "chat_create: name may not be empty".into(),
                    ));
                }
                let mut obj = serde_json::json!({
                    "kind": "channel",
                    "spaceId": space_id,
                    "name": name,
                });
                if let Some(d) = description {
                    obj["description"] = (*d).into();
                }
                create_obj = obj;
                *client_id
            }
        };
        let client_id = super::resolve_client_id(client_id_opt);
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { client_id: create_obj },
        });
        let req = super::build_request("Chat/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Update Chat properties (JMAP Chat §Chat/set update).
    ///
    /// Issues an `update` operation patching only the fields present in `patch`.
    /// Use `Patch::Set(v)` to set nullable fields, `Patch::Clear` to null-clear
    /// them, and `Patch::Keep` (default) to leave them unchanged. Slice fields
    /// default to `None` for no-change.
    ///
    /// If all fields are `Keep`/`None`, an empty patch is sent — RFC 8620 §5.3
    /// permits this; the server treats it as a no-op but still returns the chat
    /// in `updated`.
    pub async fn chat_update(
        &self,
        id: &str,
        patch: &ChatPatch<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        if id.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "chat_update: id may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let mut patch_map = serde_json::Map::new();

        if let Some(m) = patch.muted {
            patch_map.insert("muted".into(), m.into());
        }
        if let Some(entry) = patch.mute_until.map_entry()? {
            patch_map.insert("muteUntil".into(), entry);
        }
        if let Some(rti) = patch.receive_typing_indicators {
            patch_map.insert("receiveTypingIndicators".into(), rti.into());
        }
        if let Some(ids) = patch.pinned_message_ids {
            patch_map.insert(
                "pinnedMessageIds".into(),
                serde_json::Value::Array(
                    ids.iter()
                        .map(|id| serde_json::Value::String((*id).to_owned()))
                        .collect(),
                ),
            );
        }
        if let Some(s) = patch.message_expiry_seconds {
            patch_map.insert("messageExpirySeconds".into(), s.into());
        }
        if let Some(rs) = patch.receipt_sharing {
            patch_map.insert("receiptSharing".into(), rs.into());
        }
        if let Some(n) = patch.name {
            patch_map.insert("name".into(), n.into());
        }
        if let Some(entry) = patch.description.map_entry()? {
            patch_map.insert("description".into(), entry);
        }
        if let Some(entry) = patch.avatar_blob_id.map_entry()? {
            patch_map.insert("avatarBlobId".into(), entry);
        }
        if let Some(members) = patch.add_members {
            if !members.is_empty() {
                let arr = members
                    .iter()
                    .map(|m: &AddMemberInput<'_>| {
                        let mut obj = serde_json::json!({ "id": m.id });
                        if let Some(ref role) = m.role {
                            obj["role"] = serde_json::to_value(role)?;
                        }
                        Ok(obj)
                    })
                    .collect::<Result<Vec<_>, crate::error::ClientError>>()?;
                patch_map.insert("addMembers".into(), serde_json::Value::Array(arr));
            }
        }
        if let Some(rm) = patch.remove_members {
            if !rm.is_empty() {
                patch_map.insert(
                    "removeMembers".into(),
                    serde_json::Value::Array(
                        rm.iter()
                            .map(|id| serde_json::Value::String((*id).to_owned()))
                            .collect(),
                    ),
                );
            }
        }
        if let Some(umr) = patch.update_member_roles {
            if !umr.is_empty() {
                let arr = umr
                    .iter()
                    .map(|u: &UpdateMemberRoleInput<'_>| {
                        Ok(serde_json::json!({
                            "id": u.id,
                            "role": serde_json::to_value(&u.role)?,
                        }))
                    })
                    .collect::<Result<Vec<_>, crate::error::ClientError>>()?;
                patch_map.insert("updateMemberRoles".into(), serde_json::Value::Array(arr));
            }
        }

        let args = serde_json::json!({
            "accountId": account_id,
            "update": { id: serde_json::Value::Object(patch_map) },
        });
        let req = super::build_request("Chat/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Destroy Chat objects (RFC 8620 §5.3 / Chat/set destroy).
    ///
    /// Permanently removes the listed Chat IDs from the account.
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn chat_destroy(
        &self,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "chat_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let req = super::build_request("Chat/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }
}
