use std::collections::{BTreeMap, HashMap};
use std::net::IpAddr;

use bollard::Docker;
use bollard::models::ContainerSummary;
use bollard::query_parameters::ListContainersOptionsBuilder;
use shared::docker::{
    FINISHED, Health, LABEL_RUN_ONCE, Member, Overview, PortInfo, ProjectDetail, ProjectSummary, STANDALONE,
    STANDALONE_SLUG, ServiceCard, ServiceKind,
};
use shared::errors::docker::DockerError;

use super::{LABEL_NUMBER, LABEL_PROJECT, LABEL_SERVICE, LABEL_WORKING_DIR, map_err};

pub async fn overview(docker: &Docker) -> Result<Overview, DockerError> {
    let containers = list(docker, None).await?;
    // Server facts are nice to have; the overview still works if Docker won't say.
    let info = docker.info().await.ok();
    let engine_version = info.as_ref().and_then(|i| i.server_version.clone());
    let cpus = info.as_ref().and_then(|i| i.ncpu);
    let memory = info.as_ref().and_then(|i| i.mem_total);
    let images = info.as_ref().and_then(|i| i.images);

    let projects = group(&containers)
        .into_iter()
        .map(|(key, members)| summarize(key, &cards(&members)))
        .collect();

    Ok(Overview { engine_version, cpus, memory, images, projects })
}

pub async fn project(docker: &Docker, slug: &str) -> Result<ProjectDetail, DockerError> {
    let containers = list(docker, None).await?;
    let key = (slug != STANDALONE_SLUG).then(|| slug.to_string());

    let members: Vec<&ContainerSummary> = containers
        .iter()
        .filter(|c| project_label(c).map(str::to_string) == key)
        .collect();
    if members.is_empty() {
        return Err(DockerError::NotFound);
    }

    let working_dir = members
        .iter()
        .find_map(|c| c.labels.as_ref()?.get(LABEL_WORKING_DIR).cloned());
    let services = cards(&members);
    let summary = summarize(key, &services);

    Ok(ProjectDetail { summary, working_dir, services })
}

pub(crate) async fn list(
    docker: &Docker,
    id: Option<&str>,
) -> Result<Vec<ContainerSummary>, DockerError> {
    let mut builder = ListContainersOptionsBuilder::new().all(true);
    if let Some(id) = id {
        let filters = HashMap::from([("id", vec![id])]);
        builder = builder.filters(&filters);
    }
    let mut all = docker.list_containers(Some(builder.build())).await.map_err(map_err)?;
    // The file helpers of the storage browser live for a second; they're not part of the server.
    all.retain(|c| !c.labels.as_ref().is_some_and(|l| l.contains_key(crate::volumes::LABEL_HELPER)));
    Ok(all)
}

fn project_label(c: &ContainerSummary) -> Option<&str> {
    c.labels.as_ref()?.get(LABEL_PROJECT).map(String::as_str)
}

// Compose projects in name order, "(standalone)" last.
fn group(containers: &[ContainerSummary]) -> Vec<(Option<String>, Vec<&ContainerSummary>)> {
    let mut compose: BTreeMap<String, Vec<&ContainerSummary>> = BTreeMap::new();
    let mut standalone = Vec::new();
    for c in containers {
        match project_label(c) {
            Some(name) => compose.entry(name.to_string()).or_default().push(c),
            None => standalone.push(c),
        }
    }

    let mut groups: Vec<_> = compose.into_iter().map(|(k, v)| (Some(k), v)).collect();
    if !standalone.is_empty() {
        groups.push((None, standalone));
    }
    groups
}

fn summarize(key: Option<String>, cards: &[ServiceCard]) -> ProjectSummary {
    let total = cards.len() as u32;
    // A finished run-once helper counts as up: it did its job and isn't meant to keep running.
    let running = cards.iter().filter(|c| c.is_settled()).count() as u32;
    let unhealthy = cards.iter().filter(|c| c.health_label == "Unhealthy").count() as u32;
    let exposed_ports = cards
        .iter()
        .flat_map(|c| &c.ports)
        .filter(|p| p.exposed)
        .count() as u32;

    let health = if running == 0 {
        Health::Down
    } else if cards.iter().any(|c| c.health != Health::Ok) {
        Health::Warn
    } else {
        Health::Ok
    };

    let (name, slug, standalone) = match key {
        Some(name) => (name.clone(), name, false),
        None => (STANDALONE.to_string(), STANDALONE_SLUG.to_string(), true),
    };

    let members = cards
        .iter()
        .map(|c| Member {
            name: c.name.clone(),
            container: c.container_name.clone(),
            health: c.health,
            health_label: c.health_label.clone(),
            running: c.is_settled(),
            status_text: c.status_text.clone(),
            exposed: c.ports.iter().filter(|p| p.exposed).map(PortInfo::label).collect(),
        })
        .collect();

    ProjectSummary { name, slug, standalone, running, total, unhealthy, exposed_ports, health, members }
}

