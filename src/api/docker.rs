use leptos::prelude::*;
use shared::docker::{Action, Overview, ProjectDetail, ServiceDetail};
use shared::errors::docker::DockerError;

#[cfg(feature = "ssr")]
use crate::state::AppState;
#[cfg(feature = "ssr")]
use shared::admin::Admin;

#[cfg(feature = "ssr")]
pub(crate) fn require_admin() -> Result<Admin, DockerError> {
    match super::current_admin_from_context() {
        None => Err(DockerError::Unauthorized),
        Some(admin) if admin.must_change_password => Err(DockerError::PasswordChangeRequired),
        Some(admin) => Ok(admin),
    }
}

#[cfg(feature = "ssr")]
fn docker() -> Result<backend::docker::Docker, DockerError> {
    Ok(AppState::get()?.docker)
}

#[server(endpoint = "poll/overview")]
pub async fn get_overview() -> Result<Overview, DockerError> {
    require_admin()?;
    backend::docker::overview(&docker()?).await
}

#[server(endpoint = "poll/project")]
pub async fn get_project(slug: String) -> Result<ProjectDetail, DockerError> {
    require_admin()?;
    backend::docker::project(&docker()?, &slug).await
}

#[server(endpoint = "poll/service")]
pub async fn get_service(container: String) -> Result<ServiceDetail, DockerError> {
    require_admin()?;
    if !shared::docker::valid_container_ref(&container) {
        return Err(DockerError::NotFound);
    }
    backend::docker::service(&docker()?, &container).await
}

#[server]
pub async fn container_action(
    container: String,
    action: Action,
    confirm: Option<String>,
) -> Result<(), DockerError> {
    let admin = require_admin()?;
    if !shared::docker::valid_container_ref(&container) {
        return Err(DockerError::NotFound);
    }

    let docker = docker()?;
    let target = backend::docker::target(&docker, &container).await?;

    // Checked here too, not only in the dialog.
    if action.needs_typed_name() && confirm.as_deref().map(str::trim) != Some(target.container.as_str()) {
        return Err(DockerError::Failed(format!(
            "Type {} exactly to confirm.",
            target.container
        )));
    }

    let result = backend::docker::run(&docker, &target, action).await;

    super::audit::record(
        action.audit_name(),
        Some(admin.email),
        Some(target.label()),
        result.as_ref().err().map(|e| e.to_string()),
        result.is_ok(),
    )
    .await;

    result
}
