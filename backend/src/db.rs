use surrealdb::Surreal;
use surrealdb::engine::any::{self, Any};
use surrealdb::opt::auth::Root;

pub type Db = Surreal<Any>;

pub const DEFAULT_URI: &str = "surrealkv://data/explorer.db";
const NAMESPACE: &str = "explorer";
const DATABASE: &str = "main";

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("could not open SurrealDB at `{uri}`: {reason}")]
    Connect { uri: String, reason: String },
    #[error("SurrealDB sign-in failed: {0}")]
    SignIn(String),
    #[error("could not select namespace/database: {0}")]
    Select(String),
}

pub async fn connect(uri: &str) -> Result<Db, DbError> {
    let db = any::connect(uri).await.map_err(|e| DbError::Connect {
        uri: uri.to_string(),
        reason: e.to_string(),
    })?;

    // Embedded engines have no users; a remote server needs SURREAL_USER / SURREAL_PASS.
    let remote = uri.starts_with("ws") || uri.starts_with("http");
    if remote {
        let (Ok(username), Ok(password)) = (std::env::var("SURREAL_USER"), std::env::var("SURREAL_PASS"))
        else {
            return Err(DbError::SignIn("set SURREAL_USER and SURREAL_PASS for a remote server".into()));
        };
        db.signin(Root { username, password })
            .await
            .map_err(|e| DbError::SignIn(e.to_string()))?;
    }

    db.use_ns(NAMESPACE)
        .use_db(DATABASE)
        .await
        .map_err(|e| DbError::Select(e.to_string()))?;

    Ok(db)
}
