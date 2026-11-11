use leptos::prelude::*;
use shared::docker::Health;

#[component]
pub fn StatusDot(health: Health) -> impl IntoView {
    view! { <span class=format!("dot {}", health.class()) aria-hidden="true"></span> }
}

#[component]
pub fn HealthBadge(health: Health, label: String) -> impl IntoView {
    view! {
        <span class=format!("badge {}", health.class())>
            <StatusDot health />
            {label}
        </span>
    }
}
