pub mod audit;
pub mod auth;
pub mod docker;
pub mod images;
pub mod stacks;
pub mod volumes;

#[cfg(feature = "ssr")]
pub(crate) mod queries;

#[cfg(feature = "ssr")]
pub(crate) fn current_admin_from_context() -> Option<shared::admin::Admin> {
    use crate::middleware::CurrentUser;
    use axum::http::request::Parts;
    use leptos::prelude::use_context;

    use_context::<Parts>()
        .and_then(|parts| parts.extensions.get::<CurrentUser>().cloned())
        .and_then(|current| current.0)
}
