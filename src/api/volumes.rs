use leptos::prelude::*;
use shared::errors::docker::DockerError;
use shared::volumes::Listing;

#[cfg(feature = "ssr")]
use super::docker::require_admin;
#[cfg(feature = "ssr")]
use crate::state::AppState;

#[server(endpoint = "volumes/list")]
pub async fn list_files(stack: String, storage: String, path: String) -> Result<Listing, DockerError> {
    require_admin()?;
    let state = AppState::get()?;
    let volume = state.stacks.volume_of(&stack, &storage)?;
    backend::volumes::list(&state.docker, &volume, &path).await
}

#[server(endpoint = "volumes/others")]
pub async fn other_volumes(stack: String, storage: String) -> Result<Vec<String>, DockerError> {
    require_admin()?;
    let state = AppState::get()?;
    let volume = state.stacks.volume_of(&stack, &storage)?;
    backend::volumes::others(&state.docker, &volume).await
}

#[server]
pub async fn make_folder(stack: String, storage: String, path: String) -> Result<(), DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let volume = state.stacks.volume_of(&stack, &storage)?;
    let result = backend::volumes::make_folder(&state.docker, &volume, &path).await;
    audit("volume.mkdir", admin.email, format!("{volume}: {path}"), &result).await;
    result
}

#[server]
pub async fn delete_path(stack: String, storage: String, path: String) -> Result<(), DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let volume = state.stacks.volume_of(&stack, &storage)?;
    let result = backend::volumes::delete(&state.docker, &volume, &path).await;
    audit("volume.delete", admin.email, format!("{volume}: {path}"), &result).await;
    result
}

#[server]
pub async fn copy_into(stack: String, storage: String, from: String) -> Result<u64, DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let volume = state.stacks.volume_of(&stack, &storage)?;
    let allowed = backend::volumes::others(&state.docker, &volume).await?;
    if !allowed.contains(&from) {
        return Err(DockerError::NotFound);
    }
    let result = backend::volumes::copy_in(&state.docker, &from, &volume).await;
    let shown: Result<(), DockerError> = result.as_ref().map(|_| ()).map_err(Clone::clone);
    let count = result.as_ref().map(|n| format!(" ({n} files)")).unwrap_or_default();
    audit("volume.copy", admin.email, format!("{from} → {volume}{count}"), &shown).await;
    result
}

#[cfg(feature = "ssr")]
async fn audit(action: &str, actor: String, target: String, result: &Result<(), DockerError>) {
    super::audit::record(
        action,
        Some(actor),
        Some(target),
        result.as_ref().err().map(|e| e.to_string()),
        result.is_ok(),
    )
    .await;
}
