use leptos::prelude::*;
use shared::docker::{Action, ServiceDetail};
use shared::errors::docker::DockerError;
use shared::stack::{
    join_command, parse_env_file, split_command, DependsSpec, EnvChange, MountSpec, PortSpec, Restart, ServiceSpec,
    ServiceStatus, StackSpec, StartCondition, VolumeSpec, DEFAULT_NETWORK,
};

use crate::api::docker::get_service;
use crate::api::stacks::{reveal_env, reveal_env_file, SaveStack};
use crate::components::actions::ActionBar;
use crate::components::chips::NetworkChip;
use crate::components::icon::Ico;
use crate::components::logs::LogViewer;
use crate::components::poll::Tick;
use crate::components::stats::StatsStrip;
use crate::components::status::HealthBadge;

const REVEAL_SECS: u64 = 30;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Ports,
    Storage,
    Env,
    Logs,
}

// Each row owns its signals, so typing in one field never redraws the list (and never
// loses focus); the list itself only changes when a row is added or removed.
#[derive(Clone, Copy)]
struct PortRow {
    id: u32,
    container: RwSignal<String>,
    published: RwSignal<bool>,
    public: RwSignal<bool>,
    host: RwSignal<String>,
}

#[derive(Clone, Copy)]
struct MountRow {
    id: u32,
    storage: RwSignal<String>,
    path: RwSignal<String>,
    read_only: RwSignal<bool>,
}

#[derive(Clone, Copy)]
struct AfterRow {
    id: u32,
    service: RwSignal<String>,
    condition: RwSignal<StartCondition>,
}

#[derive(Clone, Copy)]
struct VolumeRow {
    id: u32,
    name: RwSignal<String>,
    path: RwSignal<String>,
}

#[derive(Clone, Copy)]
struct EnvRow {
    id: u32,
    name: RwSignal<String>,
    existing: bool,
    value: RwSignal<String>,
    editing: RwSignal<bool>,
    shown: RwSignal<Option<String>>,
}

// What the stack page needs from one service's editor: whether it has unsaved edits,
// a way to turn the form into a spec, and a way to load a spec back into the form.
#[derive(Clone, Copy)]
pub struct Editor {
    pub dirty: RwSignal<bool>,
    pub problem: RwSignal<Option<String>>,
    pub build: StoredValue<Option<Callback<(), Result<(ServiceSpec, Vec<EnvChange>), String>>>>,
    pub reload: StoredValue<Option<Callback<ServiceSpec>>>,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            dirty: RwSignal::new(false),
            problem: RwSignal::new(None),
            build: StoredValue::new(None),
            reload: StoredValue::new(None),
        }
    }
}

