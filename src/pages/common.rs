use leptos::prelude::*;
use shared::errors::docker::DockerError;

use crate::components::icon::Ico;

#[component]
pub fn NotFound() -> impl IntoView {
    view! {
        <div class="empty-state empty-state--page">
            <span class="empty-state__icon"><Ico icon=icondata::LuCompass /></span>
            <h1>"Nothing here"</h1>
            <p>"That page doesn't exist."</p>
            <a class="btn btn--secondary" href="/">"Back to overview"</a>
        </div>
    }
}

#[component]
pub fn DockerProblem(error: DockerError) -> impl IntoView {
    let (icon, title, text) = match &error {
        DockerError::Unavailable => (
            icondata::LuWifiOff,
            "Docker isn't reachable".to_string(),
            "Start Docker Desktop (or the Docker service on the server). This page checks again every 5 seconds.".to_string(),
        ),
        DockerError::NotFound => (
            icondata::LuSearchX,
            "Not found".to_string(),
            "It may have been removed or renamed. Pick another one from the sidebar.".to_string(),
        ),
        other => (icondata::LuCircleAlert, "Something went wrong".to_string(), other.to_string()),
    };
    view! {
        <div class="empty-state">
            <span class="empty-state__icon"><Ico icon=icon /></span>
            <h2>{title}</h2>
            <p>{text}</p>
        </div>
    }
}

#[component]
pub fn CardSkeletons(count: usize) -> impl IntoView {
    view! {
        <div class="grid" aria-hidden="true">
            {(0..count).map(|_| view! { <div class="skeleton skeleton--card"></div> }).collect_view()}
        </div>
    }
}
