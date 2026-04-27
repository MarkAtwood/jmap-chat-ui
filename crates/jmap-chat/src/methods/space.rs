use super::{
    ChangesResponse, GetResponse, QueryChangesResponse, QueryResponse, SetResponse,
    SpaceAddChannelInput, SpaceAddMemberInput, SpaceCreateInput, SpaceJoinInput, SpaceJoinResponse,
    SpaceQueryInput, SpaceUpdateInput, SpaceUpdateMemberInput,
};

impl crate::client::JmapChatClient {
    /// Fetch Space objects by IDs (RFC 8620 §5.1 / JMAP Chat §Space/get).
    ///
    /// If `ids` is `None`, the server returns all Spaces for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn space_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::Space>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let (call_id, req) = super::build_request("Space/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch changes to Space objects since `since_state` (RFC 8620 §5.2 / Space/changes).
    ///
    /// If `has_more_changes` is true in the response, call again with `new_state`
    /// as `since_state` until the flag is false.
    pub async fn space_changes(
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
        let (call_id, req) = super::build_request("Space/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Destroy Space objects (RFC 8620 §5.3 / Space/set destroy).
    ///
    /// Permanently removes the listed Space IDs from the account.
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn space_set_destroy(
        &self,
        session: &crate::jmap::Session,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "space_set_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = Self::session_parts(session)?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let (call_id, req) = super::build_request("Space/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Query Space IDs with optional filter (RFC 8620 §5.5 / JMAP Chat §Space/query).
    ///
    /// Only keys that are `Some` in `input` are included in the filter object;
    /// an empty filter is sent as JSON `null`.
    pub async fn space_query(
        &self,
        session: &crate::jmap::Session,
        input: &SpaceQueryInput<'_>,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut filter = serde_json::Map::new();
        if let Some(n) = input.filter_name {
            filter.insert("name".into(), n.into());
        }
        if let Some(p) = input.filter_is_public {
            filter.insert("isPublic".into(), p.into());
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
        let (call_id, req) = super::build_request("Space/query", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch query-result changes for Space since `since_query_state`
    /// (RFC 8620 §5.6 / Space/queryChanges).
    ///
    /// Returns which Space IDs were removed from or added to the query result set
    /// since the given state. `max_changes` may be `None`.
    pub async fn space_query_changes(
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
        let (call_id, req) = super::build_request("Space/queryChanges", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create a new Space (JMAP Chat §Space/set create).
    ///
    /// `client_id` is a caller-supplied ULID used as the creation key. The server
    /// maps it to the server-assigned Space id in `SetResponse.created`.
    pub async fn space_create(
        &self,
        session: &crate::jmap::Session,
        input: &SpaceCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut create_obj = serde_json::json!({ "name": input.name });
        if let Some(d) = input.description {
            create_obj["description"] = d.into();
        }
        if let Some(b) = input.icon_blob_id {
            create_obj["iconBlobId"] = b.into();
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { input.client_id: create_obj },
        });
        let (call_id, req) = super::build_request("Space/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Join a Space via invite code or direct ID (JMAP Chat §Space/join).
    ///
    /// `input` selects exactly one join path; the enum makes invalid inputs
    /// unrepresentable at the type level.
    pub async fn space_join(
        &self,
        session: &crate::jmap::Session,
        input: &SpaceJoinInput<'_>,
    ) -> Result<SpaceJoinResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut args = serde_json::json!({ "accountId": account_id });
        match input {
            SpaceJoinInput::InviteCode(ic) => {
                args["inviteCode"] = (*ic).into();
            }
            SpaceJoinInput::SpaceId(sid) => {
                args["spaceId"] = (*sid).into();
            }
        }
        let (call_id, req) = super::build_request("Space/join", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Update Space properties (JMAP Chat §Space/set update).
    ///
    /// Issues an `update` operation patching only the fields present in `input`.
    /// Nullable fields (`description`, `icon_blob_id`) accept `Some(None)` to clear
    /// and `Some(Some(v))` to set. Slice fields default to `&[]` for no-change.
    ///
    /// If all fields are absent/empty, an empty patch is sent — RFC 8620 §5.3
    /// permits this; the server treats it as a no-op but still returns the Space
    /// in `updated`.
    ///
    /// **Out of scope**: `addRoles`, `removeRoles`, `updateRoles`,
    /// `updateChannels`, `addCategories`, `removeCategories`, `updateCategories`
    /// are not included. Role and category management will be added in a future
    /// iteration of this API.
    pub async fn space_set_update(
        &self,
        session: &crate::jmap::Session,
        input: &SpaceUpdateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut patch = serde_json::Map::new();

        if let Some(n) = input.name {
            patch.insert("name".into(), n.into());
        }
        if let Some(desc) = &input.description {
            patch.insert(
                "description".into(),
                desc.map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(ib) = &input.icon_blob_id {
            patch.insert(
                "iconBlobId".into(),
                ib.map(serde_json::Value::from)
                    .unwrap_or(serde_json::Value::Null),
            );
        }
        if let Some(ip) = input.is_public {
            patch.insert("isPublic".into(), ip.into());
        }
        if let Some(ipp) = input.is_publicly_previewable {
            patch.insert("isPubliclyPreviewable".into(), ipp.into());
        }
        if !input.add_members.is_empty() {
            let arr: Vec<serde_json::Value> = input
                .add_members
                .iter()
                .map(|m: &SpaceAddMemberInput<'_>| {
                    let mut obj = serde_json::json!({ "id": m.id });
                    if let Some(role_ids) = m.role_ids {
                        obj["roleIds"] = serde_json::Value::Array(
                            role_ids
                                .iter()
                                .map(|id| serde_json::Value::String((*id).to_owned()))
                                .collect(),
                        );
                    }
                    obj
                })
                .collect();
            patch.insert("addMembers".into(), serde_json::Value::Array(arr));
        }
        if !input.remove_members.is_empty() {
            patch.insert(
                "removeMembers".into(),
                serde_json::Value::Array(
                    input
                        .remove_members
                        .iter()
                        .map(|id| serde_json::Value::String((*id).to_owned()))
                        .collect(),
                ),
            );
        }
        if !input.update_members.is_empty() {
            let arr: Vec<serde_json::Value> = input
                .update_members
                .iter()
                .map(|u: &SpaceUpdateMemberInput<'_>| {
                    let mut obj = serde_json::json!({ "id": u.id });
                    if let Some(role_ids) = u.role_ids {
                        obj["roleIds"] = serde_json::Value::Array(
                            role_ids
                                .iter()
                                .map(|id| serde_json::Value::String((*id).to_owned()))
                                .collect(),
                        );
                    }
                    if let Some(nick) = &u.nick {
                        obj["nick"] = nick
                            .map(serde_json::Value::from)
                            .unwrap_or(serde_json::Value::Null);
                    }
                    obj
                })
                .collect();
            patch.insert("updateMembers".into(), serde_json::Value::Array(arr));
        }
        if !input.add_channels.is_empty() {
            let arr: Vec<serde_json::Value> = input
                .add_channels
                .iter()
                .map(|c: &SpaceAddChannelInput<'_>| {
                    let mut obj = serde_json::json!({ "name": c.name });
                    if let Some(cat) = c.category_id {
                        obj["categoryId"] = cat.into();
                    }
                    if let Some(pos) = c.position {
                        obj["position"] = pos.into();
                    }
                    if let Some(t) = c.topic {
                        obj["topic"] = t.into();
                    }
                    obj
                })
                .collect();
            patch.insert("addChannels".into(), serde_json::Value::Array(arr));
        }
        if !input.remove_channels.is_empty() {
            patch.insert(
                "removeChannels".into(),
                serde_json::Value::Array(
                    input
                        .remove_channels
                        .iter()
                        .map(|id| serde_json::Value::String((*id).to_owned()))
                        .collect(),
                ),
            );
        }

        let args = serde_json::json!({
            "accountId": account_id,
            "update": { input.id: serde_json::Value::Object(patch) },
        });
        let (call_id, req) = super::build_request("Space/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
