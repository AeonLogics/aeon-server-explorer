use garde::Validate;
use serde::{Deserialize, Serialize};

pub const MIN_PASSWORD: usize = 12;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct LoginInput {
    #[garde(email, length(max = 254))]
    pub email: String,

    #[garde(length(chars, min = 1, max = 128))]
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct ChangePasswordInput {
    #[garde(length(chars, min = 1, max = 128))]
    pub current_password: String,

    #[garde(length(chars, min = 12, max = 128))]
    pub new_password: String,
}
