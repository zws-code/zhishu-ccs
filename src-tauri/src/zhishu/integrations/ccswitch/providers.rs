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

    pub(crate) fn update(
        state: &AppState,
        app: AppType,
        original_id: Option<&str>,
        provider: Provider,
    ) -> Result<bool, AppError> {
        ProviderService::update(state, app, original_id, provider)
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
