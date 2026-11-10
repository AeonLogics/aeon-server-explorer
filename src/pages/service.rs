use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use shared::docker::{Action, ServiceDetail};

use super::common::DockerProblem;
use super::project::kind_style;
use crate::api::docker::get_service;
use crate::components::actions::ActionBar;
use crate::components::chips::{NetworkChip, PortChip};
use crate::components::icon::Ico;
use crate::components::logs::LogViewer;
use crate::components::poll::{route_parts, watch_session, Tick};
use crate::components::stats::StatsStrip;
use crate::components::status::HealthBadge;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Logs,
    Details,
    Env,
    Mounts,
}

#[component]
pub fn ServicePanel() -> impl IntoView {
    let location = use_location();
    let tick = Tick::signal();
    let route = Memo::new(move |_| route_parts(&location.pathname.get()));
    let slug = Memo::new(move |_| route.get().0.unwrap_or_default());
    let container = Memo::new(move |_| route.get().1.unwrap_or_default());
    let close_href = move || format!("/p/{}", slug.get());

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

    let navigate = use_navigate();
    Effect::new(move |_| {
        let navigate = navigate.clone();
        let handle = window_event_listener(leptos::ev::keydown, move |ev| {
            if ev.key() != "Escape" {
                return;
            }
            if dialog.get_untracked().is_some() {
                dialog.set(None);
            } else {
                navigate(&format!("/p/{}", slug.get_untracked()), NavigateOptions::default());
            }
        });
        on_cleanup(move || handle.remove());
    });

    let loaded = move || detail.get().and_then(Result::ok);

    view! {
        <a class="panel-scrim" href=close_href aria-label="Close panel"></a>
        <aside class="panel" aria-label="Service details">
            <header class="panel__head">
                <Transition fallback=|| view! { <div class="skeleton skeleton--title"></div> }>
                    {move || {
                        detail
                            .get()
                            .map(|result| match result {
                                Ok(d) => view! { <PanelTitle detail=d /> }.into_any(),
                                Err(_) => view! { <h2 class="panel__title">{container.get()}</h2> }.into_any(),
                            })
                    }}
                </Transition>
                <a class="icon-btn" href=close_href aria-label="Close" title="Close (Esc)">
                    <Ico icon=icondata::LuX />
                </a>
            </header>

            <ActionBar detail=Signal::derive(loaded) dialog on_done />

            {move || {
                epoch.track();
                let c = container.get();
                view! { <StatsStrip container=c /> }
            }}

            <div class="tabs" role="tablist">
                <TabButton tab which=Tab::Logs label="Logs" icon=icondata::LuSquareTerminal />
                <TabButton tab which=Tab::Details label="Details" icon=icondata::LuListFilter />
                <TabButton
                    tab
                    which=Tab::Env
                    label="Environment"
                    icon=icondata::LuVariable
                    count=Signal::derive(move || loaded().map(|d| d.env_names.len()))
                />
                <TabButton
                    tab
                    which=Tab::Mounts
                    label="Mounts"
                    icon=icondata::LuHardDrive
                    count=Signal::derive(move || loaded().map(|d| d.mounts.len()))
                />
            </div>

            <div class="panel__body">
                <div class="panel__logs" class:is-hidden=move || tab.get() != Tab::Logs>
                    {move || {
                        epoch.track();
                        let c = container.get();
                        let reconnect = Callback::new(move |_: ()| epoch.update(|e| *e += 1));
                        view! { <LogViewer container=c reconnect /> }
                    }}
                </div>

                <Show when=move || tab.get() != Tab::Logs>
                    <div class="panel__scroll">
                        <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                            {move || {
                                detail
                                    .get()
                                    .map(|result| match result {
                                        Ok(d) => match tab.get() {
                                            Tab::Details => view! { <DetailsTab detail=d /> }.into_any(),
                                            Tab::Env => view! { <EnvTab names=d.env_names /> }.into_any(),
                                            Tab::Mounts => view! { <MountsTab detail=d /> }.into_any(),
                                            Tab::Logs => ().into_any(),
                                        },
                                        Err(error) => view! { <DockerProblem error /> }.into_any(),
                                    })
                            }}
                        </Transition>
                    </div>
                </Show>
            </div>
        </aside>
    }
}

#[component]
fn TabButton(
    tab: RwSignal<Tab>,
    which: Tab,
    label: &'static str,
    icon: icondata::Icon,
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
            <Ico icon=icon />
            {label}
            {count.map(|count| view! {
                <span class="tabs__count">{move || count.get().map(|n| n.to_string()).unwrap_or_default()}</span>
            })}
        </button>
    }
}

