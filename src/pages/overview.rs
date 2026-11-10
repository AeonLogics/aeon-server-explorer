use leptos::prelude::*;
use shared::docker::{Health, Overview, ProjectSummary};

use super::common::{CardSkeletons, DockerProblem};
use super::gate::OverviewData;
use crate::components::icon::Ico;
use crate::components::poll::POLL_SECS;
use crate::components::status::StatusDot;

#[component]
pub fn OverviewPage() -> impl IntoView {
    let overview = OverviewData::get();

    view! {
        <div class="page">
            <header class="page__head">
                <div class="page__titles">
                    <span class="eyebrow"><span class="eyebrow__dot"></span>"OVERVIEW"</span>
                    <h1 class="page__title">"Your server, at a glance"</h1>
                    <p class="page__lead">"Every compose project and container Docker knows about. Read-only for now."</p>
                </div>
                <RefreshPill />
            </header>

            <Transition fallback=|| view! { <CardSkeletons count=6 /> }>
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
pub fn RefreshPill() -> impl IntoView {
    view! {
        <span class="refresh-pill" title="Lists refresh on their own; logs and stats are live">
            <span class="dot is-ok dot--pulse"></span>
            {format!("Auto-refresh · {POLL_SECS}s")}
        </span>
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
                <span class="empty-state__icon"><Ico icon=icondata::LuBoxes /></span>
                <h2>"No containers yet"</h2>
                <p>"Start something with "<code>"docker compose up -d"</code>" and it shows up here within a few seconds."</p>
            </div>
        }
        .into_any();
    }

    view! {
        <section class="tiles" aria-label="Summary">
            <Tile
                icon=icondata::LuActivity
                label="RUNNING"
                value=format!("{running}")
                sub=format!("of {total} containers")
                tone=if running == total { Health::Ok } else { Health::Warn }
                meter=Some(if total == 0 { 0.0 } else { running as f64 / total as f64 })
            />
            <Tile
                icon=icondata::LuLayers
                label="PROJECTS"
                value=format!("{compose}")
                sub="compose projects".to_string()
                tone=Health::Ok
                meter=None
            />
            <Tile
                icon=icondata::LuHeartPulse
                label="UNHEALTHY"
                value=format!("{unhealthy}")
                sub=if unhealthy == 0 { "all checks passing".to_string() } else { "failing health checks".to_string() }
                tone=if unhealthy == 0 { Health::Ok } else { Health::Down }
                meter=None
            />
            <Tile
                icon=icondata::LuTriangleAlert
                label="EXPOSED PORTS"
                value=format!("{exposed}")
                sub=if exposed == 0 { "everything on localhost".to_string() } else { "published beyond localhost".to_string() }
                tone=if exposed == 0 { Health::Ok } else { Health::Warn }
                meter=None
            />
        </section>

        <div class="section-head">
            <h2>"Projects"</h2>
            <span class="label">{format!("{} GROUPS", data.projects.len())}</span>
        </div>
        <div class="grid">
            {data.projects.into_iter().map(|project| view! { <ProjectCard project /> }).collect_view()}
        </div>
    }
    .into_any()
}

#[component]
fn Tile(
    icon: icondata::Icon,
    label: &'static str,
    value: String,
    sub: String,
    tone: Health,
    meter: Option<f64>,
) -> impl IntoView {
    view! {
        <div class=format!("tile {}", tone.class())>
            <div class="tile__head">
                <span class="tile__icon"><Ico icon=icon /></span>
                <span class="label">{label}</span>
            </div>
            <strong class="tile__value">{value}</strong>
            <span class="tile__sub">{sub}</span>
            {meter.map(|m| view! {
                <span class="tile__meter"><span style=format!("width: {:.1}%", m * 100.0)></span></span>
            })}
        </div>
    }
}

#[component]
fn ProjectCard(project: ProjectSummary) -> impl IntoView {
    let href = format!("/p/{}", project.slug);
    let ratio = if project.total == 0 { 0.0 } else { project.running as f64 / project.total as f64 };
    let icon = if project.standalone { icondata::LuBox } else { icondata::LuFolderOpen };

    view! {
        <a class=format!("project-card {}", project.health.class()) href=href>
            <div class="project-card__head">
                <span class="project-card__icon"><Ico icon=icon /></span>
                <span class="project-card__name">{project.name.clone()}</span>
                <StatusDot health=project.health pulse=project.health == Health::Ok />
            </div>
            <div class="project-card__meter" aria-hidden="true">
                <span style=format!("width: {:.1}%", ratio * 100.0)></span>
            </div>
            <div class="project-card__foot">
                <span>{format!("{} of {} running", project.running, project.total)}</span>
                <span class="project-card__flags">
                    {(project.unhealthy > 0).then(|| view! {
                        <span class="flag is-down" title="Failing health checks">
                            <Ico icon=icondata::LuHeartPulse />{project.unhealthy}
                        </span>
                    })}
                    {(project.exposed_ports > 0).then(|| view! {
                        <span class="flag is-warn" title="Ports published beyond localhost">
                            <Ico icon=icondata::LuTriangleAlert />{project.exposed_ports}
                        </span>
                    })}
                    <Ico icon=icondata::LuChevronRight class="project-card__go" />
                </span>
            </div>
        </a>
    }
}
