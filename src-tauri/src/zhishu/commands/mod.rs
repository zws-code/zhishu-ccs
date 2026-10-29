//! Stable IPC registration facade; command names and payloads remain unchanged.
mod config_history;
mod deepseek_harness;
pub use super::services::platform_auth::*;
pub use super::services::platform_models::*;
pub use super::storage::platform_login_history::*;
pub use config_history::*;
pub(crate) use deepseek_harness::*;
