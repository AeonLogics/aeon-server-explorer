use backend::db::Db;

use crate::api::queries::{AuthQuery, SCHEMA};

pub async fn apply_schema(db: &Db) -> Result<(), String> {
    for (name, surql) in SCHEMA {
        db.query(*surql)
            .await
            .and_then(|res| res.check())
            .map_err(|e| format!("schema `{name}` failed: {e}"))?;
    }
    Ok(())
}

// The admin account is created once, from ADMIN_EMAIL / ADMIN_PASSWORD, and must change its
// password on first login. After that the env vars are ignored.
pub async fn ensure_admin(db: &Db) -> Result<(), String> {
    // 0 RETURN exists
    let exists = db
        .query(AuthQuery::AdminExists)
        .await
        .and_then(|res| res.check())
        .and_then(|mut res| res.take::<Option<bool>>(0))
        .map_err(|e| format!("could not check for the admin account: {e}"))?
        .unwrap_or(false);
    if exists {
        return Ok(());
    }

    let env = |key: &str| std::env::var(key).ok().filter(|v| !v.trim().is_empty());
    let (Some(email), Some(password)) = (env("ADMIN_EMAIL"), env("ADMIN_PASSWORD")) else {
        return Err(
            "no admin account yet: set ADMIN_EMAIL and ADMIN_PASSWORD (in .env) for the first start"
                .into(),
        );
    };

    db.query(AuthQuery::SeedAdmin)
        .bind(("email", email.trim().to_lowercase()))
        .bind(("password", password))
        .await
        .and_then(|res| res.check())
        .map_err(|e| format!("could not create the admin account (is ADMIN_EMAIL a valid email?): {e}"))?;

    println!("created the admin account from ADMIN_EMAIL; you must change its password on first login");
    Ok(())
}
