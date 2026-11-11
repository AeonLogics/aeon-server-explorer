use std::net::SocketAddr;

use axum::extract::ConnectInfo;
use axum::http::{header, HeaderMap};
use leptos_axum::extract;

#[derive(Debug, Clone, Default)]
pub struct ClientMeta {
    pub ip: Option<String>,
    pub user_agent: Option<String>,
}

// The app only listens on loopback, so the socket address is the truth; no proxy headers.
pub async fn client_meta() -> ClientMeta {
    let headers = extract::<HeaderMap>().await.unwrap_or_default();

    let user_agent = headers
        .get(header::USER_AGENT)
        .and_then(|v| v.to_str().ok())
        .map(|ua| ua.chars().take(512).collect());

    let ip = extract::<ConnectInfo<SocketAddr>>()
        .await
        .ok()
        .map(|ConnectInfo(addr)| addr.ip().to_string());

    ClientMeta { ip, user_agent }
}
