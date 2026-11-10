use leptos::prelude::*;
use shared::docker::{Health, Overview, ProjectSummary};

use super::common::{CardSkeletons, DockerProblem};
use super::gate::OverviewData;
use crate::components::icon::Ico;
use crate::components::status::StatusDot;

#[component]
pub fn OverviewPage() -> impl IntoView {
    let overview = OverviewData::get();

    view! {
        <div class="page">
            <header class="page__head">
                <div class="page__titles">
                    <h1 class="page__title">"Overview"</h1>
                    <p class="page__lead">"Every compose project and container on this server."</p>
                </div>
            </header>

            <Transition fallback=|| view! { <CardSkeletons count=3 /> }>
                {move || {
                    overview
                        .get()
                        .map(|result| match result {
                            Ok(data) => view! { <OverviewBody data /> }.into_any(),
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn OverviewBody(data: Overview) -> impl IntoView {
    let total: u32 = data.projects.iter().map(|p| p.total).sum();
    let running: u32 = data.projects.iter().map(|p| p.running).sum();
    let unhealthy: u32 = data.projects.iter().map(|p| p.unhealthy).sum();
    let exposed: u32 = data.projects.iter().map(|p| p.exposed_ports).sum();
    let compose = data.projects.iter().filter(|p| !p.standalone).count();

    if data.projects.is_empty() {
        return view! {
            <div class="empty-state">
                <h2>"No containers yet"</h2>
                <p>"Start something with "<code>"docker compose up -d"</code>" and it appears here within a few seconds."</p>
            </div>
        }
        .into_any();
    }

    view! {
        <section class="summary" aria-label="Summary">
            <Stat label="Running" value=format!("{running}") sub=format!("of {total} containers") tone=None />
            <Stat label="Projects" value=format!("{compose}") sub="compose projects".to_string() tone=None />
            <Stat
                label="Unhealthy"
                value=format!("{unhealthy}")
                sub=if unhealthy == 0 { "all checks passing".to_string() } else { "failing health checks".to_string() }
                tone=(unhealthy > 0).then_some(Health::Down)
            />
            <Stat
                label="Exposed ports"
                value=format!("{exposed}")
                sub=if exposed == 0 { "all on localhost".to_string() } else { "beyond localhost".to_string() }
                tone=(exposed > 0).then_some(Health::Warn)
            />
        </section>

        <section class="table" aria-label="Projects">
            <div class="table__row table__row--head" aria-hidden="true">
                <span>"Project"</span>
                <span>"Running"</span>
                <span>"Attention"</span>
                <span></span>
            </div>
            {data.projects.into_iter().map(|project| view! { <ProjectRow project /> }).collect_view()}
        </section>
    }
    .into_any()
}

#[component]
fn Stat(label: &'static str, value: String, sub: String, tone: Option<Health>) -> impl IntoView {
    view! {
        <div class=format!("stat {}", tone.map(Health::class).unwrap_or(""))>
            <span class="stat__label">{label}</span>
            <strong class="stat__value">{value}</strong>
            <span class="stat__sub">{sub}</span>
        </div>
    }
}

#[component]
fn ProjectRow(project: ProjectSummary) -> impl IntoView {
    let href = format!("/p/{}", project.slug);
    let mut attention = Vec::new();
    if project.running < project.total {
        attention.push(format!("{} stopped", project.total - project.running));
    }
    if project.unhealthy > 0 {
        attention.push(format!("{} unhealthy", project.unhealthy));
    }
    if project.exposed_ports > 0 {
        attention.push(format!("{} exposed port{}", project.exposed_ports, if project.exposed_ports == 1 { "" } else { "s" }));
    }
    let calm = attention.is_empty();

    view! {
        <a class="table__row" href=href>
            <span class="table__name">
                <StatusDot health=project.health />
                <span class:is-standalone=project.standalone>{project.name}</span>
            </span>
            <span class="table__mono">{format!("{} / {}", project.running, project.total)}</span>
            <span class="table__note" class:is-calm=calm>
                {if calm { "Nothing".to_string() } else { attention.join(" · ") }}
            </span>
            <span class="table__go"><Ico icon=icondata::LuChevronRight /></span>
        </a>
    }
}
