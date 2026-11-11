use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use shared::docker::{ProjectDetail, ServiceCard as Card, ServiceKind};

use super::common::{CardSkeletons, DockerProblem};
use crate::api::docker::get_project;
use crate::api::stacks::is_managed;
use crate::components::chips::{NetworkChip, PortChip};
use crate::components::icon::Ico;
use crate::components::poll::{route_parts, watch_session, Tick};
use crate::components::status::HealthBadge;

#[component]
pub fn ProjectPage() -> impl IntoView {
    let location = use_location();
    let tick = Tick::signal();
    let slug = Memo::new(move |_| route_parts(&location.pathname.get()).0.unwrap_or_default());

    let project = Resource::new(move || (slug.get(), tick.get()), |(slug, _)| get_project(slug));
    let managed = Resource::new(move || slug.get(), is_managed);
    watch_session(move || project.get().and_then(Result::err));

    // A stack this app manages has one home: its workbench, where it's edited and run.
    let navigate = use_navigate();
    Effect::new(move |_| {
        if matches!(managed.get(), Some(Ok(true))) {
            navigate(
                &format!("/stacks/{}", slug.get_untracked()),
                NavigateOptions { replace: true, ..Default::default() },
            );
        }
    });

    view! {
        <div class="page">
            <Transition fallback=|| view! { <CardSkeletons count=4 /> }>
                {move || {
                    project
                        .get()
                        .map(|result| match result {
                            Ok(detail) => {
                                let managed = managed.get().and_then(Result::ok).unwrap_or(false);
                                view! { <ProjectBody detail managed /> }.into_any()
                            }
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn ProjectBody(detail: ProjectDetail, managed: bool) -> impl IntoView {
    let summary = detail.summary;
    let slug = summary.slug.clone();

    view! {
        <nav class="crumbs" aria-label="Breadcrumb">
            <a href="/">"Overview"</a>
            <span aria-hidden="true">"/"</span>
            <span>{summary.name.clone()}</span>
        </nav>

        <header class="page__head">
            <div class="page__titles">
                <h1 class="page__title">{summary.name.clone()}</h1>
                <p class="page__meta">
                    <span>{format!("{} of {} running", summary.running, summary.total)}</span>
                    {detail.working_dir.map(|dir| view! {
                        <span class="page__meta-mono" title="Compose working directory">{dir}</span>
                    })}
                </p>
            </div>
            {managed.then(|| view! {
                <a class="btn btn--secondary" href=format!("/stacks/{slug}")>"Edit stack"</a>
            })}
            {(summary.exposed_ports > 0).then(|| view! {
                <p class="notice notice--warn">
                    <Ico icon=icondata::LuTriangleAlert />
                    {format!(
                        "{} port{} published beyond localhost",
                        summary.exposed_ports,
                        if summary.exposed_ports == 1 { "" } else { "s" }
                    )}
                </p>
            })}
        </header>

        <div class="grid grid--services">
            {detail
                .services
                .into_iter()
                .map(|card| view! { <ServiceCard card slug=slug.clone() /> })
                .collect_view()}
        </div>
    }
}

#[component]
fn ServiceCard(card: Card, slug: String) -> impl IntoView {
    let href = format!("/p/{slug}/s/{}", card.container_name);
    let (kind_icon, kind_label) = kind_style(card.kind);

    view! {
        <a class=format!("svc {}", card.health.class()) href=href>
            <div class="svc__head">
                <span class="svc__kind" title=kind_label><Ico icon=kind_icon /></span>
                <span class="svc__titles">
                    <strong class="svc__name">{card.name.clone()}</strong>
                    <span class="svc__container">{card.container_name.clone()}</span>
                </span>
                <HealthBadge health=card.health label=card.health_label.clone() />
            </div>

            <dl class="svc__facts">
                <dt>"Image"</dt>
                <dd class="is-mono" title=card.image.clone()>{card.image.clone()}</dd>
                <dt>"Status"</dt>
                <dd>{card.status_text.clone()}</dd>
                <dt>"Ports"</dt>
                <dd>
                    {if card.ports.is_empty() {
                        view! { <span class="svc__none">"none"</span> }.into_any()
                    } else {
                        view! {
                            <span class="chips">
                                {card.ports.into_iter().map(|port| view! { <PortChip port /> }).collect_view()}
                            </span>
                        }
                        .into_any()
                    }}
                </dd>
                <dt>"Networks"</dt>
                <dd>
                    {if card.networks.is_empty() {
                        view! { <span class="svc__none">"none"</span> }.into_any()
                    } else {
                        view! {
                            <span class="chips">
                                {card.networks.into_iter().map(|name| view! { <NetworkChip name /> }).collect_view()}
                            </span>
                        }
                        .into_any()
                    }}
                </dd>
            </dl>
        </a>
    }
}

pub fn kind_style(kind: ServiceKind) -> (icondata::Icon, &'static str) {
    match kind {
        ServiceKind::Database => (icondata::LuDatabase, "Database"),
        ServiceKind::Proxy => (icondata::LuGlobe, "Proxy"),
        ServiceKind::App => (icondata::LuBox, "App"),
    }
}
