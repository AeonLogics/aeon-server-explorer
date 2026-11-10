use leptos::prelude::*;
use shared::errors::docker::DockerError;

use crate::components::auth::AuthState;

pub const POLL_SECS: u64 = 5;

// One shared clock for every background refresh on the page.
#[derive(Clone, Copy)]
pub struct Tick(pub RwSignal<u32>);

impl Tick {
    pub fn provide() -> RwSignal<u32> {
        let tick = RwSignal::new(0u32);
        Effect::new(move |_| {
            let handle = set_interval_with_handle(
                move || tick.update(|t| *t = t.wrapping_add(1)),
                std::time::Duration::from_secs(POLL_SECS),
            );
            if let Ok(handle) = handle {
                on_cleanup(move || handle.clear());
            }
        });
        provide_context(Self(tick));
        tick
    }

    pub fn signal() -> RwSignal<u32> {
        expect_context::<Self>().0
    }
}

// A refresh that says the session is gone sends the whole app back to the login screen.
pub fn watch_session(error: impl Fn() -> Option<DockerError> + 'static) {
    let auth = AuthState::get();
    Effect::new(move |_| {
        if error().is_some_and(|e| e.ends_session()) {
            auth.refresh();
        }
    });
}

pub fn route_parts(path: &str) -> (Option<String>, Option<String>) {
    let mut parts = path.trim_matches('/').split('/');
    match (parts.next(), parts.next(), parts.next(), parts.next()) {
        (Some("p"), Some(slug), Some("s"), Some(container)) if !slug.is_empty() => {
            (Some(slug.to_string()), Some(container.to_string()).filter(|c| !c.is_empty()))
        }
        (Some("p"), Some(slug), _, _) if !slug.is_empty() => (Some(slug.to_string()), None),
        _ => (None, None),
    }
}
