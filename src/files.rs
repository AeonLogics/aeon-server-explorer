// Plain Axum routes for moving file contents in and out of volumes: these stream, which server
// functions can't do.

use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use futures::StreamExt;
use serde::Deserialize;
use shared::volumes::{join_path, MAX_FILE_UPLOAD};

use crate::middleware::CurrentUser;
use crate::state::AppState;

// Like image uploads: a type a cross-site form can't send.
const FILE_TYPE: &str = "application/x-explorer-file";

fn admin_of(req_user: Option<&CurrentUser>) -> Result<shared::admin::Admin, Response> {
    match req_user.and_then(|u| u.0.clone()) {
        None => Err(StatusCode::UNAUTHORIZED.into_response()),
        Some(admin) if admin.must_change_password => Err(StatusCode::FORBIDDEN.into_response()),
        Some(admin) => Ok(admin),
    }
}

#[derive(Deserialize)]
pub struct DownloadQuery {
    stack: String,
    storage: String,
    #[serde(default)]
    path: String,
    // "file" (default): the file itself; "all": the whole folder as a .tar.
    #[serde(default)]
    kind: String,
    // Open in the browser (images, text) instead of saving.
    #[serde(default)]
    inline: bool,
}

pub async fn download(
    State(state): State<AppState>,
    Query(query): Query<DownloadQuery>,
    request: Request,
) -> Response {
    if let Err(denied) = admin_of(request.extensions().get::<CurrentUser>()) {
        return denied;
    }
    let volume = match state.stacks.volume_of(&query.stack, &query.storage) {
        Ok(v) => v,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };

    if query.kind == "all" {
        let name = if query.path.is_empty() {
            format!("{}.tar", query.storage)
        } else {
            format!("{}-{}.tar", query.storage, query.path.replace('/', "-"))
        };
        return match backend::volumes::download(&state.docker, &volume, &query.path).await {
            Ok(stream) => {
                let mut headers = HeaderMap::new();
                headers.insert(header::CONTENT_TYPE, HeaderValue::from_static("application/x-tar"));
                headers.insert(header::CONTENT_DISPOSITION, disposition("attachment", &name));
                (headers, Body::from_stream(stream)).into_response()
            }
            Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
        };
    }

    match backend::volumes::download_file(&state.docker, &volume, &query.path).await {
        Ok(bytes) => {
            let name = query.path.rsplit('/').next().unwrap_or("file").to_string();
            let mut headers = HeaderMap::new();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type(&name)));
            headers.insert(
                header::CONTENT_DISPOSITION,
                disposition(if query.inline { "inline" } else { "attachment" }, &name),
            );
            // A file opened in the browser must not run scripts in this app's origin.
            headers.insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("sandbox"));
            headers.insert("x-content-type-options", HeaderValue::from_static("nosniff"));
            (headers, bytes).into_response()
        }
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

#[derive(Deserialize)]
pub struct UploadQuery {
    stack: String,
    storage: String,
    #[serde(default)]
    dir: String,
    name: String,
}

pub async fn upload(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    Query(query): Query<UploadQuery>,
    request: Request,
) -> Response {
    let admin = match admin_of(request.extensions().get::<CurrentUser>()) {
        Ok(admin) => admin,
        Err(denied) => return denied,
    };
    let headers = request.headers();
    if headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok()) != Some(FILE_TYPE) {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Upload a file.").into_response();
    }
    let Some(size) = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok())
    else {
        return (StatusCode::LENGTH_REQUIRED, "The upload needs its size.").into_response();
    };
    if size > MAX_FILE_UPLOAD {
        return (StatusCode::PAYLOAD_TOO_LARGE, "Files can be up to 2 GB.").into_response();
    }
    let volume = match state.stacks.volume_of(&query.stack, &query.storage) {
        Ok(v) => v,
        Err(e) => return (StatusCode::NOT_FOUND, e.to_string()).into_response(),
    };
    let user_agent = headers.get(header::USER_AGENT).and_then(|v| v.to_str().ok()).map(str::to_string);

    let body = Body::into_data_stream(request.into_body()).map(|chunk| chunk.map_err(std::io::Error::other));
    let result = backend::volumes::upload(&state.docker, &volume, &query.dir, &query.name, size, body).await;

    crate::api::audit::record_direct(
        &state.db,
        "volume.upload",
        Some(admin.email),
        Some(format!("{volume}: {} ({:.1} MB)", join_path(&query.dir, &query.name), size as f64 / 1_048_576.0)),
        result.as_ref().err().map(|e| e.to_string()),
        result.is_ok(),
        Some(addr.ip().to_string()),
        user_agent,
    )
    .await;

    match result {
        Ok(()) => StatusCode::OK.into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}

fn disposition(kind: &str, name: &str) -> HeaderValue {
    let ascii: String = name.chars().map(|c| if c.is_ascii_graphic() && c != '"' && c != '\\' { c } else { '_' }).collect();
    HeaderValue::from_str(&format!("{kind}; filename=\"{ascii}\"")).unwrap_or_else(|_| HeaderValue::from_static("attachment"))
}

fn content_type(name: &str) -> &'static str {
    let ext = name.rsplit('.').next().unwrap_or_default().to_ascii_lowercase();
    match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "avif" => "image/avif",
        "txt" | "log" | "md" | "env" | "conf" | "toml" | "yml" | "yaml" | "surql" | "sql" | "csv" => "text/plain; charset=utf-8",
        "json" => "application/json",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}
