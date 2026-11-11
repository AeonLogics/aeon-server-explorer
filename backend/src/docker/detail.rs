use bollard::Docker;
use bollard::query_parameters::InspectContainerOptions;
use chrono::{DateTime, Local, Utc};
use shared::docker::{MountInfo, NetworkInfo, ServiceDetail};
use shared::errors::docker::DockerError;

use super::actions::Target;
use super::summary::{card, list};
use super::{LABEL_CONFIG_FILES, LABEL_PROJECT, LABEL_WORKING_DIR, humanize_secs, map_err};

pub async fn service(docker: &Docker, id: &str) -> Result<ServiceDetail, DockerError> {
    let inspect = docker
        .inspect_container(id, None::<InspectContainerOptions>)
        .await
        .map_err(map_err)?;
    let full_id = inspect.id.clone().ok_or(DockerError::NotFound)?;

    let summary = list(docker, Some(&full_id))
        .await?
        .into_iter()
        .next()
        .ok_or(DockerError::NotFound)?;
    let card = card(&summary);
    let labels = summary.labels.clone().unwrap_or_default();
    let target = Target::from_labels(card.container_name.clone(), &labels);

    let state = inspect.state.as_ref();
    let running = state.and_then(|s| s.running).unwrap_or(false);
    let started_ago = state
        .filter(|_| running)
        .and_then(|s| s.started_at.as_deref())
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| humanize_secs((Utc::now() - t.with_timezone(&Utc)).num_seconds()));
    let exit_code = state.filter(|_| !running).and_then(|s| s.exit_code);

    let created = inspect
        .created
        .as_deref()
        .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
        .map(|t| t.with_timezone(&Local).format("%Y-%m-%d %H:%M").to_string());

    let restart_policy = inspect
        .host_config
        .as_ref()
        .and_then(|h| h.restart_policy.as_ref())
        .and_then(|r| r.name.as_ref())
        .map(|n| n.to_string())
        .filter(|n| !n.is_empty());

    let mut env_names: Vec<String> = inspect
        .config
        .as_ref()
        .and_then(|c| c.env.as_ref())
        .into_iter()
        .flatten()
        .filter_map(|pair| pair.split('=').next())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect();
    env_names.sort();
    env_names.dedup();

    let mut networks: Vec<NetworkInfo> = inspect
        .network_settings
        .as_ref()
        .and_then(|n| n.networks.as_ref())
        .into_iter()
        .flatten()
        .map(|(name, endpoint)| NetworkInfo {
            name: name.clone(),
            ip: endpoint.ip_address.clone().filter(|ip| !ip.is_empty()),
            aliases: endpoint.aliases.clone().unwrap_or_default(),
        })
        .collect();
    networks.sort_by(|a, b| a.name.cmp(&b.name));

    let mounts = inspect
        .mounts
        .iter()
        .flatten()
        .map(|m| MountInfo {
            kind: m.typ.as_ref().map(|t| t.to_string()).unwrap_or_else(|| "mount".into()),
            source: m
                .name
                .clone()
                .filter(|n| !n.is_empty())
                .or_else(|| m.source.clone())
                .unwrap_or_default(),
            destination: m.destination.clone().unwrap_or_default(),
            read_only: !m.rw.unwrap_or(true),
        })
        .collect();

    Ok(ServiceDetail {
        card,
        project: labels.get(LABEL_PROJECT).cloned(),
        service: target.service.clone(),
        // Managed stacks are deployed with Apply (it never pulls), not with Redeploy.
        redeploy_blocker: if labels.contains_key(crate::stacks::LABEL_MANAGED) {
            Some("This stack is managed here: change it in the stack page and press Apply.".into())
        } else {
            target.redeploy_blocker()
        },
        is_self: target.is_self,
        image_id: inspect.image.clone(),
        command: summary.command.clone().filter(|c| !c.is_empty()),
        created,
        started_ago,
        restart_count: inspect.restart_count.unwrap_or(0),
        exit_code,
        restart_policy,
        working_dir: labels.get(LABEL_WORKING_DIR).cloned(),
        config_files: labels.get(LABEL_CONFIG_FILES).cloned(),
        networks,
        env_names,
        mounts,
    })
}
