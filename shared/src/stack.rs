use serde::{Deserialize, Serialize};

use crate::docker::Health;

pub const MAX_SERVICES: usize = 30;

// One env value to set: which service, which variable, the new value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EnvChange {
    pub service: String,
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackSpec {
    pub name: String,
    pub services: Vec<ServiceSpec>,
    // Volumes that belong to the stack, so several services can mount the same one.
    #[serde(default)]
    pub storages: Vec<StorageSpec>,
    // Extra networks for keeping services apart; every stack also has `default`.
    #[serde(default)]
    pub networks: Vec<NetworkSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StorageSpec {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkSpec {
    pub name: String,
}

// A stack storage mounted into one service.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountSpec {
    pub storage: String,
    pub path: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StartCondition {
    Started,
    Healthy,
    Finished,
}

impl StartCondition {
    pub const ALL: [StartCondition; 3] = [Self::Started, Self::Healthy, Self::Finished];

    pub fn compose(self) -> &'static str {
        match self {
            Self::Started => "service_started",
            Self::Healthy => "service_healthy",
            Self::Finished => "service_completed_successfully",
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Healthy => "healthy",
            Self::Finished => "finished",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Started => "is running",
            Self::Healthy => "is healthy",
            Self::Finished => "finished successfully",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DependsSpec {
    pub service: String,
    pub condition: StartCondition,
}

pub const DEFAULT_NETWORK: &str = "default";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSpec {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub restart: Restart,
    #[serde(default)]
    pub ports: Vec<PortSpec>,
    #[serde(default)]
    pub volumes: Vec<VolumeSpec>,
    // Names only; values live in env/<service>.env on the server.
    #[serde(default)]
    pub env: Vec<String>,
    // Overrides the image's default command; empty keeps the image's own.
    #[serde(default)]
    pub command: Vec<String>,
    // The user the program runs as, like "0" or "1000:1000"; None keeps the image's own.
    #[serde(default)]
    pub user: Option<String>,
    #[serde(default)]
    pub mounts: Vec<MountSpec>,
    // Empty means only the default network.
    #[serde(default)]
    pub networks: Vec<String>,
    #[serde(default)]
    pub depends_on: Vec<DependsSpec>,
    // A helper that runs to the end and stays stopped (restart: "no").
    #[serde(default)]
    pub run_once: bool,
}

impl ServiceSpec {
    pub fn new(name: String, image: String) -> Self {
        Self {
            name,
            image,
            restart: Restart::default(),
            ports: Vec::new(),
            volumes: Vec::new(),
            env: Vec::new(),
            command: Vec::new(),
            user: None,
            mounts: Vec::new(),
            networks: Vec::new(),
            depends_on: Vec::new(),
            run_once: false,
        }
    }

    pub fn on_networks(&self) -> Vec<String> {
        if self.networks.is_empty() {
            vec![DEFAULT_NETWORK.to_string()]
        } else {
            self.networks.clone()
        }
    }
}

// The compose key of a service's own volume; a shared storage uses its plain name.
pub fn own_volume_key(service: &str, volume: &str) -> String {
    format!("{service}-{volume}")
}

pub const MAX_ARGS: usize = 50;
pub const MAX_ARG: usize = 500;

// Splits a command line like a shell would for plain words and "quoted" or 'quoted' parts.
pub fn split_command(line: &str) -> Result<Vec<String>, String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut started = false;
    for c in line.chars() {
        match (quote, c) {
            (Some(q), c) if c == q => quote = None,
            (Some(_), c) => current.push(c),
            (None, '"' | '\'') => {
                quote = Some(c);
                started = true;
            }
            (None, c) if c.is_whitespace() => {
                if started {
                    args.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            (None, c) => {
                current.push(c);
                started = true;
            }
        }
    }
    if quote.is_some() {
        return Err("A quote in the command isn't closed.".into());
    }
    if started {
        args.push(current);
    }
    Ok(args)
}

// The reverse, for showing a saved command in the text box.
pub fn join_command(args: &[String]) -> String {
    args.iter()
        .map(|a| {
            if a.is_empty() || a.chars().any(|c| c.is_whitespace() || c == '"' || c == '\'') {
                if a.contains('"') { format!("'{a}'") } else { format!("\"{a}\"") }
            } else {
                a.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Restart {
    No,
    Always,
    #[default]
    UnlessStopped,
    OnFailure,
}

impl Restart {
    pub const ALL: [Restart; 4] = [Self::UnlessStopped, Self::Always, Self::OnFailure, Self::No];

    pub fn compose(self) -> &'static str {
        match self {
            Self::No => "no",
            Self::Always => "always",
            Self::UnlessStopped => "unless-stopped",
            Self::OnFailure => "on-failure",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::No => "Never",
            Self::Always => "Always",
            Self::UnlessStopped => "Unless stopped",
            Self::OnFailure => "On failure",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortSpec {
    pub container: u16,
    // Some(port) publishes it on 127.0.0.1:port; None keeps it inside the stack.
    pub host: Option<u16>,
    // A debug port: published on 0.0.0.0, so every network the server is on can reach it.
    #[serde(default)]
    pub public: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeSpec {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StackState {
    NotDeployed,
    Deployed,
    Pending,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackSummary {
    pub name: String,
    pub services: usize,
    pub state: StackState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StackView {
    pub spec: StackSpec,
    pub state: StackState,
    pub folder: String,
    pub compose: String,
    pub status: Vec<ServiceStatus>,
    pub last_apply: Option<ApplyRecord>,
}

// The outcome of the most recent deploy, kept so a failure's reason survives a refresh.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyRecord {
    pub at: String,
    pub ok: bool,
    pub output: String,
}

// Reads a .env-style text: KEY=value per line, with # comments, blank lines,
// an optional `export ` prefix and optional quotes around the value.
pub fn parse_env_file(text: &str) -> Result<Vec<(String, String)>, String> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (index, raw) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").unwrap_or(line);
        let Some((name, value)) = line.split_once('=') else {
            return Err(format!("Line {line_no}: expected NAME=value."));
        };
        let name = name.trim();
        if !valid_env_name(name) {
            return Err(format!("Line {line_no}: \"{name}\" isn't a valid variable name."));
        }
        if out.iter().any(|(n, _)| n == name) {
            return Err(format!("Line {line_no}: {name} is listed twice."));
        }
        let value = value.trim();
        let value = ['"', '\'']
            .iter()
            .find_map(|q| value.strip_prefix(*q).and_then(|v| v.strip_suffix(*q)))
            .unwrap_or(value);
        out.push((name.to_string(), value.to_string()));
    }
    Ok(out)
}

// What Docker says about one running (or crashed) container of a managed stack.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceStatus {
    pub service: String,
    pub container: String,
    pub health: Health,
    pub health_label: String,
    pub status_text: String,
    pub running: bool,
}

// A compose project / service / volume name: lowercase letters, digits, - and _.
pub fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && name.len() <= 40
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
}

pub fn valid_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_alphabetic() || c == '_')
        && name.len() <= 64
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

// Only characters that can appear in an image reference, so it's always safe in YAML.
pub fn valid_image(image: &str) -> bool {
    !image.is_empty()
        && image.len() <= 200
        && image
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/' | ':' | '@'))
        && image.chars().next().is_some_and(|c| c.is_ascii_alphanumeric())
}

pub fn valid_container_path(path: &str) -> bool {
    path.starts_with('/')
        && path != "/"
        && path.len() <= 200
        && !path.split('/').any(|part| part == ".." || part == ".")
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '.'))
}

// A user name or ID, optionally with a group: "0", "postgres", "1000:1000".
pub fn valid_user(user: &str) -> bool {
    let part = |p: &str| {
        !p.is_empty() && p.len() <= 32 && p.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    };
    match user.split_once(':') {
        Some((u, g)) => part(u) && part(g),
        None => part(user),
    }
}

pub fn valid_host_port(port: u16) -> bool {
    port >= 1024
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecProblem(pub String);

// Everything the form can get wrong, checked the same way in the browser and on the server.
pub fn check_spec(spec: &StackSpec) -> Result<(), SpecProblem> {
    let fail = |m: String| Err(SpecProblem(m));
    if !valid_name(&spec.name) {
        return fail("Stack names use lowercase letters, digits, - and _ (up to 40).".into());
    }
    if spec.services.len() > MAX_SERVICES {
        return fail(format!("A stack can have up to {MAX_SERVICES} services."));
    }

    let mut storage_names = std::collections::HashSet::new();
    for st in &spec.storages {
        if !valid_name(&st.name) {
            return fail(format!("\"{}\" isn't a valid storage name.", st.name));
        }
        if !storage_names.insert(st.name.as_str()) {
            return fail(format!("Two storages are called {}.", st.name));
        }
    }
    let mut network_names = std::collections::HashSet::new();
    for n in &spec.networks {
        if !valid_name(&n.name) || n.name == DEFAULT_NETWORK {
            return fail(format!("\"{}\" can't be a network name.", n.name));
        }
        if !network_names.insert(n.name.as_str()) {
            return fail(format!("Two networks are called {}.", n.name));
        }
    }
    let all_services: std::collections::HashSet<&str> = spec.services.iter().map(|s| s.name.as_str()).collect();
    for s in &spec.services {
        for v in &s.volumes {
            let key = own_volume_key(&s.name, &v.name);
            if storage_names.contains(key.as_str()) {
                return fail(format!(
                    "The storage {key} has the same name as {}'s own storage {}. Rename one of them.",
                    s.name, v.name
                ));
            }
        }
    }

    let mut service_names = std::collections::HashSet::new();
    let mut host_ports = std::collections::HashSet::new();
    for s in &spec.services {
        if !valid_name(&s.name) {
            return fail(format!("\"{}\" isn't a valid service name.", s.name));
        }
        if !service_names.insert(s.name.as_str()) {
            return fail(format!("Two services are called {}.", s.name));
        }
        if !valid_image(&s.image) {
            return fail(format!("{}: pick an image.", s.name));
        }

        let mut container_ports = std::collections::HashSet::new();
        for p in &s.ports {
            if p.container == 0 {
                return fail(format!("{}: a container port can't be 0.", s.name));
            }
            if !container_ports.insert(p.container) {
                return fail(format!("{}: port {} is listed twice.", s.name, p.container));
            }
            if let Some(host) = p.host {
                if !valid_host_port(host) {
                    return fail(format!("{}: server ports start at 1024.", s.name));
                }
                if !host_ports.insert(host) {
                    return fail(format!("Server port {host} is used twice in this stack."));
                }
            }
        }

        let mut volume_names = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for v in &s.volumes {
            if !valid_name(&v.name) {
                return fail(format!("{}: \"{}\" isn't a valid storage name.", s.name, v.name));
            }
            if !volume_names.insert(v.name.as_str()) {
                return fail(format!("{}: storage {} is listed twice.", s.name, v.name));
            }
            if !valid_container_path(&v.path) {
                return fail(format!("{}: \"{}\" isn't a valid absolute path.", s.name, v.path));
            }
            if !paths.insert(v.path.as_str()) {
                return fail(format!("{}: {} is mounted twice.", s.name, v.path));
            }
        }
        let mut mounted = std::collections::HashSet::new();
        for m in &s.mounts {
            if !storage_names.contains(m.storage.as_str()) {
                return fail(format!("{}: there's no storage called {} in this stack.", s.name, m.storage));
            }
            if !mounted.insert(m.storage.as_str()) {
                return fail(format!("{}: storage {} is mounted twice.", s.name, m.storage));
            }
            if !valid_container_path(&m.path) {
                return fail(format!("{}: \"{}\" isn't a valid absolute path.", s.name, m.path));
            }
            if !paths.insert(m.path.as_str()) {
                return fail(format!("{}: {} is mounted twice.", s.name, m.path));
            }
        }

        let mut joined = std::collections::HashSet::new();
        for n in &s.networks {
            if n != DEFAULT_NETWORK && !network_names.contains(n.as_str()) {
                return fail(format!("{}: there's no network called {n} in this stack.", s.name));
            }
            if !joined.insert(n.as_str()) {
                return fail(format!("{}: network {n} is listed twice.", s.name));
            }
        }

        let mut after = std::collections::HashSet::new();
        for d in &s.depends_on {
            if d.service == s.name {
                return fail(format!("{} can't wait for itself.", s.name));
            }
            if !all_services.contains(d.service.as_str()) {
                return fail(format!("{}: there's no service called {} to wait for.", s.name, d.service));
            }
            if !after.insert(d.service.as_str()) {
                return fail(format!("{}: {} is listed twice in Start after.", s.name, d.service));
            }
        }
        if s.ports.iter().any(|p| p.public && p.host.is_none()) {
            return fail(format!("{}: a debug port needs a server port.", s.name));
        }

        if s.user.as_deref().is_some_and(|u| !valid_user(u)) {
            return fail(format!("{}: run as takes a user name or ID, like 0 or 1000:1000.", s.name));
        }

        if s.command.len() > MAX_ARGS {
            return fail(format!("{}: the command can have up to {MAX_ARGS} parts.", s.name));
        }
        if s.command.iter().any(|a| a.len() > MAX_ARG || a.chars().any(char::is_control)) {
            return fail(format!("{}: command parts must be one line, up to {MAX_ARG} characters.", s.name));
        }

        let mut env_names = std::collections::HashSet::new();
        for e in &s.env {
            if !valid_env_name(e) {
                return fail(format!("{}: \"{e}\" isn't a valid variable name.", s.name));
            }
            if !env_names.insert(e.as_str()) {
                return fail(format!("{}: {e} is listed twice.", s.name));
            }
        }
    }
    if let Some(cycle) = start_cycle(spec) {
        return fail(format!("{cycle} wait for each other, so none of them could start."));
    }
    Ok(())
}

// Finds a loop in Start after, like "web waits for db, db waits for web".
fn start_cycle(spec: &StackSpec) -> Option<String> {
    fn visit<'a>(
        name: &'a str,
        spec: &'a StackSpec,
        path: &mut Vec<&'a str>,
        done: &mut std::collections::HashSet<&'a str>,
    ) -> Option<String> {
        if let Some(start) = path.iter().position(|p| *p == name) {
            let names: Vec<&str> = path[start..].to_vec();
            return Some(names.join(" and "));
        }
        if done.contains(name) {
            return None;
        }
        path.push(name);
        if let Some(service) = spec.services.iter().find(|s| s.name == name) {
            for d in &service.depends_on {
                if let Some(found) = visit(&d.service, spec, path, done) {
                    return Some(found);
                }
            }
        }
        path.pop();
        done.insert(name);
        None
    }

    let mut done = std::collections::HashSet::new();
    spec.services
        .iter()
        .find_map(|s| visit(&s.name, spec, &mut Vec::new(), &mut done))
}