#[component]
fn PanelTitle(detail: ServiceDetail) -> impl IntoView {
    let card = detail.card;
    let (icon, class) = kind_style(card.kind);
    view! {
        <div class="panel__titles">
            <span class=format!("svc__icon {class}")><Ico icon=icon /></span>
            <div>
                <h2 class="panel__title">{card.name}</h2>
                <span class="panel__sub">{card.image}</span>
            </div>
            <HealthBadge health=card.health label=card.health_label />
        </div>
    }
}

#[component]
fn DetailsTab(detail: ServiceDetail) -> impl IntoView {
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
        ("Container", Some(card.container_name.clone()), true),
        ("ID", Some(short_id), true),
        ("Image", Some(card.image.clone()), true),
        ("Image ID", image_id, true),
        ("State", Some(card.status_text.clone()), false),
        ("Created", detail.created.clone(), false),
        ("Restarts", Some(detail.restart_count.to_string()), false),
        ("Restart policy", detail.restart_policy.clone(), true),
        ("Command", detail.command.clone(), true),
    ];

    view! {
        <section class="block">
            <h3 class="block__title">"Container"</h3>
            <dl class="kv">
                {uptime.map(|(k, v)| view! { <dt>{k}</dt><dd>{v}</dd> })}
                {rows
                    .into_iter()
                    .filter_map(|(k, v, mono)| v.map(|v| (k, v, mono)))
                    .map(|(k, v, mono)| view! { <dt>{k}</dt><dd class:is-mono=mono>{v}</dd> })
                    .collect_view()}
            </dl>
        </section>

        {detail.project.clone().map(|project| view! {
            <section class="block">
                <h3 class="block__title">"Compose"</h3>
                <dl class="kv">
                    <dt>"Project"</dt><dd class="is-mono">{project}</dd>
                    {detail.working_dir.clone().map(|d| view! { <dt>"Folder"</dt><dd class="is-mono">{d}</dd> })}
                    {detail.config_files.clone().map(|f| view! { <dt>"Files"</dt><dd class="is-mono">{f}</dd> })}
                </dl>
            </section>
        })}

        <section class="block">
            <h3 class="block__title">"Ports"</h3>
            <div class="chips">
                {if card.ports.is_empty() {
                    view! { <span class="chips__none">"No ports exposed"</span> }.into_any()
                } else {
                    card.ports.into_iter().map(|port| view! { <PortChip port /> }).collect_view().into_any()
                }}
            </div>
        </section>

        <section class="block">
            <h3 class="block__title">"Networks"</h3>
            <ul class="rows">
                {detail
                    .networks
                    .into_iter()
                    .map(|n| {
                        let aliases = n.aliases.join(", ");
                        view! {
                            <li class="row">
                                <NetworkChip name=n.name />
                                <span class="row__mono">{n.ip.unwrap_or_else(|| "no IP".into())}</span>
                                <span class="row__faint" title="DNS names other services can use">{aliases}</span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
}

#[component]
fn EnvTab(names: Vec<String>) -> impl IntoView {
    let count = names.len();
    view! {
        <section class="block">
            <p class="note">
                <Ico icon=icondata::LuLock />
                "Only variable names are shown. Values never leave the server."
            </p>
            <h3 class="block__title">{format!("{count} variables")}</h3>
            <ul class="env">
                {names.into_iter().map(|name| view! { <li><code>{name}</code><span>"••••••"</span></li> }).collect_view()}
            </ul>
        </section>
    }
}

#[component]
fn MountsTab(detail: ServiceDetail) -> impl IntoView {
    if detail.mounts.is_empty() {
        return view! {
            <div class="empty-state empty-state--small">
                <span class="empty-state__icon"><Ico icon=icondata::LuHardDrive /></span>
                <h2>"No mounts"</h2>
                <p>"This container keeps nothing outside its own filesystem."</p>
            </div>
        }
        .into_any();
    }

    view! {
        <section class="block">
            <ul class="mounts">
                {detail
                    .mounts
                    .into_iter()
                    .map(|m| {
                        view! {
                            <li class="mount">
                                <span class=format!("mount__kind is-{}", m.kind)>{m.kind.clone()}</span>
                                <div class="mount__paths">
                                    <code class="mount__src" title=m.source.clone()>{m.source.clone()}</code>
                                    <span class="mount__arrow">"→"</span>
                                    <code class="mount__dst">{m.destination}</code>
                                </div>
                                <span class="mount__mode" class:is-ro=m.read_only>
                                    {if m.read_only { "read-only" } else { "read-write" }}
                                </span>
                            </li>
                        }
                    })
                    .collect_view()}
            </ul>
        </section>
    }
    .into_any()
}
