use leptos::prelude::*;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_location;
use shared::docker::{ProjectDetail, ServiceCard as Card, ServiceKind};

use super::common::{CardSkeletons, DockerProblem};
use super::overview::RefreshPill;
use crate::api::docker::get_project;
use crate::components::chips::{NetworkChip, PortChip};
use crate::components::icon::Ico;
use crate::components::poll::{route_parts, watch_session, Tick};
use crate::components::status::{HealthBadge, StatusDot};

#[component]
pub fn ProjectPage() -> impl IntoView {
    let location = use_location();
    let tick = Tick::signal();
    let route = Memo::new(move |_| route_parts(&location.pathname.get()));
    let slug = Memo::new(move |_| route.get().0.unwrap_or_default());
    let selected = Memo::new(move |_| route.get().1);

    let project = Resource::new(move || (slug.get(), tick.get()), |(slug, _)| get_project(slug));
    watch_session(move || project.get().and_then(Result::err));

    view! {
        <div class="page page--project" class:has-panel=move || selected.get().is_some()>
            <Transition fallback=|| view! { <CardSkeletons count=4 /> }>
                {move || {
                    project
                        .get()
                        .map(|result| match result {
                            Ok(detail) => view! { <ProjectBody detail selected /> }.into_any(),
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                        })
                }}
            </Transition>
            <Outlet />
        </div>
    }
}

#[component]
fn ProjectBody(detail: ProjectDetail, selected: Memo<Option<String>>) -> impl IntoView {
    let summary = detail.summary;
    let slug = summary.slug.clone();

    view! {
        <header class="page__head">
            <div class="page__titles">
                <nav class="crumbs" aria-label="Breadcrumb">
                    <a href="/">"Overview"</a>
                    <Ico icon=icondata::LuChevronRight />
                    <span>"Project"</span>
                </nav>
                <h1 class="page__title">
                    <StatusDot health=summary.health pulse=true />
                    {summary.name.clone()}
                </h1>
                <div class="page__meta">
                    <span class="meta-pill">
                        <Ico icon=icondata::LuActivity />
                        {format!("{} of {} running", summary.running, summary.total)}
                    </span>
                    {detail.working_dir.map(|dir| view! {
                        <span class="meta-pill meta-pill--mono" title="Compose working directory">
                            <Ico icon=icondata::LuFolder />{dir}
                        </span>
                    })}
                    {(summary.exposed_ports > 0).then(|| view! {
                        <span class="meta-pill is-warn">
                            <Ico icon=icondata::LuTriangleAlert />
                            {format!("{} port(s) beyond localhost", summary.exposed_ports)}
                        </span>
                    })}
                </div>
            </div>
            <RefreshPill />
        </header>

        <div class="grid grid--services">
            {detail
                .services
                .into_iter()
                .map(|card| {
                    let name = card.container_name.clone();
                    let active = Signal::derive(move || selected.get().as_deref() == Some(name.as_str()));
                    view! { <ServiceCard card slug=slug.clone() active /> }
                })
                .collect_view()}
        </div>
    }
}

#[component]
fn ServiceCard(card: Card, slug: String, active: Signal<bool>) -> impl IntoView {
    let href = format!("/p/{slug}/s/{}", card.container_name);
    let (kind_icon, kind_class) = kind_style(card.kind);

    view! {
        <a
            class=format!("svc {}", card.health.class())
            class:is-active=move || active.get()
            href=href
            aria-current=move || active.get().then_some("page")
        >
            <div class="svc__head">
                <span class=format!("svc__icon {kind_class}")><Ico icon=kind_icon /></span>
                <span class="svc__titles">
                    <strong class="svc__name">{card.name.clone()}</strong>
                    <span class="svc__container">{card.container_name.clone()}</span>
                </span>
                <HealthBadge health=card.health label=card.health_label.clone() />
            </div>

            <div class="svc__image" title=card.image.clone()>
                <Ico icon=icondata::LuPackage />
                <span>{card.image.clone()}</span>
            </div>

            <div class="svc__status">
                <Ico icon=icondata::LuClock />
                <span>{card.status_text.clone()}</span>
            </div>

            <div class="svc__group">
                <span class="label">"PORTS"</span>
                <div class="chips">
                    {if card.ports.is_empty() {
                        view! { <span class="chips__none">"none"</span> }.into_any()
                    } else {
                        card.ports.into_iter().map(|port| view! { <PortChip port /> }).collect_view().into_any()
                    }}
                </div>
            </div>

            <div class="svc__group">
                <span class="label">"NETWORKS"</span>
                <div class="chips">
                    {if card.networks.is_empty() {
                        view! { <span class="chips__none">"none"</span> }.into_any()
                    } else {
                        card.networks.into_iter().map(|name| view! { <NetworkChip name /> }).collect_view().into_any()
                    }}
                </div>
            </div>
        </a>
    }
}

pub fn kind_style(kind: ServiceKind) -> (icondata::Icon, &'static str) {
    match kind {
        ServiceKind::Database => (icondata::LuDatabase, "is-db"),
        ServiceKind::Proxy => (icondata::LuGlobe, "is-proxy"),
        ServiceKind::App => (icondata::LuBox, "is-app"),
    }
}
