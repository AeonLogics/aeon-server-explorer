mod actions;
mod detail;
mod streams;
mod summary;

pub use actions::{run, target, Target};
pub(crate) use actions::{cli, Busy};
pub(crate) use summary::list as list_containers;
pub(crate) use summary::card as container_card;
pub use bollard::Docker;
pub use detail::service;
pub use streams::{logs, stats};
pub use summary::{overview, project};

use bollard::errors::Error;
use shared::errors::{InternalServerError, docker::DockerError};

pub const LABEL_PROJECT: &str = "com.docker.compose.project";
pub const LABEL_SERVICE: &str = "com.docker.compose.service";
pub const LABEL_NUMBER: &str = "com.docker.compose.container-number";
pub const LABEL_WORKING_DIR: &str = "com.docker.compose.project.working_dir";
pub const LABEL_CONFIG_FILES: &str = "com.docker.compose.project.config_files";
pub const LABEL_SELF: &str = "dev.aeonlogics.explorer.self";

// Named pipe on Windows (Docker Desktop), /var/run/docker.sock on Linux. The connection is
// lazy, so this succeeds even while Docker is still starting.
pub fn connect() -> Result<Docker, Error> {
    Docker::connect_with_local_defaults()
}

pub(crate) fn map_err(err: Error) -> DockerError {
    match err {
        Error::DockerResponseServerError { status_code: 404, .. } => DockerError::NotFound,
        Error::DockerResponseServerError { status_code, message } => {
            InternalServerError::new(format!("docker answered {status_code}: {message}")).into()
        }
        other => {
            eprintln!("[docker] unreachable: {other}");
            DockerError::Unavailable
        }
    }
}

pub fn local_time(rfc3339: &str) -> String {
    chrono::DateTime::parse_from_rfc3339(rfc3339)
        .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M:%S").to_string())
        .unwrap_or_else(|_| rfc3339.to_string())
}

pub(crate) fn humanize_secs(secs: i64) -> String {
    let secs = secs.max(0);
    let (d, h, m) = (secs / 86_400, secs % 86_400 / 3_600, secs % 3_600 / 60);
    match (d, h, m) {
        (0, 0, 0) => format!("{secs}s"),
        (0, 0, m) => format!("{m}m"),
        (0, h, m) => format!("{h}h {m}m"),
        (d, h, _) => format!("{d}d {h}h"),
    }
}
