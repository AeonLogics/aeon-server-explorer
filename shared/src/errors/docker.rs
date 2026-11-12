use serde::{Deserialize, Serialize};
use server_fn::{
    codec::JsonEncoding,
    error::{FromServerFnError, ServerFnErrorErr},
};

use super::InternalServerError;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, thiserror::Error)]
pub enum DockerError {
    #[error("Your session has ended. Sign in again.")]
    Unauthorized,
    #[error("Change your password first.")]
    PasswordChangeRequired,
    #[error("Docker isn't reachable. Is Docker running?")]
    Unavailable,
    #[error("That container doesn't exist anymore.")]
    NotFound,
    #[error("{0}")]
    Failed(String),
    #[error(transparent)]
    Internal(#[from] InternalServerError),
    #[error("Connection error {0}")]
    Transport(ServerFnErrorErr),
}

impl DockerError {
    pub fn ends_session(&self) -> bool {
        matches!(self, Self::Unauthorized | Self::PasswordChangeRequired)
    }
}

impl FromServerFnError for DockerError {
    type Encoder = JsonEncoding;
    fn from_server_fn_error(value: ServerFnErrorErr) -> Self {
        DockerError::Transport(value)
    }
}