fn cards(members: &[&ContainerSummary]) -> Vec<ServiceCard> {
    let mut cards: Vec<ServiceCard> = members.iter().map(|c| card(c)).collect();
    cards.sort_by(|a, b| a.name.cmp(&b.name));
    cards
}

pub(crate) fn card(c: &ContainerSummary) -> ServiceCard {
    let labels = c.labels.clone().unwrap_or_default();
    let container_name = c
        .names
        .as_ref()
        .and_then(|n| n.first())
        .map(|n| n.trim_start_matches('/').to_string())
        .unwrap_or_default();

    let name = match labels.get(LABEL_SERVICE) {
        Some(service) => match labels.get(LABEL_NUMBER).and_then(|n| n.parse::<u32>().ok()) {
            Some(n) if n > 1 => format!("{service} #{n}"),
            _ => service.clone(),
        },
        None => container_name.clone(),
    };

    let image = c.image.clone().unwrap_or_default();
    let state = c.state.as_ref().map(|s| s.to_string()).unwrap_or_else(|| "unknown".into());
    let check = c
        .health
        .as_ref()
        .and_then(|h| h.status.as_ref())
        .map(|s| s.to_string())
        .unwrap_or_default();
    let status_text = c.status.clone().unwrap_or_default();
    let finished_helper = labels.get(LABEL_RUN_ONCE).map(String::as_str) == Some("true")
        && state == "exited"
        && status_text.starts_with("Exited (0)");
    let (health, health_label) = if finished_helper { (Health::Ok, FINISHED) } else { classify(&state, &check) };

    let mut networks: Vec<String> = c
        .network_settings
        .as_ref()
        .and_then(|n| n.networks.as_ref())
        .map(|n| n.keys().cloned().collect())
        .unwrap_or_default();
    networks.sort();

    ServiceCard {
        id: c.id.clone().unwrap_or_default(),
        name,
        container_name,
        kind: kind_of(&image),
        image,
        state,
        status_text,
        health,
        health_label: health_label.to_string(),
        ports: ports(c),
        networks,
    }
}

fn classify(state: &str, check: &str) -> (Health, &'static str) {
    match (state, check) {
        ("running", "unhealthy") => (Health::Down, "Unhealthy"),
        ("running", "starting") => (Health::Warn, "Starting"),
        ("running", "healthy") => (Health::Ok, "Healthy"),
        ("running", _) => (Health::Ok, "Running"),
        ("restarting", _) => (Health::Down, "Restarting"),
        ("paused", _) => (Health::Warn, "Paused"),
        ("created", _) => (Health::Down, "Created"),
        ("removing", _) => (Health::Warn, "Removing"),
        ("stopping", _) => (Health::Warn, "Stopping"),
        ("dead", _) => (Health::Down, "Dead"),
        _ => (Health::Down, "Exited"),
    }
}

fn kind_of(image: &str) -> ServiceKind {
    let image = image.to_ascii_lowercase();
    let base = image.rsplit('/').next().unwrap_or(&image);
    const DATABASES: &[&str] = &[
        "postgres", "postgis", "mysql", "mariadb", "mongo", "redis", "valkey", "keydb", "surrealdb",
        "clickhouse", "cockroach", "memcached", "influxdb", "timescale", "elasticsearch", "minio",
    ];
    const PROXIES: &[&str] = &["caddy", "nginx", "traefik", "haproxy", "cloudflared", "envoy"];
    if DATABASES.iter().any(|d| base.starts_with(d)) {
        ServiceKind::Database
    } else if PROXIES.iter().any(|p| base.starts_with(p)) {
        ServiceKind::Proxy
    } else {
        ServiceKind::App
    }
}

// Docker lists one entry per address family (0.0.0.0 and ::), so merge those.
fn ports(c: &ContainerSummary) -> Vec<PortInfo> {
    let mut out: Vec<PortInfo> = Vec::new();
    for p in c.ports.iter().flatten() {
        let protocol = p.typ.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "tcp".into());
        let host_ip = p.ip.clone().filter(|ip| !ip.is_empty());
        let exposed = p.public_port.is_some() && !is_loopback(host_ip.as_deref());

        let duplicate = out.iter_mut().find(|o| {
            o.container_port == p.private_port
                && o.host_port == p.public_port
                && o.protocol == protocol
                && o.exposed == exposed
        });
        match duplicate {
            Some(existing) => {
                if host_ip.as_deref() == Some("0.0.0.0") {
                    existing.host_ip = host_ip;
                }
            }
            None => out.push(PortInfo {
                host_ip,
                host_port: p.public_port,
                container_port: p.private_port,
                protocol,
                exposed,
            }),
        }
    }
    out.sort_by_key(|p| (p.host_port.is_none(), p.container_port));
    out
}

fn is_loopback(ip: Option<&str>) -> bool {
    ip.and_then(|ip| ip.parse::<IpAddr>().ok())
        .is_some_and(|ip| ip.is_loopback())
}
