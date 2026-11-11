use leptos::prelude::*;
use shared::docker::{Action, ServiceDetail};
use shared::errors::docker::DockerError;
use shared::stack::{
    check_spec, join_command, parse_env_file, split_command, EnvChange, PortSpec, Restart, ServiceSpec, ServiceStatus,
    StackSpec, VolumeSpec,
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
    host: RwSignal<String>,
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

// The stack page owns `dirty` and `save_now`: the changes bar reads `dirty` and bumps
// `save_now` when you press Save, so the bar can live outside the inspector.
#[component]
pub fn ServiceInspector(
    stack: StackSpec,
    latest: Memo<Option<StackSpec>>,
    service: String,
    deployed: Signal<bool>,
    status: Signal<Option<ServiceStatus>>,
    save: ServerAction<SaveStack>,
    images: Resource<Result<Vec<String>, DockerError>>,
    dirty: RwSignal<bool>,
    save_now: RwSignal<u32>,
    on_saved: Callback<()>,
) -> impl IntoView {
    let Some(original) = stack.services.iter().find(|s| s.name == service).cloned() else {
        return view! { <p class="tab-empty">"That service isn't in the stack anymore."</p> }.into_any();
    };

    let tab = RwSignal::new(Tab::General);
    let next_id = StoredValue::new(0u32);
    let new_id = move || {
        next_id.update_value(|n| *n += 1);
        next_id.get_value()
    };
    let touch = move || dirty.set(true);

    let image = RwSignal::new(original.image.clone());
    let restart = RwSignal::new(original.restart);
    let command = RwSignal::new(join_command(&original.command));
    let ports = RwSignal::new(
        original
            .ports
            .iter()
            .map(|p| PortRow {
                id: new_id(),
                container: RwSignal::new(p.container.to_string()),
                published: RwSignal::new(p.host.is_some()),
                host: RwSignal::new(p.host.map(|h| h.to_string()).unwrap_or_default()),
            })
            .collect::<Vec<_>>(),
    );
    let volumes = RwSignal::new(
        original
            .volumes
            .iter()
            .map(|v| VolumeRow { id: new_id(), name: RwSignal::new(v.name.clone()), path: RwSignal::new(v.path.clone()) })
            .collect::<Vec<_>>(),
    );
    let env = RwSignal::new(
        original
            .env
            .iter()
            .map(|n| EnvRow {
                id: new_id(),
                name: RwSignal::new(n.clone()),
                existing: true,
                value: RwSignal::new(String::new()),
                editing: RwSignal::new(false),
                shown: RwSignal::new(None),
            })
            .collect::<Vec<_>>(),
    );

    let problem = RwSignal::new(None::<String>);
    let env_text = RwSignal::new(None::<String>);
    let env_loading = RwSignal::new(false);
    let submitted = RwSignal::new(false);
    let stack_name = StoredValue::new(stack.name.clone());
    let service_name = StoredValue::new(service.clone());
    let default_container = format!("{}-{}-1", stack.name, service);
    let container_name = Signal::derive(move || {
        status.get().map(|s| s.container).unwrap_or_else(|| default_container.clone())
    });
    let running = Signal::derive(move || status.with(|s| s.as_ref().is_some_and(|s| s.running)));

    // Live facts and the Start / Stop / Restart buttons come from Docker, refreshed with the page.
    let tick = Tick::signal();
    let detail = Resource::new(
        move || (status.with(Option::is_some), container_name.get(), tick.get()),
        |(exists, container, _)| async move {
            if exists { get_service(container).await.ok() } else { None }
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
                host: RwSignal::new(String::new()),
            })
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
            out_ports.push(PortSpec { container, host });
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

        let spec = ServiceSpec {
            name,
            image: image.get_untracked(),
            restart: restart.get_untracked(),
            ports: out_ports,
            volumes: out_volumes,
            env: names,
            command: split_command(&command.get_untracked())?,
        };
        Ok((spec, changes))
    };

    let save_draft = move || {
        let (service_spec, changes) = match build() {
            Ok(built) => built,
            Err(message) => return problem.set(Some(message)),
        };
        let Some(mut next) = latest.get_untracked() else { return };
        if let Some(slot) = next.services.iter_mut().find(|s| s.name == service_spec.name) {
            *slot = service_spec;
        }
        if let Err(p) = check_spec(&next) {
            return problem.set(Some(p.0));
        }
        problem.set(None);
        submitted.set(true);
        save.dispatch(SaveStack { spec: next, changes });
    };

    let seen = StoredValue::new(save_now.get_untracked());
    Effect::new(move |_| {
        let n = save_now.get();
        if n != seen.get_value() {
            seen.set_value(n);
            save_draft();
        }
    });

    Effect::new(move |_| {
        let result = save.value().get();
        if !submitted.get_untracked() {
            return;
        }
        match result {
            Some(Ok(())) => {
                submitted.set(false);
                dirty.set(false);
                on_saved.run(());
            }
            Some(Err(e)) => {
                submitted.set(false);
                problem.set(Some(e.to_string()));
            }
            None => {}
        }
    });

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
                <TabButton tab which=Tab::Storage label="Storage" count=Signal::derive(move || count(volumes.with(Vec::len))) />
                <TabButton tab which=Tab::Env label="Env" count=Signal::derive(move || count(env.with(Vec::len))) />
                <TabButton tab which=Tab::Logs label="Logs" count=Signal::derive(|| String::new()) />
            </div>

            {move || problem.get().map(|p| view! { <p class="inspector__problem" role="alert">{p}</p> })}

            <div class="inspector__body" class:is-hidden=move || tab.get() != Tab::General>
                <div class="inspector__grid">
                    <label class="field">
                        <span class="field__label">"IMAGE"</span>
                        <Transition fallback=|| ()>
                            {move || {
                                let mut list = images.get().and_then(Result::ok).unwrap_or_default();
                                let current = image.get_untracked();
                                if !current.is_empty() && !list.contains(&current) {
                                    list.insert(0, current);
                                }
                                view! {
                                    <select
                                        class="field__input field__input--plain"
                                        prop:value=move || image.get()
                                        on:change=move |ev| {
                                            image.set(event_target_value(&ev));
                                            touch();
                                        }
                                    >
                                        {list.into_iter().map(|i| view! { <option value=i.clone()>{i.clone()}</option> }).collect_view()}
                                    </select>
                                }
                            }}
                        </Transition>
                    </label>
                    <label class="field">
                        <span class="field__label">"RESTART"</span>
                        <select
                            class="field__input field__input--plain"
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
                <Show when=move || running.get() && tab.get() == Tab::General>
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
                    <div class="editor__port">
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
                            prop:value=move || if row.published.get() { "server" } else { "stack" }
                            on:change=move |ev| {
                                row.published.set(event_target_value(&ev) == "server");
                                touch();
                            }
                        >
                            <option value="stack">"Only inside this stack"</option>
                            <option value="server">"Also on the server"</option>
                        </select>
                        <Show when=move || row.published.get()>
                            <span class="editor__host">
                                <span class="editor__host-ip">"127.0.0.1:"</span>
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
                <p class="inspector__hint">"Each storage is a named volume, kept across restarts and redeploys. Docker sets its owner, so the container can write to it."</p>
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
                    when=move || deployed.get() && tab.get() == Tab::Logs
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
