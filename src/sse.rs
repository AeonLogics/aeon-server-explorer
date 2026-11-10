use std::convert::Infallible;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use futures::{Stream, StreamExt};
use serde::Serialize;
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
