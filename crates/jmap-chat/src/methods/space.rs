use super::{
    ChangesResponse, GetResponse, QueryChangesResponse, QueryResponse, SetResponse,
    SpaceAddChannelInput, SpaceAddMemberInput, SpaceCreateInput, SpaceJoinInput, SpaceJoinResponse,
    SpacePatch, SpaceQueryInput, SpaceUpdateMemberInput,
};

impl super::SessionClient {
    /// Fetch Space objects by IDs (RFC 8620 §5.1 / JMAP Chat §Space/get).
    ///
    /// If `ids` is `None`, the server returns all Spaces for the account.
    /// Pass `properties: None` to return all fields.
    pub async fn space_get(
        &self,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::Space>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let req = super::build_request("Space/get", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Fetch changes to Space objects since `since_state` (RFC 8620 §5.2 / Space/changes).
    ///
    /// If `has_more_changes` is true in the response, call again with `new_state`
    /// as `since_state` until the flag is false.
    pub async fn space_changes(
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
        let req = super::build_request("Space/changes", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Destroy Space objects (RFC 8620 §5.3 / Space/set destroy).
    ///
    /// Permanently removes the listed Space IDs from the account.
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn space_destroy(
        &self,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "space_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let req = super::build_request("Space/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Query Space IDs with optional filter (RFC 8620 §5.5 / JMAP Chat §Space/query).
    ///
    /// Only keys that are `Some` in `input` are included in the filter object;
    /// an empty filter is sent as JSON `null`.
    pub async fn space_query(
        &self,
        input: &SpaceQueryInput<'_>,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
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
        let req = super::build_request("Space/query", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Fetch query-result changes for Space since `since_query_state`
    /// (RFC 8620 §5.6 / Space/queryChanges).
    ///
    /// Returns which Space IDs were removed from or added to the query result set
    /// since the given state. `max_changes` may be `None`.
    pub async fn space_query_changes(
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
        let req = super::build_request("Space/queryChanges", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Create a new Space (JMAP Chat §Space/set create).
    ///
    /// When `input.client_id` is `None`, a ULID is generated automatically.
    /// The server maps the creation key to the server-assigned Space id in
    /// `SetResponse.created`.
    pub async fn space_create(
        &self,
        input: &SpaceCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        if input.name.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "space_create: name may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let client_id = super::resolve_client_id(input.client_id);
        let mut create_obj = serde_json::json!({ "name": input.name });
        if let Some(d) = input.description {
            create_obj["description"] = d.into();
        }
        if let Some(b) = input.icon_blob_id {
            create_obj["iconBlobId"] = b.into();
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { client_id: create_obj },
        });
        let req = super::build_request("Space/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Join a Space via invite code or direct ID (JMAP Chat §Space/join).
    ///
    /// `input` selects exactly one join path; the enum makes invalid inputs
    /// unrepresentable at the type level.
    pub async fn space_join(
        &self,
        input: &SpaceJoinInput<'_>,
    ) -> Result<SpaceJoinResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({ "accountId": account_id });
        match input {
            SpaceJoinInput::InviteCode(ic) => {
                args["inviteCode"] = (*ic).into();
            }
            SpaceJoinInput::SpaceId(sid) => {
                args["spaceId"] = (*sid).into();
            }
        }
        let req = super::build_request("Space/join", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }

    /// Update Space properties (JMAP Chat §Space/set update).
    ///
    /// Issues an `update` operation patching only the fields present in `patch`.
    /// Use `Patch::Set(v)` to set nullable fields, `Patch::Clear` to null-clear
    /// them, and `Patch::Keep` (default) to leave them unchanged. Slice fields
    /// default to `None` for no-change.
    ///
    /// **Out of scope**: `addRoles`, `removeRoles`, `updateRoles`,
    /// `updateChannels`, `addCategories`, `removeCategories`, `updateCategories`
    /// are not included. Role and category management will be added in a future
    /// iteration of this API.
    pub async fn space_update(
        &self,
        id: &str,
        patch: &SpacePatch<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        if id.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "space_update: id may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let mut patch_map = serde_json::Map::new();

        if let Some(n) = patch.name {
            patch_map.insert("name".into(), n.into());
        }
        if let Some(entry) = patch.description.map_entry()? {
            patch_map.insert("description".into(), entry);
        }
        if let Some(entry) = patch.icon_blob_id.map_entry()? {
            patch_map.insert("iconBlobId".into(), entry);
        }
        if let Some(ip) = patch.is_public {
            patch_map.insert("isPublic".into(), ip.into());
        }
        if let Some(ipp) = patch.is_publicly_previewable {
            patch_map.insert("isPubliclyPreviewable".into(), ipp.into());
        }
        if let Some(members) = patch.add_members {
            if !members.is_empty() {
                let arr: Vec<serde_json::Value> = members
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
        if let Some(um) = patch.update_members {
            if !um.is_empty() {
                let arr: Vec<serde_json::Value> = um
                    .iter()
                    .map(|u: &SpaceUpdateMemberInput<'_>| -> Result<serde_json::Value, crate::error::ClientError> {
                        let mut obj = serde_json::json!({ "id": u.id });
                        if let Some(role_ids) = u.role_ids {
                            obj["roleIds"] = serde_json::Value::Array(
                                role_ids
                                    .iter()
                                    .map(|id| serde_json::Value::String((*id).to_owned()))
                                    .collect(),
                            );
                        }
                        if let Some(entry) = u.nick.map_entry()? {
                            obj["nick"] = entry;
                        }
                        Ok(obj)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                patch_map.insert("updateMembers".into(), serde_json::Value::Array(arr));
            }
        }
        if let Some(ac) = patch.add_channels {
            if !ac.is_empty() {
                let arr: Vec<serde_json::Value> = ac
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
                patch_map.insert("addChannels".into(), serde_json::Value::Array(arr));
            }
        }
        if let Some(rc) = patch.remove_channels {
            if !rc.is_empty() {
                patch_map.insert(
                    "removeChannels".into(),
                    serde_json::Value::Array(
                        rc.iter()
                            .map(|id| serde_json::Value::String((*id).to_owned()))
                            .collect(),
                    ),
                );
            }
        }

        let args = serde_json::json!({
            "accountId": account_id,
            "update": { id: serde_json::Value::Object(patch_map) },
        });
        let req = super::build_request("Space/set", args, super::USING_CHAT);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, super::CALL_ID)
    }
}
