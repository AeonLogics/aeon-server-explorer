// Files inside Docker volumes, without touching /var/lib/docker: every operation goes through a
// small helper container that has only the volume mounted and no network.

use std::collections::HashMap;
use std::time::Duration;

use bollard::Docker;
use bollard::container::LogOutput;
use bollard::errors::Error;
use bollard::models::{ContainerCreateBody, HostConfig};
use bollard::query_parameters::{
    CreateContainerOptions, CreateImageOptionsBuilder, DownloadFromContainerOptionsBuilder, ListVolumesOptions,
    LogsOptionsBuilder, RemoveContainerOptionsBuilder, StartContainerOptions, UploadToContainerOptionsBuilder,
    WaitContainerOptions,
};
use bytes::Bytes;
use futures::{Stream, StreamExt};
use shared::errors::docker::DockerError;
use shared::volumes::{FileEntry, Listing, MAX_FILE_DOWNLOAD, valid_file_name, valid_rel_path};
use tokio::sync::mpsc;

use crate::docker::{LABEL_SELF, list_containers, map_err};

const HELPER_IMAGE: &str = "alpine:3";
pub const LABEL_HELPER: &str = "dev.aeonlogics.explorer.helper";
const ROOT: &str = "/v";
const RUN_TIMEOUT: Duration = Duration::from_secs(10 * 60);
const TRANSFER_TIMEOUT: Duration = Duration::from_secs(60 * 60);

fn failed(message: impl Into<String>) -> DockerError {
    DockerError::Failed(message.into())
}

fn target(path: &str) -> Result<String, DockerError> {
    if !valid_rel_path(path) {
        return Err(failed("That path isn't allowed."));
    }
    Ok(if path.is_empty() { ROOT.to_string() } else { format!("{ROOT}/{path}") })
}

pub async fn exists(docker: &Docker, volume: &str) -> Result<(), DockerError> {
    match docker.inspect_volume(volume).await {
        Ok(_) => Ok(()),
        Err(Error::DockerResponseServerError { status_code: 404, .. }) => Err(failed(
            "This storage doesn't exist on the server yet. Deploy the stack once so Docker creates it.",
        )),
        Err(e) => Err(map_err(e)),
    }
}

async fn ensure_helper_image(docker: &Docker) -> Result<(), DockerError> {
    if docker.inspect_image(HELPER_IMAGE).await.is_ok() {
        return Ok(());
    }
    let options = CreateImageOptionsBuilder::new().from_image("alpine").tag("3").build();
    let mut pull = std::pin::pin!(docker.create_image(Some(options), None, None));
    while let Some(step) = pull.next().await {
        step.map_err(|e| failed(format!("Couldn't get the helper image {HELPER_IMAGE}: {e}")))?;
    }
    Ok(())
}

