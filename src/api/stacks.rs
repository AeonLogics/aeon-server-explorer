use leptos::prelude::*;
use shared::errors::docker::DockerError;
use shared::stack::{EnvChange, StackSpec, StackSummary, StackView};

#[cfg(feature = "ssr")]
use super::docker::require_admin;
#[cfg(feature = "ssr")]
use crate::state::AppState;

#[server(endpoint = "poll/stacks")]
pub async fn get_stacks() -> Result<Vec<StackSummary>, DockerError> {
    require_admin()?;
    let state = AppState::get()?;
    state.stacks.list(&state.docker).await
}

#[server(endpoint = "poll/stack")]
pub async fn get_stack(name: String) -> Result<StackView, DockerError> {
    require_admin()?;
    let state = AppState::get()?;
    state.stacks.view(&state.docker, &name).await
}

#[server(endpoint = "poll/managed")]
pub async fn is_managed(name: String) -> Result<bool, DockerError> {
    require_admin()?;
    Ok(AppState::get()?.stacks.is_managed(&name))
}

#[server]
pub async fn get_images() -> Result<Vec<String>, DockerError> {
    require_admin()?;
    let state = AppState::get()?;
    let mut images: Vec<String> = backend::stacks::images(&state.docker).await?.into_iter().collect();
    images.sort();
    Ok(images)
}

#[server]
pub async fn create_stack(name: String) -> Result<(), DockerError> {
    let admin = require_admin()?;
    let name = name.trim().to_lowercase();
    let state = AppState::get()?;
    let result = state.stacks.create(&state.docker, &name).await;
    audit("stack.create", admin.email, &name, &result).await;
    result
}

// JSON, because form encoding drops empty lists (no services, no env changes).
#[server(input = leptos::server_fn::codec::Json)]
pub async fn save_stack(spec: StackSpec, changes: Vec<EnvChange>) -> Result<(), DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let name = spec.name.clone();
    let changed: Vec<String> = changes.iter().map(|c| format!("{}.{}", c.service, c.name)).collect();
    let result = state.stacks.save(&state.docker, spec, changes).await;
    let target = if changed.is_empty() {
        name
    } else {
        format!("{name} (set {})", changed.join(", "))
    };
    audit("stack.save", admin.email, &target, &result).await;
    result
}

#[server]
pub async fn apply_stack(name: String) -> Result<(), DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let result = state.stacks.apply(&name).await;
    audit("stack.apply", admin.email, &name, &result).await;
    result
}

#[server]
pub async fn reveal_env(stack: String, service: String, name: String) -> Result<String, DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let result = state.stacks.reveal(&stack, &service, &name);
    let shown: Result<(), DockerError> = result.as_ref().map(|_| ()).map_err(Clone::clone);
    audit("env.reveal", admin.email, &format!("{stack}/{service}: {name}"), &shown).await;
    result
}

#[server]
pub async fn reveal_env_file(stack: String, service: String) -> Result<String, DockerError> {
    let admin = require_admin()?;
    let state = AppState::get()?;
    let result = state.stacks.reveal_all(&stack, &service);
    let shown: Result<(), DockerError> = result.as_ref().map(|_| ()).map_err(Clone::clone);
    audit("env.reveal_all", admin.email, &format!("{stack}/{service}"), &shown).await;
    result
}

#[cfg(feature = "ssr")]
async fn audit(action: &str, actor: String, target: &str, result: &Result<(), DockerError>) {
    super::audit::record(
        action,
        Some(actor),
        Some(target.to_string()),
        result.as_ref().err().map(|e| e.to_string()),
        result.is_ok(),
    )
    .await;
}
