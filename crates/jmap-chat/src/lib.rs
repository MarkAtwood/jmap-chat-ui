pub mod auth;
pub mod blob;
pub mod client;
pub mod error;
pub mod jmap;
pub mod methods;
pub mod sse;
pub mod types;
pub mod utils;
pub mod ws;

// Core client types
#[doc(inline)]
pub use client::JmapChatClient;
#[doc(inline)]
pub use methods::SessionClient;

// Error type
#[doc(inline)]
pub use error::ClientError;

// Auth providers
#[doc(inline)]
pub use auth::{AuthProvider, BasicAuth, BearerAuth, CustomCaAuth, NoneAuth};

// JMAP core types
#[doc(inline)]
pub use jmap::{Id, Session, UTCDate};

// Commonly-used enum types
#[doc(inline)]
pub use types::{ChatStreamDataType, EndpointType, PushUrgency, QuotaScope};
