use std::convert::Infallible;

use std::net::SocketAddr;

use axum::extract::{ConnectInfo, Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use futures::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use shared::images::{valid_repo, valid_tag};
use shared::docker::valid_container_ref;
use shared::errors::docker::DockerError;

use crate::middleware::CurrentUser;
use crate::state::AppState;

const LOG_TAIL: u32 = 300;

pub async fn logs(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(container): Path<String>,
) -> Response {
    if let Err(denied) = check(&user, &container) {
        return denied;
    }
    events(backend::docker::logs(state.docker, container, LOG_TAIL)).into_response()
}

pub async fn stats(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    Path(container): Path<String>,
) -> Response {
    if let Err(denied) = check(&user, &container) {
        return denied;
    }
    events(backend::docker::stats(state.docker, container)).into_response()
}

#[derive(Deserialize)]
pub struct PullQuery {
    image: String,
    tag: String,
}

// Streams a pull's progress; the pull itself finishes even if the tab is closed.
pub async fn pull(
    State(state): State<AppState>,
    Extension(user): Extension<CurrentUser>,
    ConnectInfo(addr): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Query(query): Query<PullQuery>,
) -> Response {
    let admin = match &user.0 {
        None => return StatusCode::UNAUTHORIZED.into_response(),
        Some(admin) if admin.must_change_password => return StatusCode::FORBIDDEN.into_response(),
        Some(admin) => admin.clone(),
    };
    let (image, tag) = (query.image.trim().to_string(), query.tag.trim().to_string());
    if !valid_repo(&image) || !valid_tag(&tag) {
        return StatusCode::BAD_REQUEST.into_response();
    }

    let db = state.db.clone();
    let target = format!("{image}:{tag}");
    let ip = Some(addr.ip().to_string());
    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);

    let stream = backend::images::pull(state.docker, image, tag).map(move |event| {
        if event.done {
            let (db, actor, target, ip, user_agent) =
                (db.clone(), admin.email.clone(), target.clone(), ip.clone(), user_agent.clone());
            let error = event.error.clone();
            tokio::spawn(async move {
                crate::api::audit::record_direct(
                    &db,
                    "image.pull",
                    Some(actor),
                    Some(target),
                    error.clone(),
                    error.is_none(),
                    ip,
                    user_agent,
                )
                .await;
            });
        }
        Ok::<Event, Infallible>(
            Event::default()
                .json_data(&event)
                .unwrap_or_else(|_| Event::default().comment("unserializable")),
        )
    });
    let stream = stream.chain(futures::stream::once(async { Ok(Event::default().event("end").data("end")) }));
    Sse::new(stream).keep_alive(KeepAlive::default()).into_response()
}

fn check(user: &CurrentUser, container: &str) -> Result<(), Response> {
    match &user.0 {
        None => Err(StatusCode::UNAUTHORIZED.into_response()),
        Some(admin) if admin.must_change_password => Err(StatusCode::FORBIDDEN.into_response()),
        Some(_) if !valid_container_ref(container) => Err(StatusCode::BAD_REQUEST.into_response()),
        Some(_) => Ok(()),
    }
}

// A final `end` event tells the browser not to reconnect (EventSource retries on its own).
fn events<T, S>(source: S) -> Sse<impl Stream<Item = Result<Event, Infallible>>>
where
    T: Serialize,
    S: Stream<Item = Result<T, DockerError>> + Send + 'static,
{
    let stream = source
        .map(|item| {
            let event = match item {
                Ok(value) => Event::default()
                    .json_data(&value)
                    .unwrap_or_else(|_| Event::default().comment("unserializable")),
                Err(e) => Event::default().event("failure").data(e.to_string()),
            };
            Ok(event)
        })
        .chain(futures::stream::once(async {
            Ok(Event::default().event("end").data("end"))
        }));

    Sse::new(stream).keep_alive(KeepAlive::default())
}
