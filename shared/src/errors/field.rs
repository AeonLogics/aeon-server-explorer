use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
pub enum FieldError {
    #[error("Enter a valid email address.")]
    Email,
    #[error("Enter your password.")]
    Password,
    #[error("Use 12–128 characters.")]
    NewPassword,
}
