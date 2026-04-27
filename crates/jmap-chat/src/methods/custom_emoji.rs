use super::{
    ChangesResponse, CustomEmojiCreateInput, CustomEmojiQueryInput, GetResponse,
    QueryChangesResponse, QueryResponse, SetResponse,
};

impl super::SessionClient<'_> {
    /// Fetch CustomEmoji objects by IDs (JMAP Chat §4.16 CustomEmoji/get).
    ///
    /// If `ids` is `None`, returns all CustomEmoji objects for the account.
    pub async fn custom_emoji_get(
        &self,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::CustomEmoji>, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "ids": ids,
            "properties": properties,
        });
        let (call_id, req) = super::build_request("CustomEmoji/get", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch changes to CustomEmoji objects since `since_state` (RFC 8620 §5.2 / CustomEmoji/changes).
    pub async fn custom_emoji_changes(
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
        let (call_id, req) = super::build_request("CustomEmoji/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create a CustomEmoji (RFC 8620 §5.3 / CustomEmoji/set create).
    ///
    /// When `input.client_id` is `None`, a ULID is generated automatically.
    pub async fn custom_emoji_create(
        &self,
        input: &CustomEmojiCreateInput<'_>,
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut create_obj = serde_json::json!({
            "name": input.name,
            "blobId": input.blob_id,
        });
        if let Some(sid) = input.space_id {
            create_obj["spaceId"] = sid.into();
        }
        let mut buf = String::new();
        let client_id = super::resolve_client_id(input.client_id, &mut buf);
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { client_id: create_obj },
        });
        let (call_id, req) = super::build_request("CustomEmoji/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Destroy CustomEmoji objects (RFC 8620 §5.3 / CustomEmoji/set destroy).
    ///
    /// `ids` must be non-empty; the guard fires before any network call.
    pub async fn custom_emoji_destroy(
        &self,
        ids: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        if ids.is_empty() {
            return Err(crate::error::ClientError::InvalidArgument(
                "custom_emoji_destroy: ids may not be empty".into(),
            ));
        }
        let (api_url, account_id) = self.session_parts()?;
        let args = serde_json::json!({
            "accountId": account_id,
            "destroy": ids,
        });
        let (call_id, req) = super::build_request("CustomEmoji/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Query CustomEmoji IDs (RFC 8620 §5.5 / CustomEmoji/query).
    pub async fn custom_emoji_query(
        &self,
        input: &CustomEmojiQueryInput<'_>,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        let (api_url, account_id) = self.session_parts()?;
        let mut args = serde_json::json!({
            "accountId": account_id,
        });
        if let Some(sid) = input.filter_space_id {
            args["filter"] = serde_json::json!({"spaceId": sid});
        }
        if let Some(p) = input.position {
            args["position"] = p.into();
        }
        if let Some(l) = input.limit {
            args["limit"] = l.into();
        }
        let (call_id, req) = super::build_request("CustomEmoji/query", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Fetch query-result changes for CustomEmoji since `since_query_state`
    /// (RFC 8620 §5.6 / CustomEmoji/queryChanges).
    pub async fn custom_emoji_query_changes(
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
        let (call_id, req) = super::build_request("CustomEmoji/queryChanges", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
