//! Stable provider boundary for platform orchestration.
//! Keep locking, validation, live-file backup/rollback and persistence in upstream.
use crate::services::{provider::SwitchResult, ProviderService};
use crate::{app_config::AppType, error::AppError, provider::Provider, store::AppState};
use serde_json::Value;

pub(crate) struct Providers;

impl Providers {
    pub(crate) fn current(state: &AppState, app: AppType) -> Result<String, AppError> {
        ProviderService::current(state, app)
    }

    pub(crate) fn add_without_switching(
        state: &AppState,
        app: AppType,
        provider: Provider,
    ) -> Result<bool, AppError> {
        ProviderService::add_without_switching(state, app, provider)
    }

    pub(crate) fn add_to_live(
        state: &AppState,
        app: AppType,
        provider: Provider,
    ) -> Result<bool, AppError> {
        ProviderService::add(state, app, provider, true)
    }

    pub(crate) fn dsh_native_exists(id: &str) -> Result<bool, AppError> {
        crate::dsh_config::provider_key_exists(id)
    }

    pub(crate) fn update(
        state: &AppState,
        app: AppType,
        original_id: Option<&str>,
        provider: Provider,
    ) -> Result<bool, AppError> {
        ProviderService::update(state, app, original_id, provider)
    }

    pub(crate) fn mark_subscription(
        state: &AppState,
        app: AppType,
        id: &str,
        origin: &str,
        user_id: i64,
        groups: &std::collections::HashSet<i64>,
    ) -> Result<bool, AppError> {
        ProviderService::update_metadata(state, app, id, |meta| {
            let Some(managed) = meta
                .get_mut("platformManaged")
                .and_then(Value::as_object_mut)
            else {
                return Ok(());
            };
            if managed.get("origin").and_then(Value::as_str) == Some(origin)
                && managed.get("userId").and_then(Value::as_i64) == Some(user_id)
                && managed
                    .get("groupId")
                    .and_then(Value::as_i64)
                    .is_some_and(|id| groups.contains(&id))
            {
                managed.insert(
                    "subscriptionType".into(),
                    Value::String("subscription".into()),
                );
            }
            Ok(())
        })
    }

    pub(crate) fn switch(
        state: &AppState,
        app: AppType,
        id: &str,
    ) -> Result<SwitchResult, AppError> {
        ProviderService::switch(state, app, id)
    }

    pub(crate) fn delete(state: &AppState, app: AppType, id: &str) -> Result<(), AppError> {
        ProviderService::delete(state, app, id)
    }

    pub(crate) fn read_live_settings(app: AppType) -> Result<Value, AppError> {
        ProviderService::read_live_settings(app)
    }
}
