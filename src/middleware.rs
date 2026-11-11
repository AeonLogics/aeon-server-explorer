use axum::extract::{Request, State};
use axum::http::{header, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use shared::admin::Admin;
use shared::errors::InternalServerError;

use crate::api::queries::AuthQuery;
use crate::helpers::session;
use crate::state::AppState;

#[derive(Debug, Clone, Default)]
pub struct CurrentUser(pub Option<Admin>);

// Blocks DNS rebinding: a page on another site can resolve its own name to 127.0.0.1, but it
// can't make the browser send `Host: localhost:<port>`.
pub async fn host_guard(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let host = req
        .headers()
        .get(header::HOST)
        .and_then(|h| h.to_str().ok())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if state.allowed_hosts.iter().any(|allowed| *allowed == host) {
        next.run(req).await
    } else {
        (StatusCode::FORBIDDEN, "Forbidden: open this app through localhost.").into_response()
    }
}

pub async fn auth_layer(State(state): State<AppState>, mut req: Request, next: Next) -> Response {
    let path = req.uri().path();
    if is_asset(path) {
        return next.run(req).await;
    }

    // Background refreshes and live streams must not keep an idle session alive.
    let touch = !BACKGROUND.iter().any(|prefix| path.starts_with(prefix));

    let lookup = match session::read_cookie_from(req.headers()) {
        Some(token) => Some(find_user(&state, token, touch).await),
        None => None,
    };

    // Only an answered "no such session" clears the cookie; a database error must not log anyone out.
    let stale_cookie = matches!(lookup, Some(Ok(None)));
    let user = lookup.and_then(Result::ok).flatten();

    req.extensions_mut().insert(CurrentUser(user));
    let mut response = next.run(req).await;

    if stale_cookie && !response.headers().contains_key(header::SET_COOKIE) {
        response
            .headers_mut()
            .append(header::SET_COOKIE, session::expired_cookie());
    }

    response
}

pub const BACKGROUND: &[&str] = &["/api/poll/", "/api/logs/", "/api/stats/"];

// 0 LET $current, 1 LET $user, 2 IF touch, 3 RETURN admin
async fn find_user(
    state: &AppState,
    token: String,
    touch: bool,
) -> Result<Option<Admin>, InternalServerError> {
    let result = async {
        state
            .db
            .query(AuthQuery::CurrentUser)
            .bind(("session_token", token))
            .bind(("touch", touch))
            .await?
            .check()?
            .take::<Option<Admin>>(3)
    }
    .await;

    result.map_err(InternalServerError::new)
}

fn is_asset(path: &str) -> bool {
    !path.starts_with("/api/")
        && (path.starts_with("/pkg/")
            || path.rsplit('/').next().is_some_and(|last| last.contains('.')))
}