// A stopped container with the given mounts, removed again by `remove`.
async fn create_helper(docker: &Docker, binds: Vec<String>, cmd: Option<Vec<String>>) -> Result<String, DockerError> {
    ensure_helper_image(docker).await?;
    let body = ContainerCreateBody {
        image: Some(HELPER_IMAGE.to_string()),
        cmd,
        labels: Some(HashMap::from([(LABEL_HELPER.to_string(), "true".to_string())])),
        network_disabled: Some(true),
        host_config: Some(HostConfig {
            binds: Some(binds),
            network_mode: Some("none".to_string()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let created = docker
        .create_container(None::<CreateContainerOptions>, body)
        .await
        .map_err(map_err)?;
    Ok(created.id)
}

async fn remove(docker: &Docker, id: &str) {
    let options = RemoveContainerOptionsBuilder::new().force(true).build();
    let _ = docker.remove_container(id, Some(options)).await;
}

struct Ran {
    code: i64,
    out: String,
    err: String,
}

async fn run(docker: &Docker, binds: Vec<String>, cmd: Vec<String>) -> Result<Ran, DockerError> {
    let id = create_helper(docker, binds, Some(cmd)).await?;
    let result = async {
        docker.start_container(&id, None::<StartContainerOptions>).await.map_err(map_err)?;
        let code = tokio::time::timeout(RUN_TIMEOUT, async {
            let mut wait = std::pin::pin!(docker.wait_container(&id, None::<WaitContainerOptions>));
            let mut code = 0;
            while let Some(step) = wait.next().await {
                match step {
                    Ok(done) => code = done.status_code,
                    Err(Error::DockerContainerWaitError { code: c, .. }) => code = c,
                    Err(e) => return Err(map_err(e)),
                }
            }
            Ok(code)
        })
        .await
        .map_err(|_| failed("The helper took too long."))??;

        let options = LogsOptionsBuilder::new().stdout(true).stderr(true).build();
        let mut logs = std::pin::pin!(docker.logs(&id, Some(options)));
        let (mut out, mut err) = (String::new(), String::new());
        while let Some(item) = logs.next().await {
            match item.map_err(map_err)? {
                LogOutput::StdOut { message } => out.push_str(&String::from_utf8_lossy(&message)),
                LogOutput::StdErr { message } => err.push_str(&String::from_utf8_lossy(&message)),
                _ => {}
            }
        }
        Ok(Ran { code, out, err })
    }
    .await;
    remove(docker, &id).await;
    result
}

fn helper_error(ran: &Ran, what: &str) -> DockerError {
    let detail = ran.err.trim().replace("/v/", "").replace("/v", "");
    if detail.is_empty() { failed(format!("Couldn't {what}.")) } else { failed(format!("Couldn't {what}: {detail}")) }
}

pub async fn list(docker: &Docker, volume: &str, path: &str) -> Result<Listing, DockerError> {
    let dir = target(path)?;
    exists(docker, volume).await?;
    let cmd = ["find", &dir, "-mindepth", "1", "-maxdepth", "1", "-exec", "stat", "-c", "%F|%s|%Y|%n", "{}", "+"];
    let ran = run(docker, vec![format!("{volume}:{ROOT}:ro")], cmd.iter().map(|s| s.to_string()).collect()).await?;
    if ran.code != 0 {
        return Err(helper_error(&ran, "open that folder"));
    }

    let prefix = format!("{dir}/");
    let mut entries: Vec<FileEntry> = ran
        .out
        .lines()
        .filter_map(|line| {
            let mut parts = line.splitn(4, '|');
            let kind = parts.next()?;
            let size = parts.next()?.parse().unwrap_or(0);
            let secs: i64 = parts.next()?.parse().unwrap_or(0);
            let modified = chrono::DateTime::from_timestamp(secs, 0)
                .map(|t| t.with_timezone(&chrono::Local).format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_default();
            let name = parts.next()?.strip_prefix(&prefix)?.to_string();
            Some(FileEntry { name, is_dir: kind == "directory", size, modified })
        })
        .collect();
    entries.sort_by(|a, b| b.is_dir.cmp(&a.is_dir).then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase())));
    Ok(Listing { path: path.to_string(), entries })
}

// New folders get the owner of the folder they're in, so the app using the volume can write there.
pub async fn make_folder(docker: &Docker, volume: &str, path: &str) -> Result<(), DockerError> {
    if path.is_empty() {
        return Err(failed("Give the folder a name."));
    }
    let dir = target(path)?;
    exists(docker, volume).await?;
    let script = r#"mkdir -- "$1" && chown "$(stat -c %u:%g "$(dirname "$1")")" "$1""#;
    let cmd = vec!["sh".into(), "-c".into(), script.into(), "sh".into(), dir];
    let ran = run(docker, vec![format!("{volume}:{ROOT}")], cmd).await?;
    if ran.code != 0 {
        return Err(helper_error(&ran, "create the folder"));
    }
    Ok(())
}

pub async fn delete(docker: &Docker, volume: &str, path: &str) -> Result<(), DockerError> {
    if path.is_empty() {
        return Err(failed("Pick a file or folder to delete."));
    }
    let item = target(path)?;
    exists(docker, volume).await?;
    let cmd = vec!["rm".into(), "-rf".into(), "--".into(), item];
    let ran = run(docker, vec![format!("{volume}:{ROOT}")], cmd).await?;
    if ran.code != 0 {
        return Err(helper_error(&ran, "delete it"));
    }
    Ok(())
}

// Copies everything from one volume into another, keeping owners. The source is mounted read-only.
pub async fn copy_in(docker: &Docker, from: &str, to: &str) -> Result<u64, DockerError> {
    if from == to {
        return Err(failed("Pick a different volume to copy from."));
    }
    exists(docker, from).await?;
    exists(docker, to).await?;
    let script = "cp -a /from/. /v/ && find /from -type f | wc -l";
    let cmd = vec!["sh".into(), "-c".into(), script.into()];
    let ran = run(docker, vec![format!("{from}:/from:ro"), format!("{to}:{ROOT}")], cmd).await?;
    if ran.code != 0 {
        return Err(helper_error(&ran, "copy the files"));
    }
    Ok(ran.out.trim().parse().unwrap_or(0))
}

// Every volume on the server except the explorer's own data.
pub async fn others(docker: &Docker, except: &str) -> Result<Vec<String>, DockerError> {
    let own: Vec<String> = list_containers(docker, None)
        .await?
        .into_iter()
        .filter(|c| c.labels.as_ref().is_some_and(|l| l.contains_key(LABEL_SELF)))
        .flat_map(|c| c.mounts.unwrap_or_default())
        .filter_map(|m| m.name)
        .collect();
    let listed = docker.list_volumes(None::<ListVolumesOptions>).await.map_err(map_err)?;
    let mut names: Vec<String> = listed
        .volumes
        .unwrap_or_default()
        .into_iter()
        .map(|v| v.name)
        .filter(|n| n != except && !own.contains(n) && !n.ends_with("_explorer-data"))
        .filter(|n| !(n.len() == 64 && n.chars().all(|c| c.is_ascii_hexdigit())))
        .collect();
    names.sort();
    Ok(names)
}

// Streams a file or folder out of the volume as a tar archive, the format Docker hands it over in.
pub async fn download(
    docker: &Docker,
    volume: &str,
    path: &str,
) -> Result<impl Stream<Item = Result<Bytes, std::io::Error>> + use<>, DockerError> {
    let item = target(path)?;
    exists(docker, volume).await?;
    let id = create_helper(docker, vec![format!("{volume}:{ROOT}:ro")], None).await?;
    let docker = docker.clone().with_timeout(TRANSFER_TIMEOUT);

    let (tx, rx) = mpsc::channel::<Result<Bytes, std::io::Error>>(8);
    tokio::spawn(async move {
        {
            let options = DownloadFromContainerOptionsBuilder::new().path(&item).build();
            let mut source = std::pin::pin!(docker.download_from_container(&id, Some(options)));
            while let Some(chunk) = source.next().await {
                let chunk = chunk.map_err(std::io::Error::other);
                let stop = chunk.is_err();
                if tx.send(chunk).await.is_err() || stop {
                    break;
                }
            }
        }
        remove(&docker, &id).await;
    });
    Ok(futures::stream::unfold(rx, |mut rx| async move { rx.recv().await.map(|item| (item, rx)) }))
}

// One file's bytes, read out of the tar Docker sends.
pub async fn download_file(docker: &Docker, volume: &str, path: &str) -> Result<Bytes, DockerError> {
    if path.is_empty() {
        return Err(failed("Pick a file."));
    }
    let stream = download(docker, volume, path).await?;
    let mut stream = std::pin::pin!(stream);
    let mut archive: Vec<u8> = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| failed(format!("Couldn't read the file: {e}")))?;
        archive.extend_from_slice(&chunk);
        if archive.len() as u64 > MAX_FILE_DOWNLOAD + 64 * 1024 {
            return Err(failed("That file is too big to open here. Use Download all instead."));
        }
    }
    first_file(Bytes::from(archive))
}

fn first_file(archive: Bytes) -> Result<Bytes, DockerError> {
    let mut pos = 0usize;
    while pos + 512 <= archive.len() {
        let header = &archive[pos..pos + 512];
        if header.iter().all(|b| *b == 0) {
            break;
        }
        let size = octal(&header[124..136]) as usize;
        let start = pos + 512;
        match header[156] {
            b'0' | 0 => {
                let end = start.checked_add(size).filter(|e| *e <= archive.len()).ok_or_else(|| failed("The file came back incomplete."))?;
                return Ok(archive.slice(start..end));
            }
            b'5' => return Err(failed("That's a folder. Open it, or use Download all.")),
            _ => {}
        }
        pos = start + size.div_ceil(512) * 512;
    }
    Err(failed("That isn't a regular file."))
}

fn octal(field: &[u8]) -> u64 {
    field
        .iter()
        .skip_while(|b| **b == b' ')
        .take_while(|b| (b'0'..=b'7').contains(b))
        .fold(0u64, |n, b| n * 8 + u64::from(b - b'0'))
}

// Streams one uploaded file into a folder of the volume. It's wrapped in a single-entry tar on the
// fly, owned by the folder's owner, so the app using the volume can read and replace it.
pub async fn upload<S>(docker: &Docker, volume: &str, dir: &str, name: &str, size: u64, body: S) -> Result<(), DockerError>
where
    S: Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static,
{
    if !valid_file_name(name) {
        return Err(failed("That file name isn't allowed (up to 100 characters, no slashes)."));
    }
    let folder = target(dir)?;
    exists(docker, volume).await?;

    let ran = run(
        docker,
        vec![format!("{volume}:{ROOT}:ro")],
        vec!["stat".into(), "-c".into(), "%u %g".into(), folder.clone()],
    )
    .await?;
    if ran.code != 0 {
        return Err(helper_error(&ran, "find that folder"));
    }
    let mut ids = ran.out.split_whitespace().map(|n| n.parse::<u32>().unwrap_or(0));
    let (uid, gid) = (ids.next().unwrap_or(0), ids.next().unwrap_or(0));

    let mtime = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let header = Bytes::from(tar_header(name, size, uid, gid, mtime).to_vec());
    let padding = Bytes::from(vec![0u8; ((512 - size % 512) % 512) as usize + 1024]);

    let mut sent: u64 = 0;
    let checked = body.map(move |chunk| {
        let chunk = chunk?;
        sent += chunk.len() as u64;
        if sent > size {
            return Err(std::io::Error::other("the file is bigger than announced"));
        }
        Ok(chunk)
    });
    let tar = futures::stream::once(async move { Ok::<_, std::io::Error>(header) })
        .chain(checked)
        .chain(futures::stream::once(async move { Ok(padding) }));

    let id = create_helper(docker, vec![format!("{volume}:{ROOT}")], None).await?;
    let docker_long = docker.clone().with_timeout(TRANSFER_TIMEOUT);
    let options = UploadToContainerOptionsBuilder::new().path(&folder).build();
    let result = docker_long
        .upload_to_container(&id, Some(options), bollard::body_try_stream(tar))
        .await
        .map_err(|e| failed(format!("Couldn't save the file: {e}")));
    remove(docker, &id).await;
    result
}

fn tar_header(name: &str, size: u64, uid: u32, gid: u32, mtime: u64) -> [u8; 512] {
    fn put_octal(field: &mut [u8], value: u64) {
        let digits = field.len() - 1;
        let text = format!("{value:0digits$o}");
        let bytes = text.as_bytes();
        let start = bytes.len().saturating_sub(digits);
        field[..digits].copy_from_slice(&bytes[start..]);
        field[digits] = 0;
    }
    let mut h = [0u8; 512];
    h[..name.len()].copy_from_slice(name.as_bytes());
    put_octal(&mut h[100..108], 0o644);
    put_octal(&mut h[108..116], u64::from(uid));
    put_octal(&mut h[116..124], u64::from(gid));
    put_octal(&mut h[124..136], size);
    put_octal(&mut h[136..148], mtime);
    h[148..156].copy_from_slice(b"        ");
    h[156] = b'0';
    h[257..263].copy_from_slice(b"ustar\0");
    h[263..265].copy_from_slice(b"00");
    let sum: u32 = h.iter().map(|b| u32::from(*b)).sum();
    let check = format!("{sum:06o}\0 ");
    h[148..156].copy_from_slice(check.as_bytes());
    h
}
