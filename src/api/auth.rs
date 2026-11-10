use leptos::prelude::*;
use shared::{
    admin::Admin,
    errors::auth::AuthError,
    inputs::{ChangePasswordInput, LoginInput},
};

#[cfg(feature = "ssr")]
use shared::errors::InternalServerError;

#[cfg(feature = "ssr")]
use super::queries::AuthQuery;
#[cfg(feature = "ssr")]
use crate::helpers::{request::client_meta, session};
#[cfg(feature = "ssr")]
use crate::state::AppState;
#[cfg(feature = "ssr")]
use garde::Validate;

#[server]
pub async fn current_admin() -> Result<Option<Admin>, AuthError> {
    Ok(super::current_admin_from_context())
}

#[server]
pub async fn login_user(input: LoginInput) -> Result<Admin, AuthError> {
    let mut input = input;
    input.email = input.email.trim().to_lowercase();
    input.validate()?;

    let db = AppState::get()?.db;
    let client = client_meta().await;
    let session_token = session::new_token();

    // 0 LET $user, 1 LET $outcome, 2 CREATE audit, 3 RETURN $outcome, 4 RETURN admin
    let mut res = db
        .query(AuthQuery::LoginUser)
        .bind(("email", input.email))
        .bind(("password", input.password))
        .bind(("session_token", session_token.clone()))
        .bind(("user_ip", client.ip))
        .bind(("user_agent", client.user_agent))
        .await?
        .check()?;

    match res.take::<Option<String>>(3)?.as_deref() {
        Some("ok") => {}
        Some("locked" | "locked_now") => return Err(AuthError::Locked),
        Some("bad_credentials") => return Err(AuthError::BadCredentials),
        other => {
            return Err(
                InternalServerError::new(format!("unexpected login outcome: {other:?}")).into(),
            )
        }
    }

    let admin = res
        .take::<Option<Admin>>(4)?
        .ok_or_else(|| InternalServerError::new("login returned no admin"))?;

    session::set_cookie(&session_token)?;
    Ok(admin)
}

#[server]
pub async fn logout_user() -> Result<(), AuthError> {
    if let Some(token) = session::read_cookie() {
        let db = AppState::get()?.db;
        let client = client_meta().await;
        // 0 LET $current, 1 IF audit, 2 DELETE session
        db.query(AuthQuery::Logout)
            .bind(("session_token", token))
            .bind(("user_ip", client.ip))
            .bind(("user_agent", client.user_agent))
            .await?
            .check()?;
    }

    session::clear_cookie()?;
    Ok(())
}

#[server]
pub async fn change_password(input: ChangePasswordInput) -> Result<Admin, AuthError> {
    input.validate()?;
    let token = session::read_cookie().ok_or(AuthError::Unauthorized)?;
    let admin = super::current_admin_from_context().ok_or(AuthError::Unauthorized)?;

    let db = AppState::get()?.db;
    let client = client_meta().await;

    // 0 LET $current, 1 LET $user, 2 LET $outcome, 3 IF audit, 4 RETURN $outcome
    let mut res = db
        .query(AuthQuery::ChangePassword)
        .bind(("session_token", token))
        .bind(("current_password", input.current_password))
        .bind(("new_password", input.new_password))
        .bind(("user_ip", client.ip))
        .bind(("user_agent", client.user_agent))
        .await?
        .check()?;

    match res.take::<Option<String>>(4)?.as_deref() {
        Some("ok") => Ok(Admin {
            must_change_password: false,
            ..admin
        }),
        Some("wrong_current") => Err(AuthError::WrongCurrentPassword),
        Some("same") => Err(AuthError::SamePassword),
        Some("unauthorized") => Err(AuthError::Unauthorized),
        other => Err(InternalServerError::new(format!("unexpected outcome: {other:?}")).into()),
    }
}
