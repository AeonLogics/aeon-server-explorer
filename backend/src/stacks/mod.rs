mod compose;
mod files;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::time::Duration;

use bollard::Docker;
use bollard::query_parameters::ListImagesOptionsBuilder;
use serde::{Deserialize, Serialize};
use shared::errors::docker::DockerError;
pub use shared::stack::EnvChange;
use shared::stack::{ApplyRecord, ServiceStatus, StackSpec, StackState, StackSummary, StackView, check_spec, valid_name};

use crate::docker::{Busy, LABEL_PROJECT, LABEL_SERVICE, cli, container_card, list_containers, map_err};

pub use compose::LABEL_MANAGED;

const META: &str = "explorer.json";
const COMPOSE: &str = "compose.yml";
const UP_TIMEOUT: Duration = Duration::from_secs(5 * 60);
const CHECK_TIMEOUT: Duration = Duration::from_secs(60);
const MAX_VALUE: usize = 4096;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Meta {
    version: u32,
    spec: StackSpec,
    applied: Option<String>,
    #[serde(default)]
    last_apply: Option<ApplyRecord>,
}

#[derive(Debug, Clone)]
pub struct Stacks {
    root: PathBuf,
}

impl Stacks {
    // STACKS_DIR points at the folder holding every stack; locally it defaults to data/stacks.
    pub fn from_env() -> Self {
        let root = std::env::var("STACKS_DIR")
            .ok()
            .filter(|v| !v.trim().is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("data").join("stacks"));
        Self { root }
    }

    pub fn root(&self) -> &std::path::Path {
        &self.root
    }

    fn dir(&self, name: &str) -> Result<PathBuf, DockerError> {
        if !valid_name(name) {
            return Err(DockerError::NotFound);
        }
        Ok(self.root.join(name))
    }

    fn read_meta(&self, name: &str) -> Result<Meta, DockerError> {
        let path = self.dir(name)?.join(META);
        let text = std::fs::read_to_string(&path).map_err(|_| DockerError::NotFound)?;
        serde_json::from_str(&text).map_err(|e| failed(format!("{META} for {name} is unreadable: {e}")))
    }

    fn write_meta(&self, meta: &Meta) -> Result<(), DockerError> {
        let dir = self.dir(&meta.spec.name)?;
        let json = serde_json::to_string_pretty(meta).map_err(|e| failed(e.to_string()))?;
        files::write_atomic(&dir.join(META), json.as_bytes(), false)
    }

    pub fn is_managed(&self, name: &str) -> bool {
        self.dir(name).is_ok_and(|d| d.join(META).is_file())
    }

