use serde::{Deserialize, Serialize};

pub const STANDALONE: &str = "(standalone)";
pub const STANDALONE_SLUG: &str = "_standalone";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Health {
    Ok,
    Warn,
    Down,
}

impl Health {
    pub fn class(self) -> &'static str {
        match self {
            Self::Ok => "is-ok",
            Self::Warn => "is-warn",
            Self::Down => "is-down",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Overview {
    pub engine_version: Option<String>,
    pub projects: Vec<ProjectSummary>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectSummary {
    pub name: String,
    pub slug: String,
    pub standalone: bool,
    pub running: u32,
    pub total: u32,
    pub unhealthy: u32,
    pub exposed_ports: u32,
    pub health: Health,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProjectDetail {
    pub summary: ProjectSummary,
    pub working_dir: Option<String>,
    pub services: Vec<ServiceCard>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceCard {
    pub id: String,
    pub name: String,
    pub container_name: String,
    pub image: String,
    pub kind: ServiceKind,
    pub state: String,
    pub status_text: String,
    pub health: Health,
    pub health_label: String,
    pub ports: Vec<PortInfo>,
    pub networks: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ServiceKind {
    Database,
    Proxy,
    App,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortInfo {
    pub host_ip: Option<String>,
    pub host_port: Option<u16>,
    pub container_port: u16,
    pub protocol: String,
    pub exposed: bool,
}

impl PortInfo {
    pub fn label(&self) -> String {
        match self.host_port {
            Some(host) => {
                let ip = self.host_ip.as_deref().unwrap_or("0.0.0.0");
                let ip = if ip.contains(':') { format!("[{ip}]") } else { ip.to_string() };
                format!("{ip}:{host} → {}/{}", self.container_port, self.protocol)
            }
            None => format!("{}/{} internal", self.container_port, self.protocol),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ServiceDetail {
    pub card: ServiceCard,
    pub project: Option<String>,
    pub image_id: Option<String>,
    pub command: Option<String>,
    pub created: Option<String>,
    pub started_ago: Option<String>,
    pub restart_count: i64,
    pub exit_code: Option<i64>,
    pub restart_policy: Option<String>,
    pub working_dir: Option<String>,
    pub config_files: Option<String>,
    pub networks: Vec<NetworkInfo>,
    pub env_names: Vec<String>,
    pub mounts: Vec<MountInfo>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub ip: Option<String>,
    pub aliases: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountInfo {
    pub kind: String,
    pub source: String,
    pub destination: String,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogLine {
    pub ts: String,
    pub err: bool,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StatsSample {
    pub cpu: f64,
    pub cpus: u32,
    pub mem: u64,
    pub mem_limit: u64,
}

pub fn network_hue(name: &str) -> u32 {
    let mut hash: u32 = 0x811c_9dc5;
    for byte in name.bytes() {
        hash ^= u32::from(byte);
        hash = hash.wrapping_mul(0x0100_0193);
    }
    hash % 360
}

pub fn valid_container_ref(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}
