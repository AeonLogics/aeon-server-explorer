use std::net::SocketAddr;

use axum::body::Body;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use futures::StreamExt;
use shared::images::MAX_UPLOAD;

use crate::middleware::CurrentUser;
use crate::state::AppState;

// The browser must send this exact type. A plain cross-site form can't, and fetch from another
// site would need a CORS preflight this server never grants.
const ARCHIVE_TYPE: &str = "application/x-docker-archive";

pub async fn load_image(
    State(state): State<AppState>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    request: Request,
) -> Response {
    let admin = match request.extensions().get::<CurrentUser>().and_then(|u| u.0.clone()) {
        None => return StatusCode::UNAUTHORIZED.into_response(),
        Some(admin) if admin.must_change_password => return StatusCode::FORBIDDEN.into_response(),
        Some(admin) => admin,
    };
    let headers = request.headers();
    let content_type = headers.get(header::CONTENT_TYPE).and_then(|v| v.to_str().ok());
    if content_type != Some(ARCHIVE_TYPE) {
        return (StatusCode::UNSUPPORTED_MEDIA_TYPE, "Upload an image archive.").into_response();
    }
    let declared: Option<u64> = headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse().ok());
    if declared.is_some_and(|n| n > MAX_UPLOAD) {
        return (StatusCode::PAYLOAD_TOO_LARGE, "That file is over the 8 GB limit.").into_response();
    }
    let file_name = headers
        .get("x-file-name")
        .and_then(|v| v.to_str().ok())
        .map(|n| n.chars().filter(|c| !c.is_control()).take(120).collect::<String>())
        .unwrap_or_default();
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let mut received: u64 = 0;
    let archive = Body::into_data_stream(request.into_body()).map(move |chunk| {
        let chunk = chunk.map_err(std::io::Error::other)?;
        received += chunk.len() as u64;
        if received > MAX_UPLOAD {
            return Err(std::io::Error::other("upload is over the size limit"));
        }
        Ok(chunk)
    });

    let result = backend::images::load(&state.docker, archive).await;

    let size = declared.map(|n| format!(" ({:.0} MB)", n as f64 / 1_048_576.0)).unwrap_or_default();
    let target = match &result {
        Ok(done) => done.images.iter().map(|i| i.reference.as_str()).collect::<Vec<_>>().join(", "),
        Err(_) => file_name.clone(),
    };
    crate::api::audit::record_direct(
        &state.db,
        "image.load",
        Some(admin.email),
        Some(format!("{target}{size}")),
        result.as_ref().err().map(|e| e.to_string()),
        result.is_ok(),
        Some(addr.ip().to_string()),
        user_agent,
    )
    .await;

    match result {
        Ok(done) => Json(done).into_response(),
        Err(e) => (StatusCode::UNPROCESSABLE_ENTITY, e.to_string()).into_response(),
    }
}
