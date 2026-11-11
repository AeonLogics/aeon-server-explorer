use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use surrealdb::types::SurrealValue;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ssr", derive(SurrealValue))]
pub struct Admin {
    pub email: String,
    pub must_change_password: bool,
}
