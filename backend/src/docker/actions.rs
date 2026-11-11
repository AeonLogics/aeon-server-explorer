use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::process::Stdio;
use std::sync::Mutex;
use std::time::Duration;

use bollard::Docker;
use bollard::errors::Error;
use bollard::query_parameters::{
    InspectContainerOptions, RestartContainerOptionsBuilder, StartContainerOptions,
    StopContainerOptionsBuilder,
};
use shared::docker::Action;
use shared::errors::docker::DockerError;

use super::{LABEL_CONFIG_FILES, LABEL_PROJECT, LABEL_SELF, LABEL_SERVICE, LABEL_WORKING_DIR, map_err};

// It can't stop or replace the container it runs in: the UI would vanish mid-action.
const SELF_STOP: &str = "This is the explorer itself. Stopping it would switch off this page; stop it over SSH instead.";
const SELF_REDEPLOY: &str = "This is the explorer itself. Update it from your PC (docker build, then compose up on the server).";
const PULL_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const UP_TIMEOUT: Duration = Duration::from_secs(5 * 60);

// What the action works on, read from Docker itself. The browser only ever sends a name.
#[derive(Debug, Clone)]
pub struct Target {
    pub container: String,
    pub project: Option<String>,
    pub service: Option<String>,
    pub working_dir: Option<String>,
    pub config_files: Vec<String>,
    pub is_self: bool,
}

impl Target {
    pub fn from_labels(container: String, labels: &HashMap<String, String>) -> Self {
        Self {
            container,
            project: labels.get(LABEL_PROJECT).cloned(),
            service: labels.get(LABEL_SERVICE).cloned(),
            working_dir: labels.get(LABEL_WORKING_DIR).cloned(),
            config_files: labels
                .get(LABEL_CONFIG_FILES)
                .map(|f| {
                    f.split(',')
                        .map(str::trim)
                        .filter(|f| !f.is_empty())
                        .map(str::to_string)
                        .collect()
                })
                .unwrap_or_default(),
            is_self: labels.get(LABEL_SELF).is_some_and(|v| v == "true"),
        }
    }

    pub fn label(&self) -> String {
        match (&self.project, &self.service) {
            (Some(p), Some(s)) => format!("{p}/{s} ({})", self.container),
            _ => self.container.clone(),
        }
    }

    // Why redeploy can't run here, if it can't.
    pub fn redeploy_blocker(&self) -> Option<String> {
        if self.is_self {
            return Some(SELF_REDEPLOY.into());
        }
        if self.project.is_none() || self.service.is_none() || self.working_dir.is_none() {
            return Some("Not started by docker compose, so there's no file to redeploy from.".into());
        }
        if self.config_files.is_empty() {
            return Some("Docker doesn't know which compose file created it.".into());
        }
        self.config_files
            .iter()
            .find(|f| !Path::new(f).exists())
            .map(|f| format!("Its compose file isn't reachable from here: {f}"))
    }
}

pub async fn target(docker: &Docker, container: &str) -> Result<Target, DockerError> {
    let inspect = docker
        .inspect_container(container, None::<InspectContainerOptions>)
        .await
        .map_err(map_err)?;
    let labels = inspect.config.and_then(|c| c.labels).unwrap_or_default();
    let name = inspect
        .name
        .map(|n| n.trim_start_matches('/').to_string())
        .ok_or(DockerError::NotFound)?;
    Ok(Target::from_labels(name, &labels))
}

static BUSY: Mutex<Option<HashSet<String>>> = Mutex::new(None);

// One action per compose project at a time, so two clicks can't race each other.
pub(crate) struct Busy(String);

impl Busy {
    pub(crate) fn claim(key: String) -> Result<Self, DockerError> {
        let mut guard = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        let set = guard.get_or_insert_with(HashSet::new);
        if !set.insert(key.clone()) {
            return Err(DockerError::Failed(
                "Another action is still running on this project. Try again in a moment.".into(),
            ));
        }
        Ok(Self(key))
    }
}

impl Drop for Busy {
    fn drop(&mut self) {
        let mut guard = BUSY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(set) = guard.as_mut() {
            set.remove(&self.0);
        }
    }
}

pub async fn run(docker: &Docker, target: &Target, action: Action) -> Result<(), DockerError> {
    if target.is_self && action == Action::Stop {
        return Err(DockerError::Failed(SELF_STOP.into()));
    }
    let _busy = Busy::claim(target.project.clone().unwrap_or_else(|| target.container.clone()))?;
    let name = target.container.as_str();

    match action {
        Action::Start => engine(docker.start_container(name, None::<StartContainerOptions>).await),
        Action::Stop => {
            let options = StopContainerOptionsBuilder::new().t(10).build();
            engine(docker.stop_container(name, Some(options)).await)
        }
        Action::Restart => {
            let options = RestartContainerOptionsBuilder::new().t(10).build();
            engine(docker.restart_container(name, Some(options)).await)
        }
        Action::Redeploy => redeploy(target).await,
    }
}

fn engine(result: Result<(), Error>) -> Result<(), DockerError> {
    match result {
        Ok(()) => Ok(()),
        // 304: it was already in the state we asked for.
        Err(Error::DockerResponseServerError { status_code: 304, .. }) => Ok(()),
        Err(Error::DockerResponseServerError { status_code: 409, message }) => {
            Err(DockerError::Failed(message))
        }
        Err(e) => Err(map_err(e)),
    }
}

async fn redeploy(target: &Target) -> Result<(), DockerError> {
    if let Some(reason) = target.redeploy_blocker() {
        return Err(DockerError::Failed(reason));
    }
    let (Some(project), Some(service), Some(dir)) =
        (&target.project, &target.service, &target.working_dir)
    else {
        return Err(DockerError::Failed("Missing compose labels.".into()));
    };

    let mut base: Vec<&str> = vec!["compose", "-p", project, "--project-directory", dir];
    for file in &target.config_files {
        base.extend(["-f", file.as_str()]);
    }

    let mut pull = base.clone();
    pull.extend(["pull", "--quiet", "--ignore-buildable", service]);
    cli(&pull, PULL_TIMEOUT).await?;

    let mut up = base;
    up.extend(["up", "-d", "--no-deps", service]);
    cli(&up, UP_TIMEOUT).await
}

// Runs the docker CLI with explicit arguments, never through a shell.
pub(crate) async fn cli(args: &[&str], limit: Duration) -> Result<(), DockerError> {
    let mut command = tokio::process::Command::new("docker");
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let child = command.output();

    let output = match tokio::time::timeout(limit, child).await {
        Err(_) => return Err(DockerError::Failed("docker compose took too long and was stopped.".into())),
        Ok(Err(e)) if e.kind() == std::io::ErrorKind::NotFound => {
            return Err(DockerError::Failed("The docker CLI isn't installed where this app runs.".into()));
        }
        Ok(Err(e)) => return Err(DockerError::Failed(format!("Couldn't run docker: {e}"))),
        Ok(Ok(output)) => output,
    };

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let tail: Vec<&str> = stderr.lines().filter(|l| !l.trim().is_empty()).collect();
    // Docker puts the real reason in the last lines; keep enough of them to read it whole.
    let tail = tail[tail.len().saturating_sub(15)..].join("
");
    let mut message = format!("docker compose failed:
{tail}");
    if message.len() > 4000 {
        let mut cut = 4000;
        while !message.is_char_boundary(cut) {
            cut -= 1;
        }
        message.truncate(cut);
        message.push('…');
    }
    Err(DockerError::Failed(message))
}