    pub async fn list(&self, docker: &Docker) -> Result<Vec<StackSummary>, DockerError> {
        let deployed = deployed_projects(docker).await?;
        let Ok(entries) = std::fs::read_dir(&self.root) else {
            return Ok(Vec::new());
        };
        let mut out = Vec::new();
        for entry in entries.flatten() {
            let Some(name) = entry.file_name().to_str().map(str::to_string) else { continue };
            let Ok(meta) = self.read_meta(&name) else { continue };
            let state = self.state(&meta, deployed.contains(&name));
            out.push(StackSummary { name, services: meta.spec.services.len(), state });
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }

    pub async fn view(&self, docker: &Docker, name: &str) -> Result<StackView, DockerError> {
        let meta = self.read_meta(name)?;
        let containers = list_containers(docker, None).await?;
        let ours = |c: &&bollard::models::ContainerSummary| {
            c.labels.as_ref().and_then(|l| l.get(LABEL_PROJECT)).map(String::as_str) == Some(name)
        };
        let deployed = containers.iter().any(|c| ours(&c));
        let mut status: Vec<ServiceStatus> = containers
            .iter()
            .filter(ours)
            .filter_map(|c| {
                let service = c.labels.as_ref()?.get(LABEL_SERVICE)?.clone();
                let card = container_card(c);
                let running = card.is_settled();
                Some(ServiceStatus {
                    service,
                    container: card.container_name,
                    health: card.health,
                    health_label: card.health_label,
                    status_text: card.status_text,
                    running,
                })
            })
            .collect();
        status.sort_by(|a, b| a.service.cmp(&b.service));

        let dir = self.dir(name)?;
        let compose = std::fs::read_to_string(dir.join(COMPOSE)).unwrap_or_default();
        Ok(StackView {
            state: self.state(&meta, deployed),
            spec: meta.spec,
            folder: dir.display().to_string(),
            compose,
            status,
            last_apply: meta.last_apply,
        })
    }

    pub async fn create(&self, docker: &Docker, name: &str) -> Result<(), DockerError> {
        let dir = self.dir(name).map_err(|_| {
            failed("Stack names use lowercase letters, digits, - and _ (up to 40).".into())
        })?;
        if dir.exists() {
            return Err(failed(format!("{} already exists. Pick another name.", dir.display())));
        }
        if deployed_projects(docker).await?.contains(name) {
            return Err(failed(format!("A compose project called {name} is already running.")));
        }

        std::fs::create_dir_all(&dir).map_err(|e| failed(format!("Couldn't create {}: {e}", dir.display())))?;
        let meta = Meta {
            version: 1,
            spec: StackSpec { name: name.to_string(), services: Vec::new(), storages: Vec::new(), networks: Vec::new() },
            applied: None,
            last_apply: None,
        };
        self.write_meta(&meta)?;
        self.write_outputs(&meta.spec, &[])
    }

    pub async fn save(
        &self,
        docker: &Docker,
        spec: StackSpec,
        changes: Vec<EnvChange>,
    ) -> Result<(), DockerError> {
        check_spec(&spec).map_err(|p| failed(p.0))?;
        let mut meta = self.read_meta(&spec.name)?;
        let _busy = Busy::claim(spec.name.clone())?;

        for change in &changes {
            if change.value.len() > MAX_VALUE || change.value.contains(['\n', '\r', '\0']) {
                return Err(failed(format!("{}: values must be one line (up to 4 KB).", change.name)));
            }
        }

        let installed = images(docker).await?;
        if let Some(missing) = spec.services.iter().find(|s| !installed.contains(&s.image)) {
            return Err(failed(format!(
                "{}: the image {} isn't on this server.",
                missing.name, missing.image
            )));
        }
        self.check_ports(docker, &spec).await?;

        self.write_outputs(&spec, &changes)?;
        meta.spec = spec;
        self.write_meta(&meta)
    }

    pub async fn apply(&self, name: &str) -> Result<(), DockerError> {
        let mut meta = self.read_meta(name)?;
        if meta.spec.services.is_empty() {
            return Err(failed("Add a service before deploying.".into()));
        }
        let _busy = Busy::claim(name.to_string())?;
        let dir = self.dir(name)?;
        let dir_text = dir.display().to_string();
        let file = dir.join(COMPOSE).display().to_string();
        let base = ["compose", "-p", name, "--project-directory", &dir_text, "-f", &file];

        let result = async {
            let mut check = base.to_vec();
            check.extend(["config", "-q"]);
            cli(&check, CHECK_TIMEOUT).await?;

            let mut up = base.to_vec();
            up.extend(["up", "-d", "--remove-orphans", "--pull", "never"]);
            cli(&up, UP_TIMEOUT).await
        }
        .await;

        // Kept in explorer.json, so the reason for a failure is still there after a refresh.
        let at = chrono::Local::now().format("%Y-%m-%d %H:%M").to_string();
        meta.last_apply = Some(match &result {
            Ok(()) => ApplyRecord { at, ok: true, output: String::new() },
            Err(e) => ApplyRecord { at, ok: false, output: e.to_string() },
        });
        if result.is_ok() {
            meta.applied = Some(files::fingerprint(&dir, &meta.spec));
        }
        self.write_meta(&meta)?;
        result
    }

    // The whole env file of one service, limited to the names the stack knows.
    pub fn reveal_all(&self, stack: &str, service: &str) -> Result<String, DockerError> {
        let meta = self.read_meta(stack)?;
        let spec = meta.spec.services.iter().find(|s| s.name == service).ok_or(DockerError::NotFound)?;
        let dir = self.dir(stack)?;
        let mut text = String::new();
        for name in &spec.env {
            let value = files::read_value(&dir, service, name).unwrap_or_default();
            text.push_str(&format!("{name}={value}
"));
        }
        Ok(text)
    }

    pub fn reveal(&self, stack: &str, service: &str, name: &str) -> Result<String, DockerError> {
        let meta = self.read_meta(stack)?;
        let known = meta
            .spec
            .services
            .iter()
            .any(|s| s.name == service && s.env.iter().any(|e| e == name));
        if !known {
            return Err(DockerError::NotFound);
        }
        Ok(files::read_value(&self.dir(stack)?, service, name).unwrap_or_default())
    }

    fn state(&self, meta: &Meta, deployed: bool) -> StackState {
        if !deployed {
            return StackState::NotDeployed;
        }
        let Ok(dir) = self.dir(&meta.spec.name) else { return StackState::Pending };
        match &meta.applied {
            Some(hash) if *hash == files::fingerprint(&dir, &meta.spec) => StackState::Deployed,
            _ => StackState::Pending,
        }
    }

    fn write_outputs(&self, spec: &StackSpec, changes: &[EnvChange]) -> Result<(), DockerError> {
        let dir = self.dir(&spec.name)?;
        files::write_atomic(&dir.join(COMPOSE), compose::render(spec).as_bytes(), false)?;
        files::write_env_files(&dir, spec, changes)
    }

    // Host ports must not clash with any other container or another managed stack.
    async fn check_ports(&self, docker: &Docker, spec: &StackSpec) -> Result<(), DockerError> {
        let wanted: Vec<u16> = spec.services.iter().flat_map(|s| s.ports.iter().filter_map(|p| p.host)).collect();
        if wanted.is_empty() {
            return Ok(());
        }

        let mut taken: HashMap<u16, String> = HashMap::new();
        for c in list_containers(docker, None).await? {
            let project = c.labels.as_ref().and_then(|l| l.get(LABEL_PROJECT)).cloned();
            if project.as_deref() == Some(spec.name.as_str()) {
                continue;
            }
            let owner = project.unwrap_or_else(|| {
                c.names.as_ref().and_then(|n| n.first()).cloned().unwrap_or_default()
            });
            for p in c.ports.iter().flatten() {
                if let Some(port) = p.public_port {
                    taken.insert(port, owner.trim_start_matches('/').to_string());
                }
            }
        }
        if let Ok(entries) = std::fs::read_dir(&self.root) {
            for entry in entries.flatten() {
                let Some(other) = entry.file_name().to_str().map(str::to_string) else { continue };
                if other == spec.name {
                    continue;
                }
                let Ok(meta) = self.read_meta(&other) else { continue };
                for port in meta.spec.services.iter().flat_map(|s| s.ports.iter().filter_map(|p| p.host)) {
                    taken.entry(port).or_insert_with(|| other.clone());
                }
            }
        }

        match wanted.iter().find_map(|p| taken.get(p).map(|owner| (p, owner))) {
            Some((port, owner)) => Err(failed(format!("Port {port} is already used by {owner}."))),
            None => Ok(()),
        }
    }
}

pub async fn images(docker: &Docker) -> Result<HashSet<String>, DockerError> {
    let options = ListImagesOptionsBuilder::new().all(false).build();
    let list = docker.list_images(Some(options)).await.map_err(map_err)?;
    Ok(list
        .into_iter()
        .flat_map(|i| i.repo_tags)
        .filter(|t| !t.starts_with("<none>"))
        .collect())
}

async fn deployed_projects(docker: &Docker) -> Result<HashSet<String>, DockerError> {
    Ok(list_containers(docker, None)
        .await?
        .into_iter()
        .filter_map(|c| c.labels.and_then(|mut l| l.remove(LABEL_PROJECT)))
        .collect())
}

fn failed(message: String) -> DockerError {
    DockerError::Failed(message)
}
