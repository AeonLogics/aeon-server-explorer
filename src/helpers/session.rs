use axum::http::{header, request::Parts, HeaderMap, HeaderValue};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use leptos::prelude::*;
use leptos_axum::ResponseOptions;
use shared::errors::InternalServerError;

pub const COOKIE_NAME: &str = "__Host-explorer_session";

// Must match `session.expires_at` DEFAULT (time::now() + 12h).
const MAX_AGE_SECS: u64 = 12 * 60 * 60;

pub fn new_token() -> String {
    URL_SAFE_NO_PAD.encode(rand::random::<[u8; 32]>())
}

pub fn session_cookie(token: &str) -> Result<HeaderValue, InternalServerError> {
    HeaderValue::from_str(&format!(
        "{COOKIE_NAME}={token}; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age={MAX_AGE_SECS}"
    ))
    .map_err(InternalServerError::new)
}

pub fn expired_cookie() -> HeaderValue {
    HeaderValue::from_static(
        "__Host-explorer_session=; Path=/; HttpOnly; Secure; SameSite=Strict; Max-Age=0",
    )
}

pub fn set_cookie(token: &str) -> Result<(), InternalServerError> {
    response_options()?.append_header(header::SET_COOKIE, session_cookie(token)?);
    Ok(())
}

pub fn clear_cookie() -> Result<(), InternalServerError> {
    response_options()?.append_header(header::SET_COOKIE, expired_cookie());
    Ok(())
}

pub fn read_cookie() -> Option<String> {
    let parts = use_context::<Parts>()?;
    read_cookie_from(&parts.headers)
}

pub fn read_cookie_from(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE_NAME)
        .map(|(_, value)| value.to_owned())
        .filter(|value| !value.is_empty())
}

fn response_options() -> Result<ResponseOptions, InternalServerError> {
    use_context::<ResponseOptions>()
        .ok_or_else(|| InternalServerError::new("ResponseOptions missing from context"))
}
