use leptos::prelude::*;
use shared::docker::{Overview, ProjectDetail, ServiceDetail};
use shared::errors::docker::DockerError;

#[cfg(feature = "ssr")]
use crate::state::AppState;

#[cfg(feature = "ssr")]
fn require_admin() -> Result<(), DockerError> {
    match super::current_admin_from_context() {
        None => Err(DockerError::Unauthorized),
        Some(admin) if admin.must_change_password => Err(DockerError::PasswordChangeRequired),
        Some(_) => Ok(()),
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
