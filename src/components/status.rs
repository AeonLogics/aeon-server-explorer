use leptos::prelude::*;
use shared::docker::Health;

use crate::components::icon::Ico;

#[component]
pub fn StatusDot(health: Health, #[prop(optional)] pulse: bool) -> impl IntoView {
    view! { <span class=format!("dot {}", health.class()) class:dot--pulse=pulse aria-hidden="true"></span> }
}

#[component]
pub fn HealthBadge(health: Health, label: String) -> impl IntoView {
    let icon = match health {
        Health::Ok => icondata::LuCircleCheck,
        Health::Warn => icondata::LuCircleAlert,
        Health::Down => icondata::LuCircleX,
    };
    view! {
        <span class=format!("badge {}", health.class())>
            <Ico icon=icon />
            {label}
        </span>
    }
}
