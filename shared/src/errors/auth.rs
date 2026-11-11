use serde::{Deserialize, Serialize};
use server_fn::{
    codec::JsonEncoding,
    error::{FromServerFnError, ServerFnErrorErr},
};

use super::{FieldError, InternalServerError};

#[derive(Debug, Clone, Serialize, Deserialize, thiserror::Error)]
pub enum AuthError {
    #[error("{0}")]
    Invalid(FieldError),
    #[error("Invalid email or password.")]
    BadCredentials,
    #[error("Too many failed attempts. Try again in 15 minutes.")]
    Locked,
    #[error("Your current password is not correct.")]
    WrongCurrentPassword,
    #[error("Choose a password you haven't used here before.")]
    SamePassword,
    #[error("Your session has ended. Sign in again.")]
    Unauthorized,
    #[error(transparent)]
    Internal(#[from] InternalServerError),
    #[error("Connection error {0}")]
    Transport(ServerFnErrorErr),
}

impl FromServerFnError for AuthError {
    type Encoder = JsonEncoding;
    fn from_server_fn_error(value: ServerFnErrorErr) -> Self {
        AuthError::Transport(value)
    }
}

#[cfg(feature = "ssr")]
impl From<surrealdb::Error> for AuthError {
    #[track_caller]
    fn from(value: surrealdb::Error) -> Self {
        AuthError::Internal(InternalServerError::new(value))
    }
}

impl From<garde::Report> for AuthError {
    fn from(report: garde::Report) -> Self {
        report
            .iter()
            .find_map(|(path, _)| match path.to_string().as_str() {
                "email" => Some(FieldError::Email),
                "password" | "current_password" => Some(FieldError::Password),
                "new_password" => Some(FieldError::NewPassword),
                _ => None,
            })
            .map(AuthError::Invalid)
            .unwrap_or_else(|| {
                InternalServerError::new(format!("unmapped validation error: {report}")).into()
            })
    }
}
