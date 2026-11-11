use leptos::prelude::*;
use shared::docker::{Action, AuditEntry, Health, Overview, ProjectSummary};
use shared::stack::{StackState, StackSummary};

use super::common::{CardSkeletons, DockerProblem};
use super::gate::OverviewData;
use crate::api::audit::get_audit;
use crate::api::docker::ContainerAction;
use crate::api::stacks::get_stacks;
use crate::components::icon::Ico;
use crate::components::poll::Tick;
use crate::components::stats::bytes;

const ACTIVITY: usize = 5;

#[component]
pub fn OverviewPage() -> impl IntoView {
    let overview = OverviewData::get();
    let tick = Tick::signal();
    let start = ServerAction::<ContainerAction>::new();
    let activity = Resource::new(move || tick.get(), |_| get_audit());
    let stacks = Resource::new(move || tick.get(), |_| get_stacks());

    // A started container should show up right away, not on the next 5-second tick.
    Effect::new(move |_| {
        if matches!(start.value().get(), Some(Ok(()))) {
            tick.update(|t| *t = t.wrapping_add(1));
        }
    });

    view! {
        <div class="page page--board">
            <Transition fallback=|| view! { <CardSkeletons count=3 /> }>
                {move || {
                    overview
                        .get()
                        .map(|result| match result {
                            Ok(data) => {
                                let pending: Vec<StackSummary> = stacks
                                    .get()
                                    .and_then(Result::ok)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .filter(|s| s.state == StackState::Pending)
                                    .collect();
                                let recent: Vec<AuditEntry> = activity
                                    .get()
                                    .and_then(Result::ok)
                                    .unwrap_or_default()
                                    .into_iter()
                                    .take(ACTIVITY)
                                    .collect();
                                view! { <Board data pending recent start /> }.into_any()
                            }
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn Board(
    data: Overview,
    pending: Vec<StackSummary>,
    recent: Vec<AuditEntry>,
    start: ServerAction<ContainerAction>,
) -> impl IntoView {
    let total: u32 = data.projects.iter().map(|p| p.total).sum();
    let running: u32 = data.projects.iter().map(|p| p.running).sum();
    let unhealthy: u32 = data.projects.iter().map(|p| p.unhealthy).sum();
    let exposed: u32 = data.projects.iter().map(|p| p.exposed_ports).sum();
    let stopped = total - running;
    let compose = data.projects.iter().filter(|p| !p.standalone).count();

    let facts = [
        Some("This server".to_string()),
        data.engine_version.as_ref().map(|v| format!("Docker {v}")),
        data.cpus.map(|n| format!("{n} core{}", if n == 1 { "" } else { "s" })),
        data.memory.map(|m| bytes(m as f64)),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>()
    .join(" · ");

    let (lead, rest) = headline(total, stopped, unhealthy);
    let tone = if stopped > 0 || unhealthy > 0 { "is-warn" } else { "is-ok" };
    let subline = subline(total, stopped, unhealthy, exposed);
    let needs = needs_you(&data.projects, &pending);

    view! {
        <header class="board-head">
            <div class="board-head__text">
                <span class="label">{facts}</span>
                <h1 class="board-head__title">
                    <span class=format!("board-head__lead {tone}")>{lead}</span>
                    {rest}
                </h1>
                <p class="board-head__sub">{subline}</p>
            </div>
            <dl class="board-head__numbers">
                <div><dt>"Running"</dt><dd>{running}<small>{format!(" / {total}")}</small></dd></div>
                <div><dt>"Projects"</dt><dd>{compose}</dd></div>
                {data.images.map(|n| view! { <div><dt>"Images"</dt><dd>{n}</dd></div> })}
            </dl>
        </header>

        {(total > 0).then(|| view! { <Strip projects=data.projects.clone() /> })}

        <div class="board-grid">
            <section class="board-card" aria-label="Needs you">
                <h2 class="label">"Needs you"</h2>
                {if needs.is_empty() {
                    view! { <p class="board-card__calm">"Nothing needs you right now."</p> }.into_any()
                } else {
                    view! {
                        <ul class="needs">
                            {needs.into_iter().map(|n| view! { <NeedRow need=n start /> }).collect_view()}
                        </ul>
                    }
                    .into_any()
                }}
            </section>

            <section class="board-card" aria-label="Recent activity">
                <h2 class="label">"Recent activity"</h2>
                {if recent.is_empty() {
                    view! { <p class="board-card__calm">"No activity yet."</p> }.into_any()
                } else {
                    view! {
                        <ol class="timeline">
                            {recent
                                .into_iter()
                                .map(|e| {
                                    let when = e.at.get(5..16).unwrap_or(e.at.as_str()).to_string();
                                    let what = e.describe();
                                    let target = e.target.clone().unwrap_or_default();
                                    view! {
                                        <li class:is-failed=e.failed()>
                                            <span class="timeline__what">{what}</span>
                                            <span class="timeline__meta">{format!("{when}  {target}")}</span>
                                        </li>
                                    }
                                })
                                .collect_view()}
                        </ol>
                        <a class="board-card__more" href="/audit">"Full audit log →"</a>
                    }
                    .into_any()
                }}
            </section>
        </div>

        {(!data.projects.is_empty()).then(|| view! {
            <h2 class="label board-label">"Projects"</h2>
            <div class="board-projects">
                {data.projects.into_iter().map(|project| view! { <ProjectTile project /> }).collect_view()}
            </div>
        })}
    }
}

fn plural(n: u32, one: &str, many: &str) -> String {
    if n == 1 { format!("1 {one}") } else { format!("{n} {many}") }
}

// The big sentence: the most important fact first, in plain words.
fn headline(total: u32, stopped: u32, unhealthy: u32) -> (String, String) {
    if total == 0 {
        ("No containers".into(), " on this server yet.".into())
    } else if stopped > 0 {
        let verb = if stopped == 1 { " is stopped." } else { " are stopped." };
        (plural(stopped, "container", "containers"), verb.into())
    } else if unhealthy > 0 {
        let verb = if unhealthy == 1 { " is unhealthy." } else { " are unhealthy." };
        (plural(unhealthy, "container", "containers"), verb.into())
    } else {
        ("Everything".into(), " is running.".into())
    }
}

fn subline(total: u32, stopped: u32, unhealthy: u32, exposed: u32) -> String {
    if total == 0 {
        return "Create a stack, or start something with docker compose up -d.".into();
    }
    let health = match unhealthy {
        0 => "Nothing is unhealthy.".to_string(),
        n if stopped > 0 => format!("{} failing health checks too.", plural(n, "container is", "containers are")),
        n => format!("{} failing health checks.", plural(n, "container is", "containers are")),
    };
    let ports = match exposed {
        0 => "Every port stays on localhost.".to_string(),
        1 => "One port is open beyond localhost.".to_string(),
        n => format!("{n} ports are open beyond localhost."),
    };
    format!("{health} {ports}")
}

#[derive(Clone)]
enum Need {
    Stopped { project: String, slug: String, member: String, container: String, since: String },
    ManyStopped { project: String, slug: String, names: Vec<String> },
    Unhealthy { slug: String, member: String, container: String },
    Exposed { slug: String, member: String, container: String, port: String },
    Pending { stack: String },
}

fn needs_you(projects: &[ProjectSummary], pending: &[StackSummary]) -> Vec<Need> {
    let mut out = Vec::new();
    for p in projects {
        let stopped: Vec<_> = p.members.iter().filter(|m| !m.running).collect();
        match stopped.as_slice() {
            [] => {}
            [one] => out.push(Need::Stopped {
                project: p.name.clone(),
                slug: p.slug.clone(),
                member: one.name.clone(),
                container: one.container.clone(),
                since: one.status_text.clone(),
            }),
            many => out.push(Need::ManyStopped {
                project: p.name.clone(),
                slug: p.slug.clone(),
                names: many.iter().map(|m| m.name.clone()).collect(),
            }),
        }
    }
    for p in projects {
        for m in p.members.iter().filter(|m| m.running && m.health == Health::Down) {
            out.push(Need::Unhealthy { slug: p.slug.clone(), member: m.name.clone(), container: m.container.clone() });
        }
    }
    for p in projects {
        for m in &p.members {
            for port in &m.exposed {
                out.push(Need::Exposed {
                    slug: p.slug.clone(),
                    member: m.name.clone(),
                    container: m.container.clone(),
                    port: port.clone(),
                });
            }
        }
    }
    for s in pending {
        out.push(Need::Pending { stack: s.name.clone() });
    }
    out
}

#[component]
fn NeedRow(need: Need, start: ServerAction<ContainerAction>) -> impl IntoView {
    let service_href = |slug: &str, container: &str| format!("/p/{slug}/s/{container}");
    match need {
        Need::Stopped { project, slug, member, container, since } => {
            let target = container.clone();
            view! {
                <li class="need">
                    <span class="dot is-down"></span>
                    <a class="need__text" href=service_href(&slug, &container)>
                        <strong>{format!("{project} · {member} is stopped")}</strong>
                        <span>{since}</span>
                    </a>
                    <button
                        type="button"
                        class="btn btn--primary btn--small"
                        disabled=move || start.pending().get()
                        on:click=move |_| {
                            start.dispatch(ContainerAction { container: target.clone(), action: Action::Start, confirm: None });
                        }
                    >
                        {move || if start.pending().get() { "Starting…" } else { "Start" }}
                    </button>
                </li>
            }
            .into_any()
        }
        Need::ManyStopped { project, slug, names } => view! {
            <li class="need">
                <span class="dot is-down"></span>
                <a class="need__text" href=format!("/p/{slug}")>
                    <strong>{format!("{project} · {} stopped", names.len())}</strong>
                    <span>{names.join(", ")}</span>
                </a>
                <a class="act" href=format!("/p/{slug}")>"Open"</a>
            </li>
        }
        .into_any(),
        Need::Unhealthy { slug, member, container } => view! {
            <li class="need">
                <span class="dot is-down"></span>
                <a class="need__text" href=service_href(&slug, &container)>
                    <strong>{format!("{member} is failing its health check")}</strong>
                    <span>"Open its logs to see why"</span>
                </a>
                <a class="act" href=service_href(&slug, &container)>"View"</a>
            </li>
        }
        .into_any(),
        Need::Exposed { slug, member, container, port } => view! {
            <li class="need">
                <span class="dot is-warn"></span>
                <a class="need__text" href=service_href(&slug, &container)>
                    <strong>{format!("{member} is open to the network")}</strong>
                    <span class="need__mono">{port}</span>
                </a>
                <a class="act" href=service_href(&slug, &container)>"View"</a>
            </li>
        }
        .into_any(),
        Need::Pending { stack } => view! {
            <li class="need">
                <span class="dot is-warn"></span>
                <a class="need__text" href=format!("/stacks/{stack}")>
                    <strong>{format!("{stack} has changes that aren't live")}</strong>
                    <span>"Saved in the stack, not applied yet"</span>
                </a>
                <a class="act" href=format!("/stacks/{stack}")>"Apply"</a>
            </li>
        }
        .into_any(),
    }
}

// One square per container on the server, grouped by project: the whole machine at a glance.
#[component]
fn Strip(projects: Vec<ProjectSummary>) -> impl IntoView {
    view! {
        <section class="strip" aria-label="Every container">
            <div class="strip__groups">
                {projects
                    .into_iter()
                    .map(|p| {
                        let slug = p.slug.clone();
                        view! {
                            <div class="strip__group">
                                <div class="strip__squares">
                                    {p.members
                                        .into_iter()
                                        .map(|m| {
                                            let href = format!("/p/{slug}/s/{}", m.container);
                                            view! {
                                                <a
                                                    class=format!("square {}", square_class(m.health, m.running))
                                                    href=href
                                                    title=format!("{} · {}", m.name, m.status_text)
                                                    aria-label=format!("{}: {}", m.name, m.health_label)
                                                ></a>
                                            }
                                        })
                                        .collect_view()}
                                </div>
                                <a class="strip__name" class:is-standalone=p.standalone href=format!("/p/{}", p.slug)>
                                    {p.name}
                                </a>
                            </div>
                        }
                    })
                    .collect_view()}
            </div>
            <div class="strip__legend" aria-hidden="true">
                <span><i class="square is-ok"></i>"running"</span>
                <span><i class="square is-warn"></i>"starting / paused"</span>
                <span><i class="square is-down"></i>"stopped or failing"</span>
            </div>
        </section>
    }
}

fn square_class(health: Health, running: bool) -> &'static str {
    match (health, running) {
        (Health::Ok, _) => "is-ok",
        (Health::Warn, _) => "is-warn",
        (Health::Down, _) => "is-down",
    }
}

#[component]
fn ProjectTile(project: ProjectSummary) -> impl IntoView {
    let href = format!("/p/{}", project.slug);
    let services = project.members.iter().map(|m| m.name.clone()).collect::<Vec<_>>().join(" · ");
    view! {
        <a class="tile-project" href=href>
            <div class="tile-project__head">
                <span class="tile-project__name" class:is-standalone=project.standalone>{project.name.clone()}</span>
                <span class="tile-project__count">{format!("{} / {}", project.running, project.total)}</span>
            </div>
            <div class="tile-project__squares" aria-hidden="true">
                {project
                    .members
                    .iter()
                    .map(|m| view! { <i class=format!("square square--small {}", square_class(m.health, m.running))></i> })
                    .collect_view()}
            </div>
            <span class="tile-project__services">{services}</span>
            <Ico icon=icondata::LuChevronRight class="tile-project__go" />
        </a>
    }
}