// One inspector per service stays mounted while you work on the stack, so switching
// services, saving and deploying never throw away what you typed or which tab is open.
#[component]
pub fn ServiceInspector(
    stack_name: String,
    initial: ServiceSpec,
    latest: Memo<Option<StackSpec>>,
    deployed: Signal<bool>,
    status: Signal<Option<ServiceStatus>>,
    active: Signal<bool>,
    save: ServerAction<SaveStack>,
    images: Resource<Result<Vec<String>, DockerError>>,
    editor: Editor,
) -> impl IntoView {
    let service = initial.name.clone();
    let dirty = editor.dirty;
    let problem = editor.problem;

    let tab = RwSignal::new(Tab::General);
    let next_id = StoredValue::new(0u32);
    let new_id = move || {
        next_id.update_value(|n| *n += 1);
        next_id.get_value()
    };
    let touch = move || dirty.set(true);

    let image = RwSignal::new(String::new());
    let restart = RwSignal::new(Restart::default());
    let command = RwSignal::new(String::new());
    let user = RwSignal::new(String::new());
    let run_once = RwSignal::new(false);
    let networks = RwSignal::new(Vec::<String>::new());
    let mounts = RwSignal::new(Vec::<MountRow>::new());
    let after = RwSignal::new(Vec::<AfterRow>::new());
    let ports = RwSignal::new(Vec::<PortRow>::new());
    let volumes = RwSignal::new(Vec::<VolumeRow>::new());
    let env = RwSignal::new(Vec::<EnvRow>::new());
    let env_text = RwSignal::new(None::<String>);
    let env_loading = RwSignal::new(false);

    let load = move |s: &ServiceSpec| {
        image.set(s.image.clone());
        restart.set(s.restart);
        command.set(join_command(&s.command));
        user.set(s.user.clone().unwrap_or_default());
        run_once.set(s.run_once);
        networks.set(s.on_networks());
        mounts.set(
            s.mounts
                .iter()
                .map(|m| MountRow {
                    id: new_id(),
                    storage: RwSignal::new(m.storage.clone()),
                    path: RwSignal::new(m.path.clone()),
                    read_only: RwSignal::new(m.read_only),
                })
                .collect(),
        );
        after.set(
            s.depends_on
                .iter()
                .map(|d| AfterRow { id: new_id(), service: RwSignal::new(d.service.clone()), condition: RwSignal::new(d.condition) })
                .collect(),
        );
        ports.set(
            s.ports
                .iter()
                .map(|p| PortRow {
                    id: new_id(),
                    container: RwSignal::new(p.container.to_string()),
                    published: RwSignal::new(p.host.is_some()),
                    public: RwSignal::new(p.public),
                    host: RwSignal::new(p.host.map(|h| h.to_string()).unwrap_or_default()),
                })
                .collect(),
        );
        volumes.set(
            s.volumes
                .iter()
                .map(|v| VolumeRow { id: new_id(), name: RwSignal::new(v.name.clone()), path: RwSignal::new(v.path.clone()) })
                .collect(),
        );
        env.set(
            s.env
                .iter()
                .map(|n| EnvRow {
                    id: new_id(),
                    name: RwSignal::new(n.clone()),
                    existing: true,
                    value: RwSignal::new(String::new()),
                    editing: RwSignal::new(false),
                    shown: RwSignal::new(None),
                })
                .collect(),
        );
        env_text.set(None);
    };
    load(&initial);

    let stack_name = StoredValue::new(stack_name);
    let service_name = StoredValue::new(service.clone());
    let default_container = format!("{}-{}-1", stack_name.get_value(), service);
    let container_name = Signal::derive(move || {
        status.get().map(|s| s.container).unwrap_or_else(|| default_container.clone())
    });
    let running = Signal::derive(move || status.with(|s| s.as_ref().is_some_and(|s| s.running)));

    // Live facts and the Start / Stop / Restart buttons come from Docker, refreshed with the
    // page, but only for the service you're looking at.
    let tick = Tick::signal();
    let detail = Resource::new(
        move || (active.get() && status.with(Option::is_some), container_name.get(), tick.get()),
        |(wanted, container, _)| async move {
            if wanted { get_service(container).await.ok() } else { None }
        },
    );
    let dialog = RwSignal::new(None::<Action>);
    let log_epoch = RwSignal::new(0u32);
    let on_done = Callback::new(move |_: Action| {
        tick.update(|t| *t = t.wrapping_add(1));
        log_epoch.update(|e| *e += 1);
    });

    let add_port = move |_| {
        touch();
        ports.update(|rows| {
            rows.push(PortRow {
                id: new_id(),
                container: RwSignal::new(String::new()),
                published: RwSignal::new(false),
                public: RwSignal::new(false),
                host: RwSignal::new(String::new()),
            })
        })
    };
    let add_mount = move |_| {
        touch();
        let first = latest.with_untracked(|s| s.as_ref().and_then(|s| s.storages.first()).map(|x| x.name.clone()));
        mounts.update(|rows| {
            rows.push(MountRow {
                id: new_id(),
                storage: RwSignal::new(first.unwrap_or_default()),
                path: RwSignal::new(String::new()),
                read_only: RwSignal::new(false),
            })
        })
    };
    let add_after = move |_| {
        touch();
        let me = service_name.get_value();
        let first = latest.with_untracked(|s| {
            s.as_ref().and_then(|s| s.services.iter().find(|x| x.name != me)).map(|x| x.name.clone())
        });
        after.update(|rows| {
            rows.push(AfterRow { id: new_id(), service: RwSignal::new(first.unwrap_or_default()), condition: RwSignal::new(StartCondition::Started) })
        })
    };
    let add_volume = move |_| {
        touch();
        volumes.update(|rows| rows.push(VolumeRow { id: new_id(), name: RwSignal::new(String::new()), path: RwSignal::new(String::new()) }))
    };
    let add_env = move |_| {
        touch();
        env.update(|rows| {
            rows.push(EnvRow {
                id: new_id(),
                name: RwSignal::new(String::new()),
                existing: false,
                value: RwSignal::new(String::new()),
                editing: RwSignal::new(true),
                shown: RwSignal::new(None),
            })
        })
    };

    // Rows left completely empty are ignored, so an extra "Add" click is harmless.
    let build = move || -> Result<(ServiceSpec, Vec<EnvChange>), String> {
        let name = service_name.get_value();
        let mut out_ports = Vec::new();
        for row in ports.get_untracked() {
            let container = row.container.get_untracked().trim().to_string();
            let host = row.host.get_untracked().trim().to_string();
            if container.is_empty() && host.is_empty() {
                continue;
            }
            let container: u16 = container
                .parse()
                .ok()
                .filter(|p| *p > 0)
                .ok_or("Container ports are numbers from 1 to 65535.")?;
            let host = if row.published.get_untracked() {
                Some(host.parse::<u16>().map_err(|_| "Server ports are numbers from 1024 to 65535.")?)
            } else {
                None
            };
            let public = host.is_some() && row.public.get_untracked();
            out_ports.push(PortSpec { container, host, public });
        }

        let mut out_volumes = Vec::new();
        for row in volumes.get_untracked() {
            let v_name = row.name.get_untracked().trim().to_lowercase();
            let path = row.path.get_untracked().trim().to_string();
            if v_name.is_empty() && path.is_empty() {
                continue;
            }
            out_volumes.push(VolumeSpec { name: v_name, path });
        }

        let mut out_mounts = Vec::new();
        for row in mounts.get_untracked() {
            let path = row.path.get_untracked().trim().to_string();
            let storage = row.storage.get_untracked();
            if path.is_empty() && storage.is_empty() {
                continue;
            }
            if storage.is_empty() {
                return Err("Pick which shared storage to mount.".into());
            }
            out_mounts.push(MountSpec { storage, path, read_only: row.read_only.get_untracked() });
        }

        let out_after: Vec<DependsSpec> = after
            .get_untracked()
            .into_iter()
            .filter(|r| !r.service.get_untracked().is_empty())
            .map(|r| DependsSpec { service: r.service.get_untracked(), condition: r.condition.get_untracked() })
            .collect();

        let mut out_networks = networks.get_untracked();
        if out_networks.is_empty() {
            return Err("A service has to be on at least one network.".into());
        }
        if out_networks == [DEFAULT_NETWORK] {
            out_networks.clear();
        }

        let mut names = Vec::new();
        let mut changes = Vec::new();
        // In file mode the text box is the whole truth: every line is set, anything missing goes.
        if let Some(text) = env_text.get_untracked() {
            for (var, value) in parse_env_file(&text)? {
                changes.push(EnvChange { service: name.clone(), name: var.clone(), value });
                names.push(var);
            }
        }
        for row in env.get_untracked().into_iter().filter(|_| env_text.get_untracked().is_none()) {
            let var = row.name.get_untracked().trim().to_string();
            if var.is_empty() {
                continue;
            }
            if row.editing.get_untracked() || !row.existing {
                changes.push(EnvChange { service: name.clone(), name: var.clone(), value: row.value.get_untracked() });
            }
            names.push(var);
        }

        let user = user.get_untracked().trim().to_string();
        let spec = ServiceSpec {
            name,
            image: image.get_untracked().trim().to_string(),
            restart: restart.get_untracked(),
            ports: out_ports,
            volumes: out_volumes,
            env: names,
            command: split_command(&command.get_untracked())?,
            user: (!user.is_empty()).then_some(user),
            mounts: out_mounts,
            networks: out_networks,
            depends_on: out_after,
            run_once: run_once.get_untracked(),
        };
        Ok((spec, changes))
    };
    editor.build.set_value(Some(Callback::new(move |_: ()| build())));
    editor.reload.set_value(Some(Callback::new(move |s: ServiceSpec| load(&s))));

    let remove_service = move |_| {
        let Some(mut next) = latest.get_untracked() else { return };
        let name = service_name.get_value();
        next.services.retain(|s| s.name != name);
        dirty.set(false);
        save.dispatch(SaveStack { spec: next, changes: Vec::new() });
    };

    let reveal = move |row: EnvRow| {
        let stack = stack_name.get_value();
        let service = service_name.get_value();
        let name = row.name.get_untracked();
        leptos::task::spawn_local(async move {
            match reveal_env(stack, service, name).await {
                Ok(value) => {
                    row.shown.set(Some(value));
                    set_timeout(move || row.shown.set(None), std::time::Duration::from_secs(REVEAL_SECS));
                }
                Err(e) => problem.set(Some(e.to_string())),
            }
        });
    };

    let open_file = move |_| {
        let stack = stack_name.get_value();
        let service = service_name.get_value();
        env_loading.set(true);
        leptos::task::spawn_local(async move {
            match reveal_env_file(stack, service).await {
                Ok(text) => env_text.set(Some(text)),
                Err(e) => problem.set(Some(e.to_string())),
            }
            env_loading.set(false);
        });
    };

    let count = move |n: usize| if n == 0 { String::new() } else { n.to_string() };

    view! {
        <div class="inspector">
            <header class="inspector__head">
                <div class="inspector__titles">
                    <h2 class="inspector__title">{service.clone()}</h2>
                    <span class="inspector__image">{move || image.get()}</span>
                </div>
                <Show when=move || status.with(Option::is_some)>
                    <Transition fallback=|| ()>
                        <ActionBar detail=Signal::derive(move || detail.get().flatten()) dialog on_done />
                    </Transition>
                </Show>
            </header>

            <div class="inspector__status">
                {move || match status.get() {
                    Some(s) => view! {
                        <HealthBadge health=s.health label=s.health_label />
                        <span class="inspector__status-text">{s.status_text}</span>
                    }
                    .into_any(),
                    None => view! {
                        <span class="badge is-neutral"><span class="dot"></span>"Not running"</span>
                        <span class="inspector__status-text">
                            {if deployed.get() {
                                "No container yet. Apply the stack to create it."
                            } else {
                                "Deploy the stack to start it."
                            }}
                        </span>
                    }
                    .into_any(),
                }}
            </div>

            <div class="tabs inspector__tabs" role="tablist">
                <TabButton tab which=Tab::General label="General" count=Signal::derive(|| String::new()) />
                <TabButton tab which=Tab::Ports label="Ports" count=Signal::derive(move || count(ports.with(Vec::len))) />
                <TabButton tab which=Tab::Storage label="Storage" count=Signal::derive(move || count(volumes.with(Vec::len) + mounts.with(Vec::len))) />
                <TabButton tab which=Tab::Env label="Env" count=Signal::derive(move || count(env.with(Vec::len))) />
                <TabButton tab which=Tab::Logs label="Logs" count=Signal::derive(|| String::new()) />
            </div>

            {move || problem.get().map(|p| view! { <p class="inspector__problem" role="alert">{p}</p> })}

            <div class="inspector__body" class:is-hidden=move || tab.get() != Tab::General>
                <div class="inspector__grid">
                    <label class="field">
                        <span class="field__label">"IMAGE"</span>
                        <ImageInput
                            value=image
                            images
                            list_id=format!("image-options-{service}")
                            on_input=Callback::new(move |_: ()| touch())
                        />
                    </label>
                    <label class="field">
                        <span class="field__label">"RESTART"</span>
                        <select
                            class="field__input field__input--plain"
                            disabled=move || run_once.get()
                            prop:value=move || restart.get().compose()
                            on:change=move |ev| {
                                let picked = event_target_value(&ev);
                                if let Some(r) = Restart::ALL.into_iter().find(|r| r.compose() == picked) {
                                    restart.set(r);
                                    touch();
                                }
                            }
                        >
                            {Restart::ALL.into_iter().map(|r| view! { <option value=r.compose()>{r.label()}</option> }).collect_view()}
                        </select>
                    </label>
                </div>
                <label class="check inspector__check">
                    <input
                        type="checkbox"
                        prop:checked=move || run_once.get()
                        on:change=move |ev| {
                            run_once.set(event_target_checked(&ev));
                            touch();
                        }
                    />
                    <span>
                        <strong>"Runs once"</strong>
                        " A helper that runs to the end and stays stopped, like a setup step. Exit code 0 shows as Finished, not as a problem."
                    </span>
                </label>
                <label class="field inspector__command">
                    <span class="field__label">"COMMAND"</span>
                    <input
                        class="field__input field__input--plain"
                        type="text"
                        placeholder="empty = the image's own command"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || command.get()
                        on:input=move |ev| {
                            command.set(event_target_value(&ev));
                            touch();
                        }
                    />
                    <span class="field__hint">
                        "Arguments for the image, written like on the command line. SurrealDB, for example: "
                        <code>"start --log info surrealkv:/data/db"</code>
                        ". Keep passwords out of it; put them in Env (it shows up in compose.yml and docker inspect)."
                    </span>
                </label>
                <label class="field inspector__command">
                    <span class="field__label">"RUN AS"</span>
                    <input
                        class="field__input field__input--plain"
                        type="text"
                        placeholder="empty = the image's own user"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || user.get()
                        on:input=move |ev| {
                            user.set(event_target_value(&ev));
                            touch();
                        }
                    />
                    <span class="field__hint">
                        "A user name or ID inside the container, like "<code>"1000"</code>" or "<code>"1000:1000"</code>
                        ". If the logs say Permission denied on its storage, "<code>"0"</code>" (root inside the container) fixes it."
                    </span>
                </label>

                <div class="field inspector__command">
                    <span class="field__label">"START AFTER"</span>
                    <For each=move || after.get() key=|r| r.id let:row>
                        {
                            let me = service_name.get_value();
                            let current = row.service.get_untracked();
                            let others: Vec<String> = latest.with_untracked(|s| {
                                s.as_ref()
                                    .map(|s| s.services.iter().map(|x| x.name.clone()).filter(|n| *n != me).collect())
                                    .unwrap_or_default()
                            });
                            let condition_now = row.condition.get_untracked();
                            view! {
                                <div class="editor__row editor__row--after">
                                    <select
                                        class="field__input field__input--plain"
                                        aria-label="Service to wait for"
                                        prop:value=move || row.service.get()
                                        on:change=move |ev| {
                                            row.service.set(event_target_value(&ev));
                                            touch();
                                        }
                                    >
                                        {others
                                            .into_iter()
                                            .map(|n| {
                                                let picked = n == current;
                                                view! { <option value=n.clone() selected=picked>{n.clone()}</option> }
                                            })
                                            .collect_view()}
                                    </select>
                                    <select
                                        class="field__input field__input--plain"
                                        aria-label="Wait until it"
                                        prop:value=move || row.condition.get().key()
                                        on:change=move |ev| {
                                            let picked = event_target_value(&ev);
                                            if let Some(c) = StartCondition::ALL.into_iter().find(|c| c.key() == picked) {
                                                row.condition.set(c);
                                                touch();
                                            }
                                        }
                                    >
                                        {StartCondition::ALL
                                            .into_iter()
                                            .map(|c| view! { <option value=c.key() selected=c == condition_now>{c.label()}</option> })
                                            .collect_view()}
                                    </select>
                                    <button
                                        type="button"
                                        class="icon-btn"
                                        aria-label="Remove"
                                        on:click=move |_| {
                                            after.update(|rows| rows.retain(|r| r.id != row.id));
                                            touch();
                                        }
                                    >
                                        <Ico icon=icondata::LuTrash2 />
                                    </button>
                                </div>
                            }
                        }
                    </For>
                    <button
                        type="button"
                        class="tool-btn"
                        disabled={move || latest.with(|s| s.as_ref().is_none_or(|s| s.services.len() < 2))}
                        on:click=add_after
                    >
                        <Ico icon=icondata::LuPlus />
                        "Wait for a service"
                    </button>
                    <span class="field__hint">
                        "This service starts only once those reach that point. Use finished successfully for a Runs once helper. Is healthy only works if that image has a health check."
                    </span>
                </div>

                <div class="field inspector__command">
                    <span class="field__label">"NETWORKS"</span>
                    <div class="check-list">
                        {move || {
                            let mut all = vec![DEFAULT_NETWORK.to_string()];
                            all.extend(latest.with(|s| s.as_ref().map(|s| s.networks.iter().map(|n| n.name.clone()).collect::<Vec<_>>()).unwrap_or_default()));
                            all.into_iter()
                                .map(|n| {
                                    let for_checked = n.clone();
                                    let for_change = n.clone();
                                    view! {
                                        <label class="check">
                                            <input
                                                type="checkbox"
                                                prop:checked=move || networks.with(|l| l.contains(&for_checked))
                                                on:change=move |ev| {
                                                    let on = event_target_checked(&ev);
                                                    networks.update(|l| {
                                                        l.retain(|x| *x != for_change);
                                                        if on {
                                                            l.push(for_change.clone());
                                                        }
                                                    });
                                                    touch();
                                                }
                                            />
                                            <span>{n}</span>
                                        </label>
                                    }
                                })
                                .collect_view()
                        }}
                    </div>
                    <span class="field__hint">
                        "default is the stack's own network. Services reach each other by name only on a network they share."
                    </span>
                </div>
                <Show when=move || active.get() && running.get() && tab.get() == Tab::General>
                    {move || view! { <StatsStrip container=container_name.get() /> }}
                </Show>
                <Transition fallback=|| ()>
                    {move || detail.get().flatten().map(|d| view! { <LiveFacts detail=d /> })}
                </Transition>
                <div class="inspector__danger">
                    <div>
                        <strong>"Remove this service"</strong>
                        <p>"It leaves the stack's files now; its container goes on the next apply."</p>
                    </div>
                    <button type="button" class="act act--danger" disabled=move || save.pending().get() on:click=remove_service>
                        <Ico icon=icondata::LuTrash2 />
                        "Remove"
                    </button>
                </div>
            </div>

            <div class="inspector__body" class:is-hidden=move || tab.get() != Tab::Ports>
                <p class="inspector__hint">
                    "The first box is the port your app listens on inside its container. Then choose who may reach it."
                </p>
                <For each=move || ports.get() key=|r| r.id let:row>
                    <div class="editor__port" class:is-debug=move || row.public.get()>
                    <div class="editor__row editor__row--port">
                        <input
                            class="field__input field__input--plain"
                            type="text"
                            inputmode="numeric"
                            placeholder="3000"
                            aria-label="Container port"
                            prop:value=move || row.container.get()
                            on:input=move |ev| {
                                row.container.set(event_target_value(&ev));
                                touch();
                            }
                        />
                        <select
                            class="field__input field__input--plain"
                            aria-label="Where this port is reachable"
                            prop:value=move || {
                                if row.public.get() {
                                    "debug"
                                } else if row.published.get() {
                                    "server"
                                } else {
                                    "stack"
                                }
                            }
                            on:change=move |ev| {
                                let picked = event_target_value(&ev);
                                row.published.set(picked != "stack");
                                row.public.set(picked == "debug");
                                touch();
                            }
                        >
                            <option value="stack">"Only inside this stack"</option>
                            <option value="server">"Also on the server"</option>
                            <option value="debug">"Debug: open on all networks"</option>
                        </select>
                        <Show when=move || row.published.get()>
                            <span class="editor__host">
                                <span class="editor__host-ip">{move || if row.public.get() { "0.0.0.0:" } else { "127.0.0.1:" }}</span>
                                <input
                                    class="field__input field__input--plain"
                                    type="text"
                                    inputmode="numeric"
                                    placeholder="6391"
                                    aria-label="Port on the server"
                                    prop:value=move || row.host.get()
                                    on:input=move |ev| {
                                        row.host.set(event_target_value(&ev));
                                        touch();
                                    }
                                />
                            </span>
                        </Show>
                        <button
                            type="button"
                            class="icon-btn"
                            aria-label="Remove port"
                            on:click=move |_| {
                                ports.update(|rows| rows.retain(|r| r.id != row.id));
                                touch();
                            }
                        >
                            <Ico icon=icondata::LuTrash2 />
                        </button>
                    </div>
                    <p class="editor__explain">{move || explain_port(&stack_name.get_value(), &service_name.get_value(), row)}</p>
                    </div>
                </For>
                <button type="button" class="tool-btn" on:click=add_port><Ico icon=icondata::LuPlus />"Add port"</button>
            </div>

            <div class="inspector__body" class:is-hidden=move || tab.get() != Tab::Storage>
                <p class="inspector__hint">"Each storage is a named volume, kept across restarts and redeploys. A new volume belongs to root unless the image already has that folder, so a program running as another user may not be able to write to it. Then set Run as to 0 in General."</p>
                <span class="field__label">"OWN STORAGE"</span>
                <For each=move || volumes.get() key=|r| r.id let:row>
                    <div class="editor__row editor__row--volume">
                        <input
                            class="field__input field__input--plain"
                            type="text"
                            placeholder="data"
                            aria-label="Storage name"
                            prop:value=move || row.name.get()
                            on:input=move |ev| {
                                row.name.set(event_target_value(&ev).to_lowercase());
                                touch();
                            }
                        />
                        <input
                            class="field__input field__input--plain"
                            type="text"
                            placeholder="/path/in/container"
                            aria-label="Path inside the container"
                            prop:value=move || row.path.get()
                            on:input=move |ev| {
                                row.path.set(event_target_value(&ev));
                                touch();
                            }
                        />
                        <button
                            type="button"
                            class="icon-btn"
                            aria-label="Remove storage"
                            on:click=move |_| {
                                volumes.update(|rows| rows.retain(|r| r.id != row.id));
                                touch();
                            }
                        >
                            <Ico icon=icondata::LuTrash2 />
                        </button>
                    </div>
                </For>
                <button type="button" class="tool-btn" on:click=add_volume><Ico icon=icondata::LuPlus />"Add storage"</button>

                <span class="field__label inspector__section">"SHARED STORAGE"</span>
                {move || {
                    latest
                        .with(|s| s.as_ref().is_some_and(|s| s.storages.is_empty()))
                        .then(|| view! {
                            <p class="inspector__hint">"This stack has no shared storage yet. Add one under STORAGE in the list on the left, then mount it here."</p>
                        })
                }}
                <For each=move || mounts.get() key=|r| r.id let:row>
                    {
                        let current = row.storage.get_untracked();
                        let storages: Vec<String> = latest.with_untracked(|s| {
                            s.as_ref().map(|s| s.storages.iter().map(|x| x.name.clone()).collect()).unwrap_or_default()
                        });
                        view! {
                            <div class="editor__row editor__row--mount">
                                <select
                                    class="field__input field__input--plain"
                                    aria-label="Shared storage"
                                    prop:value=move || row.storage.get()
                                    on:change=move |ev| {
                                        row.storage.set(event_target_value(&ev));
                                        touch();
                                    }
                                >
                                    {storages
                                        .into_iter()
                                        .map(|n| {
                                            let picked = n == current;
                                            view! { <option value=n.clone() selected=picked>{n.clone()}</option> }
                                        })
                                        .collect_view()}
                                </select>
                                <input
                                    class="field__input field__input--plain"
                                    type="text"
                                    placeholder="/path/in/container"
                                    aria-label="Path inside the container"
                                    prop:value=move || row.path.get()
                                    on:input=move |ev| {
                                        row.path.set(event_target_value(&ev));
                                        touch();
                                    }
                                />
                                <label class="check">
                                    <input
                                        type="checkbox"
                                        prop:checked=move || row.read_only.get()
                                        on:change=move |ev| {
                                            row.read_only.set(event_target_checked(&ev));
                                            touch();
                                        }
                                    />
                                    <span>"read-only"</span>
                                </label>
                                <button
                                    type="button"
                                    class="icon-btn"
                                    aria-label="Stop using this storage"
                                    on:click=move |_| {
                                        mounts.update(|rows| rows.retain(|r| r.id != row.id));
                                        touch();
                                    }
                                >
                                    <Ico icon=icondata::LuTrash2 />
                                </button>
                            </div>
                        }
                    }
                </For>
                <button
                    type="button"
                    class="tool-btn"
                    disabled={move || latest.with(|s| s.as_ref().is_none_or(|s| s.storages.is_empty()))}
                    on:click=add_mount
                >
                    <Ico icon=icondata::LuPlus />
                    "Use shared storage"
                </button>
            </div>

            <div class="inspector__body" class:is-hidden=move || tab.get() != Tab::Env>
                <div class="inspector__env-head">
                    <p class="inspector__hint">"Values live in a private file on the server and stay hidden until you reveal them (written to the audit log)."</p>
                    <Show
                        when=move || env_text.with(Option::is_some)
                        fallback=move || view! {
                            <button type="button" class="tool-btn" disabled=move || env_loading.get() on:click=open_file>
                                <Ico icon=icondata::LuFileText />
                                {move || if env_loading.get() { "Opening…" } else { "Edit as file" }}
                            </button>
                        }
                    >
                        <button
                            type="button"
                            class="tool-btn"
                            title="Leave the file without saving it"
                            on:click=move |_| env_text.set(None)
                        >
                            "Back to the list"
                        </button>
                    </Show>
                </div>
                <Show when=move || env_text.with(Option::is_some)>
                    <textarea
                        class="field__input field__input--plain env-file"
                        spellcheck="false"
                        autocomplete="off"
                        rows="14"
                        placeholder="DATABASE_URL=postgres://user:pass@db:5432/app"
                        prop:value=move || env_text.get().unwrap_or_default()
                        on:input=move |ev| {
                            env_text.set(Some(event_target_value(&ev)));
                            touch();
                        }
                    ></textarea>
                    <p class="inspector__hint">
                        "One "<code>"NAME=value"</code>" per line. Comments with "<code>"#"</code>", "<code>"export"</code>
                        " and quotes are fine, so you can paste a whole .env. Saving replaces all variables with exactly these."
                    </p>
                </Show>
                <div class="env-list" class:is-hidden=move || env_text.with(Option::is_some)>
                <For each=move || env.get() key=|r| r.id let:row>
                    <div class="editor__row editor__row--env">
                        {if row.existing {
                            view! { <code class="editor__var">{move || row.name.get()}</code> }.into_any()
                        } else {
                            view! {
                                <input
                                    class="field__input field__input--plain"
                                    type="text"
                                    placeholder="NAME"
                                    aria-label="Variable name"
                                    spellcheck="false"
                                    prop:value=move || row.name.get()
                                    on:input=move |ev| {
                                        row.name.set(event_target_value(&ev));
                                        touch();
                                    }
                                />
                            }
                            .into_any()
                        }}
                        <Show
                            when=move || row.editing.get()
                            fallback=move || {
                                view! {
                                    <span class="editor__secret" class:is-shown=move || row.shown.get().is_some()>
                                        {move || row.shown.get().unwrap_or_else(|| "••••••••".into())}
                                    </span>
                                }
                            }
                        >
                            <input
                                class="field__input field__input--plain"
                                type="text"
                                placeholder="value"
                                aria-label="Value"
                                autocomplete="off"
                                spellcheck="false"
                                prop:value=move || row.value.get()
                                on:input=move |ev| {
                                    row.value.set(event_target_value(&ev));
                                    touch();
                                }
                            />
                        </Show>
                        <span class="editor__row-actions">
                            <Show when=move || row.existing && !row.editing.get()>
                                <button
                                    type="button"
                                    class="tool-btn"
                                    on:click=move |_| {
                                        if row.shown.get_untracked().is_some() {
                                            row.shown.set(None);
                                        } else {
                                            reveal(row);
                                        }
                                    }
                                >
                                    {move || if row.shown.get().is_some() { "Hide" } else { "Reveal" }}
                                </button>
                                <button
                                    type="button"
                                    class="tool-btn"
                                    on:click=move |_| {
                                        row.shown.set(None);
                                        row.editing.set(true);
                                        touch();
                                    }
                                >
                                    "Change"
                                </button>
                            </Show>
                            <button
                                type="button"
                                class="icon-btn"
                                aria-label="Remove variable"
                                on:click=move |_| {
                                    env.update(|rows| rows.retain(|r| r.id != row.id));
                                    touch();
                                }
                            >
                                <Ico icon=icondata::LuTrash2 />
                            </button>
                        </span>
                    </div>
                </For>
                <button type="button" class="tool-btn" on:click=add_env><Ico icon=icondata::LuPlus />"Add variable"</button>
                </div>
            </div>

            <div class="inspector__body inspector__body--logs" class:is-hidden=move || tab.get() != Tab::Logs>
                <Show
                    when=move || active.get() && deployed.get() && tab.get() == Tab::Logs
                    fallback=move || view! { <p class="tab-empty">"Deploy the stack to see this service's logs."</p> }
                >
                    {move || {
                        log_epoch.track();
                        view! { <LogViewer container=container_name.get() /> }
                    }}
                </Show>
            </div>
        </div>
    }
    .into_any()
}

