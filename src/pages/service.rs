use leptos::prelude::*;
use leptos_router::hooks::use_location;
use shared::docker::{Action, ServiceDetail};

use super::common::DockerProblem;
use crate::api::docker::get_service;
use crate::components::actions::ActionBar;
use crate::components::chips::{NetworkChip, PortChip};
use crate::components::logs::LogViewer;
use crate::components::poll::{route_parts, watch_session, Tick};
use crate::components::stats::StatsStrip;
use crate::components::status::HealthBadge;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Logs,
    Env,
    Mounts,
}

#[component]
pub fn ServicePage() -> impl IntoView {
    let location = use_location();
    let tick = Tick::signal();
    let route = Memo::new(move |_| route_parts(&location.pathname.get()));
    let slug = Memo::new(move |_| route.get().0.unwrap_or_default());
    let container = Memo::new(move |_| route.get().1.unwrap_or_default());

    let detail = Resource::new(move || (container.get(), tick.get()), |(c, _)| get_service(c));
    watch_session(move || detail.get().and_then(Result::err));

    let tab = RwSignal::new(Tab::Logs);
    let dialog = RwSignal::new(None::<Action>);
    // Bumped after an action, so logs and stats reconnect to the (possibly new) container.
    let epoch = RwSignal::new(0u32);
    let on_done = Callback::new(move |_: Action| {
        epoch.update(|e| *e += 1);
        tick.update(|t| *t = t.wrapping_add(1));
    });

    Effect::new(move |_| {
        let handle = window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.key() == "Escape" && dialog.get_untracked().is_some() {
                dialog.set(None);
            }
        });
        on_cleanup(move || handle.remove());
    });

    let loaded = move || detail.get().and_then(Result::ok);
    let project_name = move || {
        loaded()
            .and_then(|d| d.project)
            .unwrap_or_else(|| if slug.get() == shared::docker::STANDALONE_SLUG {
                shared::docker::STANDALONE.to_string()
            } else {
                slug.get()
            })
    };

    view! {
        <div class="page page--service">
            <nav class="crumbs" aria-label="Breadcrumb">
                <a href="/">"Overview"</a>
                <span aria-hidden="true">"/"</span>
                <a href=move || format!("/p/{}", slug.get())>{project_name}</a>
                <span aria-hidden="true">"/"</span>
                <span>{move || container.get()}</span>
            </nav>

            <header class="service-head">
                <Transition fallback=|| view! { <div class="skeleton skeleton--title"></div> }>
                    {move || {
                        detail
                            .get()
                            .map(|result| match result {
                                Ok(d) => view! { <ServiceTitle detail=d /> }.into_any(),
                                Err(_) => view! { <h1 class="page__title">{container.get()}</h1> }.into_any(),
                            })
                    }}
                </Transition>
                <ActionBar detail=Signal::derive(loaded) dialog on_done />
            </header>

            <div class="service-layout">
                <section class="service-main">
                    <div class="tabs" role="tablist">
                        <TabButton tab which=Tab::Logs label="Logs" />
                        <TabButton
                            tab
                            which=Tab::Env
                            label="Environment"
                            count=Signal::derive(move || loaded().map(|d| d.env_names.len()))
                        />
                        <TabButton
                            tab
                            which=Tab::Mounts
                            label="Mounts"
                            count=Signal::derive(move || loaded().map(|d| d.mounts.len()))
                        />
                    </div>

                    <div class="service-main__logs" class:is-hidden=move || tab.get() != Tab::Logs>
                        {move || {
                            epoch.track();
                            let c = container.get();
                            let reconnect = Callback::new(move |_: ()| epoch.update(|e| *e += 1));
                            view! { <LogViewer container=c reconnect /> }
                        }}
                    </div>

                    <Show when=move || tab.get() != Tab::Logs>
                        <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                            {move || {
                                detail
                                    .get()
                                    .map(|result| match result {
                                        Ok(d) => match tab.get() {
                                            Tab::Env => view! { <EnvTab names=d.env_names /> }.into_any(),
                                            Tab::Mounts => view! { <MountsTab detail=d /> }.into_any(),
                                            Tab::Logs => ().into_any(),
                                        },
                                        Err(error) => view! { <DockerProblem error /> }.into_any(),
                                    })
                            }}
                        </Transition>
                    </Show>
                </section>

                <aside class="service-side" aria-label="Facts">
                    {move || {
                        epoch.track();
                        let c = container.get();
                        view! { <StatsStrip container=c /> }
                    }}
                    <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                        {move || {
                            detail
                                .get()
                                .map(|result| match result {
                                    Ok(d) => view! { <Facts detail=d /> }.into_any(),
                                    Err(error) => view! { <DockerProblem error /> }.into_any(),
                                })
                        }}
                    </Transition>
                </aside>
            </div>
        </div>
    }
}

#[component]
fn TabButton(
    tab: RwSignal<Tab>,
    which: Tab,
    label: &'static str,
    #[prop(optional)] count: Option<Signal<Option<usize>>>,
) -> impl IntoView {
    let active = move || tab.get() == which;
    view! {
        <button
            type="button"
            role="tab"
            class="tabs__tab"
            class:is-active=active
            aria-selected=move || active().to_string()
            on:click=move |_| tab.set(which)
        >
            {label}
            {count.map(|count| view! {
                <span class="tabs__count">{move || count.get().map(|n| n.to_string()).unwrap_or_default()}</span>
            })}
        </button>
    }
}

