use std::sync::Arc;

use axum::extract::FromRef;
use backend::db::Db;
use backend::docker::Docker;
use leptos::prelude::*;
use shared::errors::InternalServerError;

#[derive(Clone, FromRef)]
pub struct AppState {
    pub leptos_options: LeptosOptions,
    pub db: Db,
    pub docker: Docker,
    pub stacks: backend::stacks::Stacks,
    pub allowed_hosts: Arc<[String]>,
}

impl AppState {
    pub fn get() -> Result<Self, InternalServerError> {
        use_context::<Self>()
            .ok_or_else(|| InternalServerError::new("AppState missing from context"))
    }
}
