use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use surrealdb::types::SurrealValue;

pub const STANDALONE: &str = "(standalone)";
pub const STANDALONE_SLUG: &str = "_standalone";

// Marks a helper that runs once and stops; finishing with exit code 0 is its healthy state.
pub const LABEL_RUN_ONCE: &str = "dev.aeonlogics.explorer.run-once";
pub const FINISHED: &str = "Finished";

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
    pub cpus: Option<i64>,
    pub memory: Option<i64>,
    pub images: Option<i64>,
    pub projects: Vec<ProjectSummary>,
}

// One container, just enough for the overview's strip, cards and "needs you" list.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Member {
    pub name: String,
    pub container: String,
    pub health: Health,
    pub health_label: String,
    pub running: bool,
    pub status_text: String,
    pub exposed: Vec<String>,
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
    pub members: Vec<Member>,
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

impl ServiceCard {
    // Running, or a run-once helper that finished without an error.
    pub fn is_settled(&self) -> bool {
        self.state == "running" || self.health_label == FINISHED
    }
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
    pub service: Option<String>,
    pub redeploy_blocker: Option<String>,
    pub is_self: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Start,
    Stop,
    Restart,
    Redeploy,
}

impl Action {
    pub fn audit_name(self) -> &'static str {
        match self {
            Self::Start => "container.start",
            Self::Stop => "container.stop",
            Self::Restart => "container.restart",
            Self::Redeploy => "container.redeploy",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Start => "Start",
            Self::Stop => "Stop",
            Self::Restart => "Restart",
            Self::Redeploy => "Redeploy",
        }
    }

    pub fn done(self) -> &'static str {
        match self {
            Self::Start => "Started",
            Self::Stop => "Stopped",
            Self::Restart => "Restarted",
            Self::Redeploy => "Redeployed",
        }
    }

    // Stopping takes a service offline until someone starts it again, so it needs the name typed.
    pub fn needs_typed_name(self) -> bool {
        matches!(self, Self::Stop)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(SurrealValue))]
pub struct AuditEntry {
    pub id: String,
    pub at: String,
    pub action: String,
    pub actor: Option<String>,
    pub target: Option<String>,
    pub ip: Option<String>,
    pub detail: Option<String>,
    pub ok: Option<bool>,
}

impl AuditEntry {
    pub fn failed(&self) -> bool {
        self.ok == Some(false)
            || matches!(
                self.action.as_str(),
                "login.failed" | "login.blocked" | "login.locked" | "password.change_failed"
            )
    }

    pub fn describe(&self) -> &'static str {
        match self.action.as_str() {
            "admin.created" => "Admin account created",
            "login.ok" => "Signed in",
            "login.failed" => "Wrong email or password",
            "login.blocked" => "Sign-in blocked while locked",
            "login.locked" => "Locked after 5 failed tries",
            "logout" => "Signed out",
            "password.changed" => "Password changed",
            "password.change_failed" => "Password change refused",
            "container.start" => "Started container",
            "container.stop" => "Stopped container",
            "container.restart" => "Restarted container",
            "container.redeploy" => "Redeployed service",
            "stack.create" => "Created stack",
            "stack.save" => "Saved stack",
            "stack.apply" => "Deployed stack",
            "env.reveal" => "Revealed an env value",
            "env.reveal_all" => "Revealed all env values",
            "image.pull" => "Pulled image",
            "image.remove" => "Removed image",
            "image.load" => "Uploaded image",
            _ => "Other event",
        }
    }
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