#[component]
fn ServiceTitle(detail: ServiceDetail) -> impl IntoView {
    let card = detail.card;
    view! {
        <div class="service-head__titles">
            <div class="service-head__row">
                <h1 class="page__title">{card.name}</h1>
                <HealthBadge health=card.health label=card.health_label />
            </div>
            <span class="service-head__image">{card.image}</span>
        </div>
    }
}

#[component]
fn Facts(detail: ServiceDetail) -> impl IntoView {
    let card = detail.card.clone();
    let short_id: String = card.id.chars().take(12).collect();
    let image_id = detail
        .image_id
        .clone()
        .map(|id| id.trim_start_matches("sha256:").chars().take(12).collect::<String>());
    let uptime = match (&detail.started_ago, detail.exit_code) {
        (Some(ago), _) => Some(("Up for", ago.clone())),
        (None, Some(code)) => Some(("Exit code", code.to_string())),
        _ => None,
    };

    let rows: Vec<(&'static str, Option<String>, bool)> = vec![
        ("State", Some(card.status_text.clone()), false),
        ("Restarts", Some(detail.restart_count.to_string()), false),
        ("Restart policy", detail.restart_policy.clone(), true),
        ("Created", detail.created.clone(), false),
        ("Container", Some(card.container_name.clone()), true),
        ("ID", Some(short_id), true),
        ("Image ID", image_id, true),
        ("Command", detail.command.clone(), true),
    ];

    view! {
        <section class="facts">
            <h2 class="facts__title">"Container"</h2>
            <dl class="kv">
                {uptime.map(|(k, v)| view! { <dt>{k}</dt><dd>{v}</dd> })}
                {rows
                    .into_iter()
                    .filter_map(|(k, v, mono)| v.map(|v| (k, v, mono)))
                    .map(|(k, v, mono)| view! { <dt>{k}</dt><dd class:is-mono=mono>{v}</dd> })
                    .collect_view()}
            </dl>
        </section>

        <section class="facts">
            <h2 class="facts__title">"Ports"</h2>
            {if card.ports.is_empty() {
                view! { <p class="facts__none">"None published"</p> }.into_any()
            } else {
                view! {
                    <div class="chips">
                        {card.ports.into_iter().map(|port| view! { <PortChip port /> }).collect_view()}
                    </div>
                }
                .into_any()
            }}
        </section>

        <section class="facts">
            <h2 class="facts__title">"Networks"</h2>
            <ul class="facts__list">
                {detail
                    .networks
                    .into_iter()
                    .map(|n| {
                        let aliases = n.aliases.join(", ");
                        view! {
                            <li>
                                <NetworkChip name=n.name />
                                <span class="facts__mono">{n.ip.unwrap_or_else(|| "no IP".into())}</span>
                                {(!aliases.is_empty()).then(|| view! {
                                    <span class="facts__faint" title="Names other services can use">{aliases}</span>
                                })}
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>

        {detail.project.clone().map(|project| view! {
            <section class="facts">
                <h2 class="facts__title">"Compose"</h2>
                <dl class="kv">
                    <dt>"Project"</dt><dd class="is-mono">{project}</dd>
                    {detail.service.clone().map(|s| view! { <dt>"Service"</dt><dd class="is-mono">{s}</dd> })}
                    {detail.working_dir.clone().map(|d| view! { <dt>"Folder"</dt><dd class="is-mono">{d}</dd> })}
                    {detail.config_files.clone().map(|f| view! { <dt>"Files"</dt><dd class="is-mono">{f}</dd> })}
                </dl>
            </section>
        })}
    }
}

#[component]
fn EnvTab(names: Vec<String>) -> impl IntoView {
    if names.is_empty() {
        return view! { <p class="tab-empty">"This container has no environment variables."</p> }.into_any();
    }
    view! {
        <div class="tab-body">
            <p class="tab-note">"Names only. Values never leave the server."</p>
            <ul class="env">
                {names.into_iter().map(|name| view! { <li><code>{name}</code></li> }).collect_view()}
            </ul>
        </div>
    }
    .into_any()
}

#[component]
fn MountsTab(detail: ServiceDetail) -> impl IntoView {
    if detail.mounts.is_empty() {
        return view! { <p class="tab-empty">"No mounts. This container keeps nothing outside its own filesystem."</p> }
            .into_any();
    }

    view! {
        <div class="tab-body">
            <ul class="mounts">
                {detail
                    .mounts
                    .into_iter()
                    .map(|m| {
                        view! {
                            <li class="mount">
                                <span class="mount__kind">{m.kind.clone()}</span>
                                <code class="mount__src" title=m.source.clone()>{m.source.clone()}</code>
                                <span class="mount__arrow" aria-hidden="true">"→"</span>
                                <code class="mount__dst">{m.destination}</code>
                                <span class="mount__mode">{if m.read_only { "read-only" } else { "read-write" }}</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </div>
    }
    .into_any()
}
