use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use sha2::{Digest, Sha256};
use shared::errors::docker::DockerError;
use shared::stack::StackSpec;

use super::EnvChange;
use super::compose::env_file;

// Write to a temp file next to the target, then rename, so a crash never leaves half a file.
pub fn write_atomic(path: &Path, bytes: &[u8], private: bool) -> Result<(), DockerError> {
    let fail = |e: std::io::Error| DockerError::Failed(format!("Couldn't write {}: {e}", path.display()));
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(fail)?;
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(parent, std::fs::Permissions::from_mode(0o700)).map_err(fail)?;
        }
    }
    let tmp = path.with_extension("tmp");
    {
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let _ = private;
        let mut file = options.open(&tmp).map_err(fail)?;
        file.write_all(bytes).map_err(fail)?;
        file.sync_all().map_err(fail)?;
    }
    std::fs::rename(&tmp, path).map_err(fail)
}

pub fn read_env(path: &Path) -> BTreeMap<String, String> {
    std::fs::read_to_string(path)
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_once('='))
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect()
}

// Keeps existing values for names still in the spec, applies changes, drops removed names,
// and deletes env files of services that are gone.
pub fn write_env_files(dir: &Path, spec: &StackSpec, changes: &[EnvChange]) -> Result<(), DockerError> {
    for service in &spec.services {
        let path = dir.join(env_file(&service.name));
        if service.env.is_empty() {
            let _ = std::fs::remove_file(&path);
            continue;
        }
        let old = read_env(&path);
        let mut body = String::new();
        for name in &service.env {
            let value = changes
                .iter()
                .rev()
                .find(|c| c.service == service.name && c.name == *name)
                .map(|c| c.value.clone())
                .or_else(|| old.get(name).cloned())
                .unwrap_or_default();
            body.push_str(name);
            body.push('=');
            body.push_str(&value);
            body.push('\n');
        }
        write_atomic(&path, body.as_bytes(), true)?;
    }

    if let Ok(entries) = std::fs::read_dir(dir.join("env")) {
        for entry in entries.flatten() {
            let file = entry.file_name().to_string_lossy().to_string();
            let keep = spec
                .services
                .iter()
                .any(|s| !s.env.is_empty() && format!("{}.env", s.name) == file);
            if !keep && file.ends_with(".env") {
                let _ = std::fs::remove_file(entry.path());
            }
        }
    }
    Ok(())
}

pub fn read_value(dir: &Path, service: &str, name: &str) -> Option<String> {
    read_env(&dir.join(env_file(service))).get(name).cloned()
}

// What "applied" means: the compose file plus every env file, byte for byte.
pub fn fingerprint(dir: &Path, spec: &StackSpec) -> String {
    let mut hash = Sha256::new();
    hash.update(std::fs::read(dir.join(super::COMPOSE)).unwrap_or_default());
    for service in &spec.services {
        hash.update(service.name.as_bytes());
        hash.update(std::fs::read(dir.join(env_file(&service.name))).unwrap_or_default());
    }
    hash.finalize().iter().map(|b| format!("{b:02x}")).collect()
}
