use leptos::prelude::*;
use shared::docker::AuditEntry;
use shared::errors::docker::DockerError;

#[cfg(feature = "ssr")]
use super::queries::AuditQuery;
#[cfg(feature = "ssr")]
use crate::helpers::request::client_meta;
#[cfg(feature = "ssr")]
use crate::state::AppState;
#[cfg(feature = "ssr")]
use shared::errors::InternalServerError;

// A failed audit write is logged loudly but never hides what the action itself did.
#[cfg(feature = "ssr")]
pub(crate) async fn record(
    action: &str,
    actor: Option<String>,
    target: Option<String>,
    detail: Option<String>,
    ok: bool,
) {
    let Ok(state) = AppState::get() else { return };
    let client = client_meta().await;
    let detail = detail.map(|d| d.chars().take(500).collect::<String>());
    let target = target.map(|t| t.chars().take(300).collect::<String>());

    let result = state
        .db
        .query(AuditQuery::Record)
        .bind(("action", action.to_string()))
        .bind(("actor", actor))
        .bind(("target", target))
        .bind(("detail", detail))
        .bind(("ok", ok))
        .bind(("user_ip", client.ip))
        .bind(("user_agent", client.user_agent))
        .await
        .and_then(|res| res.check());

    if let Err(e) = result {
        let _ = InternalServerError::new(format!("audit write failed for {action}: {e}"));
    }
}

#[server(endpoint = "poll/audit")]
pub async fn get_audit() -> Result<Vec<AuditEntry>, DockerError> {
    super::docker::require_admin()?;
    let db = AppState::get()?.db;

    // 0 SELECT
    let entries = db
        .query(AuditQuery::List)
        .await
        .and_then(|res| res.check())
        .and_then(|mut res| res.take::<Vec<AuditEntry>>(0))
        .map_err(InternalServerError::new)?;

    Ok(entries
        .into_iter()
        .map(|e| AuditEntry {
            at: backend::docker::local_time(&e.at),
            ..e
        })
        .collect())
}
