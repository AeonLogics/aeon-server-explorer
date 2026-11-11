use std::time::Duration;

use bollard::Docker;
use bollard::errors::Error;
use bollard::query_parameters::{
    CreateImageOptionsBuilder, ListImagesOptionsBuilder, RemoveImageOptions, SearchImagesOptionsBuilder,
};
use chrono::{DateTime, Local};
use futures::{Stream, StreamExt};
use serde::Deserialize;
use shared::errors::docker::DockerError;
use shared::images::{HubResult, HubTag, LocalImage, PullEvent, is_docker_hub, valid_repo};
use tokio::sync::mpsc;

use crate::docker::{list_containers, map_err};

const HUB_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn local(docker: &Docker) -> Result<Vec<LocalImage>, DockerError> {
    let options = ListImagesOptionsBuilder::new().all(false).build();
    let images = docker.list_images(Some(options)).await.map_err(map_err)?;
    let containers = list_containers(docker, None).await?;

    let mut out: Vec<LocalImage> = images
        .into_iter()
        .flat_map(|image| {
            let used_by = containers
                .iter()
                .filter(|c| c.image_id.as_deref() == Some(image.id.as_str()))
                .count();
            let created = DateTime::from_timestamp(image.created, 0)
                .map(|t| t.with_timezone(&Local).format("%Y-%m-%d").to_string())
                .unwrap_or_default();
            let short_id: String = image.id.trim_start_matches("sha256:").chars().take(12).collect();
            let mut tags: Vec<String> = image
                .repo_tags
                .iter()
                .filter(|t| !t.starts_with("<none>"))
                .cloned()
                .collect();
            if tags.is_empty() {
                tags.push(short_id.clone());
            }
            tags.into_iter().map(move |reference| LocalImage {
                id: short_id.clone(),
                reference,
                size: image.size,
                created: created.clone(),
                used_by,
            })
        })
        .collect();
    out.sort_by(|a, b| a.reference.cmp(&b.reference));
    Ok(out)
}

pub async fn search(docker: &Docker, term: &str) -> Result<Vec<HubResult>, DockerError> {
    let options = SearchImagesOptionsBuilder::new().term(term).limit(25).build();
    let results = docker.search_images(options).await.map_err(hub_err)?;
    let mut out: Vec<HubResult> = results
        .into_iter()
        .filter_map(|r| {
            Some(HubResult {
                name: r.name?,
                description: r.description.unwrap_or_default(),
                stars: r.star_count.unwrap_or(0),
                official: r.is_official.unwrap_or(false),
            })
        })
        .collect();
    out.sort_by(|a, b| b.official.cmp(&a.official).then(b.stars.cmp(&a.stars)));
    Ok(out)
}

#[derive(Deserialize)]
struct TagPage {
    results: Vec<TagItem>,
}

#[derive(Deserialize)]
struct TagItem {
    name: String,
    full_size: Option<i64>,
    last_updated: Option<String>,
}

// Docker Hub's public API; other registries don't offer a standard tag list.
pub async fn tags(repo: &str) -> Result<Vec<HubTag>, DockerError> {
    if !valid_repo(repo) {
        return Err(DockerError::Failed("That isn't a valid image name.".into()));
    }
    if !is_docker_hub(repo) {
        return Err(DockerError::Failed("Tags are only listed for Docker Hub images. Type the tag yourself.".into()));
    }
    let path = if repo.contains('/') { repo.to_string() } else { format!("library/{repo}") };
    let url = format!("https://hub.docker.com/v2/repositories/{path}/tags?page_size=30&ordering=last_updated");

    let client = reqwest::Client::builder()
        .timeout(HUB_TIMEOUT)
        .user_agent("aeon-server-explorer")
        .build()
        .map_err(|e| DockerError::Failed(format!("Couldn't reach Docker Hub: {e}")))?;
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| DockerError::Failed(format!("Docker Hub didn't answer: {e}")))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(DockerError::Failed(format!("Docker Hub has no image called {repo}.")));
    }
    if !response.status().is_success() {
        return Err(DockerError::Failed(format!("Docker Hub answered {}.", response.status())));
    }
    let page: TagPage = response
        .json()
        .await
        .map_err(|e| DockerError::Failed(format!("Docker Hub sent something unexpected: {e}")))?;

    Ok(page
        .results
        .into_iter()
        .map(|t| HubTag {
            name: t.name,
            size: t.full_size,
            updated: t.last_updated.map(|d| d.chars().take(10).collect()),
        })
        .collect())
}

// The pull keeps going even if the tab is closed; the browser only watches it.
pub fn pull(docker: Docker, repo: String, tag: String) -> impl Stream<Item = PullEvent> {
    let (tx, rx) = mpsc::channel::<PullEvent>(256);
    tokio::spawn(async move {
        let options = CreateImageOptionsBuilder::new().from_image(&repo).tag(&tag).build();
        let mut source = std::pin::pin!(docker.create_image(Some(options), None, None));
        let mut failure: Option<String> = None;
        while let Some(item) = source.next().await {
            let event = match item {
                Ok(info) => {
                    if let Some(message) = info.error_detail.as_ref().and_then(|d| d.message.clone()) {
                        failure = Some(message.clone());
                        PullEvent { error: Some(message), ..Default::default() }
                    } else {
                        PullEvent {
                            status: info.status.clone().unwrap_or_default(),
                            layer: info.id.clone(),
                            current: info.progress_detail.as_ref().and_then(|p| p.current),
                            total: info.progress_detail.as_ref().and_then(|p| p.total),
                            error: None,
                            done: false,
                        }
                    }
                }
                Err(e) => {
                    failure = Some(pull_message(e));
                    break;
                }
            };
            let _ = tx.send(event).await;
        }
        let _ = tx.send(PullEvent { done: true, error: failure, ..Default::default() }).await;
    });
    futures::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|item| (item, rx)) })
}

pub async fn remove(docker: &Docker, reference: &str) -> Result<(), DockerError> {
    match docker.remove_image(reference, None::<RemoveImageOptions>, None).await {
        Ok(_) => Ok(()),
        Err(Error::DockerResponseServerError { status_code: 409, .. }) => Err(DockerError::Failed(
            "A container still uses this image. Remove or change that service first.".into(),
        )),
        Err(e) => Err(map_err(e)),
    }
}

fn pull_message(err: Error) -> String {
    match err {
        Error::DockerResponseServerError { status_code: 404, .. } => "That image or tag doesn't exist.".into(),
        Error::DockerResponseServerError { message, .. } => message,
        other => other.to_string(),
    }
}

fn hub_err(err: Error) -> DockerError {
    match err {
        Error::DockerResponseServerError { message, .. } => DockerError::Failed(format!("Docker Hub search failed: {message}")),
        other => map_err(other),
    }
}
