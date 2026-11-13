use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalImage {
    pub id: String,
    pub reference: String,
    pub size: i64,
    pub created: String,
    pub used_by: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubResult {
    pub name: String,
    pub description: String,
    pub stars: i64,
    pub official: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HubTag {
    pub name: String,
    pub size: Option<i64>,
    pub updated: Option<String>,
}

// One step of a pull, as sent to the browser. The last one has `done: true`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PullEvent {
    pub status: String,
    pub layer: Option<String>,
    pub current: Option<i64>,
    pub total: Option<i64>,
    pub error: Option<String>,
    pub done: bool,
}

// Uploads are streamed straight into Docker, so this only guards against runaway files.
pub const MAX_UPLOAD: u64 = 8 * 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedImage {
    pub reference: String,
    // Like "linux/amd64"; empty when Docker doesn't say.
    pub platform: String,
    pub fits: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadResult {
    pub images: Vec<LoadedImage>,
    pub server_platform: String,
}

// A repository name: registry host, path and name, without the tag.
pub fn valid_repo(repo: &str) -> bool {
    !repo.is_empty()
        && repo.len() <= 200
        && repo.chars().next().is_some_and(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        && repo
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '_' | '-' | '/' | ':'))
}

pub fn valid_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 128
        && tag.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

// Docker Hub names have no registry host in front (no dot or colon in the first part).
pub fn is_docker_hub(repo: &str) -> bool {
    let first = repo.split('/').next().unwrap_or_default();
    !(repo.contains('/') && (first.contains('.') || first.contains(':')))
}
