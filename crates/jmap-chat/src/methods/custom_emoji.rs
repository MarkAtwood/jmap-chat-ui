use super::{
    ChangesResponse, CustomEmojiCreateInput, CustomEmojiQueryInput, GetResponse,
    QueryChangesResponse, QueryResponse, SetResponse,
};

impl crate::client::JmapChatClient {
    /// Fetch CustomEmoji objects by IDs (JMAP Chat §4.16 CustomEmoji/get).
    ///
    /// If `ids` is `None`, returns all CustomEmoji objects for the account.
    pub async fn custom_emoji_get(
        &self,
        session: &crate::jmap::Session,
        ids: Option<&[&str]>,
        properties: Option<&[&str]>,
    ) -> Result<GetResponse<crate::types::CustomEmoji>, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
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
        let (call_id, req) = super::build_request("CustomEmoji/changes", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Create or destroy CustomEmoji objects (RFC 8620 §5.3 / CustomEmoji/set).
    ///
    /// `input` describes a single emoji to create. `destroy` is a list of
    /// existing CustomEmoji IDs to delete.
    pub async fn custom_emoji_set(
        &self,
        session: &crate::jmap::Session,
        input: &CustomEmojiCreateInput<'_>,
        destroy: &[&str],
    ) -> Result<SetResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
        let mut create_obj = serde_json::json!({
            "name": input.name,
            "blobId": input.blob_id,
        });
        if let Some(sid) = input.space_id {
            create_obj["spaceId"] = sid.into();
        }
        let args = serde_json::json!({
            "accountId": account_id,
            "create": { input.client_id: create_obj },
            "destroy": destroy,
        });
        let (call_id, req) = super::build_request("CustomEmoji/set", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }

    /// Query CustomEmoji IDs (RFC 8620 §5.5 / CustomEmoji/query).
    pub async fn custom_emoji_query(
        &self,
        session: &crate::jmap::Session,
        input: &CustomEmojiQueryInput<'_>,
    ) -> Result<QueryResponse, crate::error::ClientError> {
        let (api_url, account_id) = Self::session_parts(session)?;
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
        let (call_id, req) = super::build_request("CustomEmoji/queryChanges", args);
        let resp = self.call(api_url, &req).await?;
        crate::client::extract_response(resp, call_id)
    }
}