// Type to search the installed images (the browser suggests matches as you type), or pick one
// from the list. Anything typed is checked against what's on the server when saving.
#[component]
pub fn ImageInput(
    value: RwSignal<String>,
    images: Resource<Result<Vec<String>, DockerError>>,
    list_id: String,
    #[prop(optional)] on_input: Option<Callback<()>>,
) -> impl IntoView {
    let options_id = list_id.clone();
    view! {
        <input
            class="field__input field__input--plain"
            type="text"
            list=list_id
            placeholder="Type to search, like postgres or redis"
            autocomplete="off"
            spellcheck="false"
            prop:value=move || value.get()
            on:input=move |ev| {
                value.set(event_target_value(&ev));
                if let Some(cb) = on_input {
                    cb.run(());
                }
            }
        />
        <Transition fallback=|| ()>
            {move || {
                let list = images.get().and_then(Result::ok).unwrap_or_default();
                let typed = value.get().trim().to_string();
                let missing = !typed.is_empty() && !list.is_empty() && !list.contains(&typed);
                let empty = list.is_empty();
                view! {
                    <datalist id=options_id.clone()>
                        {list.into_iter().map(|i| view! { <option value=i.clone()></option> }).collect_view()}
                    </datalist>
                    <span class="field__hint" class:is-warn=missing>
                        {if empty {
                            "No images on this server yet. Pull or upload one in Images."
                        } else if missing {
                            "Not on this server. Use the full name with its tag, or pull it in Images first."
                        } else {
                            ""
                        }}
                    </span>
                }
            }}
        </Transition>
    }
}

