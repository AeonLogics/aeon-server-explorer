use leptos::prelude::*;
use leptos_router::hooks::use_location;
use shared::admin::Admin;
use shared::docker::{Health, ProjectSummary};
use shared::errors::docker::DockerError;

use super::gate::OverviewData;
use crate::components::auth::LogoutButton;
use crate::components::icon::Ico;
use crate::components::poll::{route_parts, settled};
use crate::components::status::{calm_projects, StatusDot};

#[component]
pub fn Sidebar(admin: Admin) -> impl IntoView {
    let overview = OverviewData::get();
    let location = use_location();
    let search = RwSignal::new(String::new());

    let current_slug = move || route_parts(&location.pathname.get()).0;
    let on_home = move || location.pathname.get() == "/";
    let on_audit = move || location.pathname.get() == "/audit";
    let on_stacks = move || location.pathname.get().starts_with("/stacks");
    let on_images = move || location.pathname.get() == "/images";

    let loaded = settled(move || overview.get().and_then(Result::ok).map(|o| calm_projects(o.projects)));
    let projects = move || loaded.get().unwrap_or_default();
    let filtered = move || {
        let needle = search.get().trim().to_lowercase();
        projects()
            .into_iter()
            .filter(|p| needle.is_empty() || p.name.to_lowercase().contains(&needle))
            .collect::<Vec<_>>()
    };

    let initial = admin
        .email
        .chars()
        .next()
        .map(|c| c.to_uppercase().to_string())
        .unwrap_or_default();

    view! {
        <aside class="sidebar" aria-label="Projects">
            <a class="brand" href="/">
                <span class="brand__mark">
                    <img src="/logo.svg" alt="" width="30" height="21" />
                </span>
                <span class="brand__text">
                    <strong>"Server Explorer"</strong>
                    <span class="label">"AEON LOGICS"</span>
                </span>
            </a>

            <a class="side-link" class:is-active=on_home href="/">
                <Ico icon=icondata::LuLayoutDashboard />
                "Overview"
            </a>
            <a class="side-link" class:is-active=on_stacks href="/stacks">
                <Ico icon=icondata::LuLayers />
                "Stacks"
            </a>
            <a class="side-link" class:is-active=on_images href="/images">
                <Ico icon=icondata::LuPackage />
                "Images"
            </a>
            <a class="side-link" class:is-active=on_audit href="/audit">
                <Ico icon=icondata::LuScrollText />
                "Audit log"
            </a>

            <div class="sidebar__section">
                <span class="label">"PROJECTS"</span>
                <Transition fallback=|| ()>
                    {move || {
                        let _ = overview.get();
                        view! { <span hidden></span> }
                    }}
                    <span class="sidebar__count">{move || projects().len()}</span>
                </Transition>
            </div>

            <label class="search">
                <Ico icon=icondata::LuSearch />
                <span class="sr-only">"Find a project"</span>
                <input
                    type="search"
                    placeholder="Find a project"
                    prop:value=move || search.get()
                    on:input=move |ev| search.set(event_target_value(&ev))
                />
            </label>

            <nav class="projects">
                <Transition fallback=|| view! { <ProjectSkeleton /> }>
                    {move || {
                        let _ = overview.get();
                        view! { <span hidden></span> }
                    }}
                    {move || {
                        let list = filtered();
                        if list.is_empty() && loaded.with(Option::is_some) {
                            let message = if search.get().trim().is_empty() {
                                "No containers yet."
                            } else {
                                "No project matches."
                            };
                            return view! { <p class="projects__empty">{message}</p> }.into_any();
                        }
                        list.into_iter()
                            .map(|project| {
                                let active = {
                                    let slug = project.slug.clone();
                                    move || current_slug().as_deref() == Some(slug.as_str())
                                };
                                view! { <ProjectLink project active=Signal::derive(active) /> }
                            })
                            .collect_view()
                            .into_any()
                    }}
                </Transition>
            </nav>

            <div class="sidebar__foot">
                <Transition fallback=|| ()>
                    <EngineStatus />
                </Transition>
                <div class="who">
                    <span class="who__avatar" aria-hidden="true">{initial}</span>
                    <span class="who__text">
                        <span class="label">"ADMIN"</span>
                        <span class="who__email" title=admin.email.clone()>{admin.email.clone()}</span>
                    </span>
                    <LogoutButton />
                </div>
            </div>
        </aside>
    }
}

#[component]
fn ProjectLink(project: ProjectSummary, active: Signal<bool>) -> impl IntoView {
    let href = format!("/p/{}", project.slug);
    let warn = project.exposed_ports > 0;
    view! {
        <a class="project-link" class:is-active=move || active.get() class:is-standalone=project.standalone href=href>
            <StatusDot health=project.health />
            <span class="project-link__name">{project.name}</span>
            <Show when=move || warn>
                <span class="project-link__warn" title="Has ports published beyond localhost">
                    <Ico icon=icondata::LuTriangleAlert />
                </span>
            </Show>
            <span class="project-link__count">{format!("{}/{}", project.running, project.total)}</span>
        </a>
    }
}

#[component]
fn EngineStatus() -> impl IntoView {
    let overview = OverviewData::get();
    move || {
        let (health, title, sub) = match overview.get() {
            Some(Ok(o)) => (
                Health::Ok,
                "Docker connected".to_string(),
                o.engine_version.map(|v| format!("Engine {v}")).unwrap_or_default(),
            ),
            Some(Err(DockerError::Unavailable)) => (
                Health::Down,
                "Docker offline".to_string(),
                "Retrying every 5 s".to_string(),
            ),
            Some(Err(_)) => (Health::Warn, "Docker error".to_string(), "See the page".to_string()),
            None => (Health::Warn, "Connecting…".to_string(), String::new()),
        };
        view! {
            <div class="engine">
                <span class="engine__icon"><Ico icon=icondata::LuServer /></span>
                <span class="engine__text">
                    <strong>{title}</strong>
                    <span>{sub}</span>
                </span>
                <StatusDot health />
            </div>
        }
    }
}

#[component]
fn ProjectSkeleton() -> impl IntoView {
    view! {
        <div class="skeleton-list" aria-hidden="true">
            <span class="skeleton"></span>
            <span class="skeleton"></span>
            <span class="skeleton"></span>
        </div>
    }
}
