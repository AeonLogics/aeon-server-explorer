use serde::{Deserialize, Serialize};

// One upload is streamed through, so this only stops runaway files.
pub const MAX_FILE_UPLOAD: u64 = 2 * 1024 * 1024 * 1024;
// A single-file download is read whole before it's sent.
pub const MAX_FILE_DOWNLOAD: u64 = 512 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    // Server time, like "2026-11-13 14:05".
    pub modified: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Listing {
    pub path: String,
    pub entries: Vec<FileEntry>,
}

// A path inside a volume, relative to its root: "" or "cakes" or "cakes/2026".
// No "." or "..", no backslashes or control characters, so it can't leave the volume.
pub fn valid_rel_path(path: &str) -> bool {
    if path.is_empty() {
        return true;
    }
    path.len() <= 1024 && path.split('/').all(valid_segment)
}

fn valid_segment(part: &str) -> bool {
    !part.is_empty()
        && part != "."
        && part != ".."
        && part.len() <= 255
        && !part.chars().any(|c| c.is_control() || c == '\\' || c == '/')
}

// A file name for an upload. Tar's plain header holds 100 bytes, so that's the limit.
pub fn valid_file_name(name: &str) -> bool {
    valid_segment(name) && name.len() <= 100
}

pub fn join_path(dir: &str, name: &str) -> String {
    if dir.is_empty() { name.to_string() } else { format!("{dir}/{name}") }
}