// Says, for this exact row, who can reach the port.
fn explain_port(stack: &str, service: &str, row: PortRow) -> String {
    let port = row.container.get().trim().to_string();
    if port.is_empty() {
        return "Type the port your app listens on, like 3000.".into();
    }
    if !row.published.get() {
        return format!(
            "Other services in {stack} reach it at {service}:{port}. Nothing outside this stack can: not the server, not your LAN."
        );
    }
    let host = row.host.get().trim().to_string();
    let host = if host.is_empty() { "<port>".to_string() } else { host };
    if row.public.get() {
        return format!(
            "Debug port: anyone who can reach this server on port {host} can connect, so every device on your LAN, and the internet too if your router forwards it. Switch it back when you're done; Needs you lists it while it's open."
        );
    }
    format!(
        "Also on the server at 127.0.0.1:{host}, which only the server itself can reach: open it from your PC with an SSH tunnel, or later through Cloudflare. Your LAN and the internet still can't. Inside the stack it stays {service}:{port}."
    )
}

#[component]
fn LiveFacts(detail: ServiceDetail) -> impl IntoView {
    let short_id: String = detail.card.id.chars().take(12).collect();
    let uptime = match (&detail.started_ago, detail.exit_code) {
        (Some(ago), _) => Some(("Up for", ago.clone())),
        (None, Some(code)) => Some(("Exit code", code.to_string())),
        _ => None,
    };
    view! {
        <section class="inspector__facts">
            <dl class="kv">
                {uptime.map(|(k, v)| view! { <dt>{k}</dt><dd>{v}</dd> })}
                <dt>"Restarts"</dt>
                <dd>{detail.restart_count}</dd>
                {detail.created.clone().map(|c| view! { <dt>"Created"</dt><dd>{c}</dd> })}
                <dt>"Container"</dt>
                <dd class="is-mono">{detail.card.container_name.clone()}</dd>
                <dt>"ID"</dt>
                <dd class="is-mono">{short_id}</dd>
                <dt>"Networks"</dt>
                <dd class="inspector__nets">
                    {detail
                        .networks
                        .into_iter()
                        .map(|n| {
                            let ip = n.ip.unwrap_or_default();
                            view! {
                                <span class="inspector__net">
                                    <NetworkChip name=n.name />
                                    <span class="is-mono">{ip}</span>
                                </span>
                            }
                        })
                        .collect_view()}
                </dd>
            </dl>
        </section>
    }
}

#[component]
fn TabButton(tab: RwSignal<Tab>, which: Tab, label: &'static str, count: Signal<String>) -> impl IntoView {
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
            <span class="tabs__count">{move || count.get()}</span>
        </button>
    }
}
