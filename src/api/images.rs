use leptos::prelude::*;
use shared::errors::docker::DockerError;
use shared::images::{HubResult, HubTag, LocalImage};

#[cfg(feature = "ssr")]
use super::docker::require_admin;
#[cfg(feature = "ssr")]
use crate::state::AppState;

#[server(endpoint = "poll/images")]
pub async fn get_local_images() -> Result<Vec<LocalImage>, DockerError> {
    require_admin()?;
    backend::images::local(&AppState::get()?.docker).await
}

#[server]
pub async fn search_hub(term: String) -> Result<Vec<HubResult>, DockerError> {
    require_admin()?;
    let term = term.trim().to_string();
    if term.is_empty() || term.len() > 100 {
        return Ok(Vec::new());
    }
    backend::images::search(&AppState::get()?.docker, &term).await
}

#[server]
pub async fn hub_tags(repo: String) -> Result<Vec<HubTag>, DockerError> {
    require_admin()?;
    backend::images::tags(repo.trim()).await
}

#[server]
pub async fn delete_image(reference: String) -> Result<(), DockerError> {
    let admin = require_admin()?;
    let valid = reference
        .split_once(':')
        .map(|(repo, tag)| shared::images::valid_repo(repo) && shared::images::valid_tag(tag))
        .unwrap_or_else(|| reference.chars().all(|c| c.is_ascii_hexdigit()));
    if !valid {
        return Err(DockerError::Failed("That isn't a valid image reference.".into()));
    }
    let result = backend::images::remove(&AppState::get()?.docker, &reference).await;
    super::audit::record(
        "image.remove",
        Some(admin.email),
        Some(reference),
        result.as_ref().err().map(|e| e.to_string()),
        result.is_ok(),
    )
    .await;
    result
}
