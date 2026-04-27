pub(crate) mod auth;
pub(crate) mod blob;
pub(crate) mod client;
pub(crate) mod error;
pub(crate) mod jmap;
pub(crate) mod methods;
pub(crate) mod sse;
pub(crate) mod types;
pub(crate) mod utils;
pub(crate) mod ws;

// --- auth ---
pub use auth::{AuthProvider, BasicAuth, BearerAuth, CustomCaAuth, NoneAuth};

// --- blob ---
pub use blob::BlobUploadResponse;

// --- client ---
pub use client::JmapChatClient;

// --- error ---
pub use error::ClientError;

// --- jmap core types ---
pub use jmap::{
    AccountInfo, ChatCapability, ChatPushCapability, ChatWebSocketCapability, Id, Invocation,
    JmapRequest, JmapRequestBuilder, JmapResponse, ResultReference, Session, UTCDate,
    WebSocketCapability,
};

// --- domain types ---
pub use types::{
    Attachment, BodyType, Category, ChannelPermission, ChannelPermissionTargetType, Chat,
    ChatContact, ChatKind, ChatMember, ChatMemberRole, ChatMessageEntry, ChatMessagePush,
    ChatPresenceEvent, ChatPushConfig, ChatStreamDataType, ChatStreamDisable, ChatStreamEnable,
    ChatTypingEvent, ContactPresence, ContactPresenceFilter, CustomEmoji, DeliveryReceipt,
    DeliveryState, Endpoint, EndpointType, Mention, Message, MessageAction, MessageRevision,
    OwnerPresence, PresenceStatus, PushUrgency, QuotaScope, Reaction, ReadPosition, RichBody,
    SenderIdOrSelf, Space, SpaceBan, SpaceInvite, SpaceMember, SpaceRole, Span, SpanType,
};

// --- SSE types ---
pub use sse::{SseEvent, SseFrame};

// --- WebSocket types ---
pub use ws::{WsFrame, WsSession};

// --- utility functions ---
pub use utils::{format_receipt_timestamp, format_receipt_timestamp_at};

// --- method response/input/patch types ---
pub use methods::blob::{BlobConvertResponse, BlobLookupEntry, BlobLookupResponse};
pub use methods::quota::Quota;
pub use methods::{
    AddMemberInput, AddedItem, ChangesResponse, ChatContactPatch, ChatContactQueryInput,
    ChatCreateInput, ChatPatch, ChatQueryInput, ContactSortProperty, CustomEmojiCreateInput,
    CustomEmojiQueryInput, GetResponse, MessageCreateInput, MessagePatch, MessageQueryInput, Patch,
    PresenceStatusPatch, PushSubscriptionCreateInput, PushSubscriptionCreateResponse,
    QueryChangesResponse, QueryResponse, ReactionChange, SessionClient, SetError, SetResponse,
    SpaceAddChannelInput, SpaceAddMemberInput, SpaceBanCreateInput, SpaceCreateInput,
    SpaceInviteCreateInput, SpaceJoinInput, SpaceJoinResponse, SpacePatch, SpaceQueryInput,
    SpaceUpdateMemberInput, TypingResponse, UpdateMemberRoleInput,
};
