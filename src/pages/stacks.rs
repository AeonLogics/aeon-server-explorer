use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use std::collections::HashMap;

use shared::stack::{check_spec, valid_name, ServiceSpec, StackSpec, StackState, StackSummary, StackView};

use super::common::DockerProblem;
use super::service_editor::{Editor, ServiceInspector};
use crate::api::stacks::{get_images, get_stack, get_stacks, ApplyStack, CreateStack, SaveStack};
use crate::components::icon::Ico;
use crate::components::poll::{watch_session, Tick};
use crate::components::status::calm_status;

#[component]
pub fn StateBadge(state: StackState) -> impl IntoView {
    let (class, label) = match state {
        StackState::Deployed => ("badge is-ok", "Deployed"),
        StackState::Pending => ("badge is-warn", "Changes pending"),
        StackState::NotDeployed => ("badge is-neutral", "Not deployed"),
    };
    view! { <span class=class><span class="dot" aria-hidden="true"></span>{label}</span> }
}

#[component]
pub fn StacksPage() -> impl IntoView {
    let tick = Tick::signal();
    let create = ServerAction::<CreateStack>::new();
    let stacks = Resource::new(
        move || (tick.get(), create.version().get()),
        |_| get_stacks(),
    );
    watch_session(move || stacks.get().and_then(Result::err));

    let name = RwSignal::new(String::new());
    let navigate = use_navigate();
    let typed_ok = move || valid_name(name.get().trim());

    Effect::new(move |_| {
        if matches!(create.value().get(), Some(Ok(()))) {
            let target = format!("/stacks/{}", name.get_untracked().trim());
            name.set(String::new());
            navigate(&target, NavigateOptions::default());
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        if typed_ok() {
            create.dispatch(CreateStack {
                name: name.get_untracked().trim().to_string(),
            });
        }
    };
    let error = move || {
        create
            .value()
            .get()
            .and_then(Result::err)
            .map(|e| e.to_string())
    };

    view! {
        <div class="page">
            <header class="page__head">
                <div class="page__titles">
                    <h1 class="page__title">"Stacks"</h1>
                    <p class="page__lead">
                        "Stacks you build here. The app writes their compose files and runs them."
                    </p>
                </div>
            </header>

            <form class="create" on:submit=on_submit>
                <label class="field create__field">
                    <span class="field__label">"NEW STACK"</span>
                    <input
                        class="field__input field__input--plain"
                        type="text"
                        placeholder="my-app"
                        maxlength="40"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || name.get()
                        on:input=move |ev| name.set(event_target_value(&ev).to_lowercase())
                    />
                </label>
                <button class="btn btn--primary" type="submit" disabled=move || !typed_ok() || create.pending().get()>
                    {move || if create.pending().get() { "Creating…" } else { "Create stack" }}
                </button>
                <p class="create__hint">
                    {move || match error() {
                        Some(e) => e,
                        None => "Lowercase letters, digits, - and _. It becomes the folder and compose project name.".into(),
                    }}
                </p>
            </form>

            <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                {move || {
                    stacks
                        .get()
                        .map(|result| match result {
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                            Ok(list) if list.is_empty() => view! {
                                <p class="tab-empty">"No stacks yet. Create one above."</p>
                            }
                            .into_any(),
                            Ok(list) => view! { <StackTable list /> }.into_any(),
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn StackTable(list: Vec<StackSummary>) -> impl IntoView {
    view! {
        <section class="table" aria-label="Stacks">
            <div class="table__row table__row--head" aria-hidden="true">
                <span>"Stack"</span>
                <span>"Services"</span>
                <span>"State"</span>
                <span></span>
            </div>
            {list
                .into_iter()
                .map(|s| {
                    let href = format!("/stacks/{}", s.name);
                    view! {
                        <a class="table__row" href=href>
                            <span class="table__name"><span>{s.name}</span></span>
                            <span class="table__mono">{s.services}</span>
                            <span><StateBadge state=s.state /></span>
                            <span class="table__go"><Ico icon=icondata::LuChevronRight /></span>
                        </a>
                    }
                })
                .collect_view()}
        </section>
    }
}

// Marks "Add service" in the selection, since no service can have this name.
const NEW: &str = "+new";

// A save started from the changes bar: which services it carried, and whether to deploy after.
#[derive(Clone)]
struct PendingSave {
    spec: StackSpec,
    services: Vec<String>,
    deploy: bool,
}

#[component]
pub fn StackPage() -> impl IntoView {
    let location = use_location();
    let tick = Tick::signal();
    let name = Memo::new(move |_| {
        location
            .pathname
            .get()
            .trim_matches('/')
            .split('/')
            .nth(1)
            .unwrap_or_default()
            .to_string()
    });

    let save = ServerAction::<SaveStack>::new();
    let apply = ServerAction::<ApplyStack>::new();
    let stack = Resource::new(
        move || {
            (
                name.get(),
                tick.get(),
                save.version().get(),
                apply.version().get(),
            )
        },
        |(n, ..)| get_stack(n),
    );
    let images = Resource::new(|| (), |_| get_images());
    watch_session(move || stack.get().and_then(Result::err));

    // The last view that loaded. A refresh that fails (Docker busy, a dropped request) keeps
    // showing it instead of tearing down the workbench and everything typed into it.
    let view_data = Memo::new(move |prev: Option<&Option<StackView>>| {
        let current = name.get();
        match stack.get() {
            Some(Ok(v)) if v.spec.name == current => Some(v),
            _ => prev
                .cloned()
                .flatten()
                .filter(|v| v.spec.name == current),
        }
    });
    let refresh_error = move || {
        stack
            .get()
            .and_then(Result::err)
            .filter(|_| view_data.with(Option::is_some))
    };

    let spec = Memo::new(move |_| view_data.with(|v| v.as_ref().map(|v| v.spec.clone())));
    let compose = Memo::new(move |_| view_data.with(|v| v.as_ref().map(|v| v.compose.clone())));
    let folder = Memo::new(move |_| {
        view_data.with(|v| v.as_ref().map(|v| v.folder.clone()).unwrap_or_default())
    });
    let state = Memo::new(move |_| view_data.with(|v| v.as_ref().map(|v| v.state.clone())));
    let deployed =
        Signal::derive(move || state.get().is_some_and(|s| s != StackState::NotDeployed));
    let status = Memo::new(move |_| {
        view_data.with(|v| {
            v.as_ref()
                .map(|v| {
                    v.status
                        .iter()
                        .cloned()
                        .map(|mut s| {
                            s.status_text = calm_status(&s.status_text);
                            s
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
        })
    });
    let last_apply = Memo::new(move |_| view_data.with(|v| v.as_ref().and_then(|v| v.last_apply.clone())));
    let service_names = Memo::new(move |_| {
        spec.with(|s| {
            s.as_ref()
                .map(|s| s.services.iter().map(|x| x.name.clone()).collect::<Vec<_>>())
                .unwrap_or_default()
        })
    });

    let selected = RwSignal::new(None::<String>);
    Effect::new(move |_| {
        let names = service_names.get();
        if spec.with(Option::is_none) {
            return;
        }
        let current = selected.get_untracked();
        let still_there = current
            .as_ref()
            .is_some_and(|c| c == NEW || names.contains(c));
        if !still_there {
            selected.set(Some(
                names.first().cloned().unwrap_or_else(|| NEW.to_string()),
            ));
        }
    });

    let editors = RwSignal::new(HashMap::<String, Editor>::new());
    let unsaved = Memo::new(move |_| {
        let mut list: Vec<String> = editors.with(|all| {
            all.iter()
                .filter(|(_, e)| e.dirty.get())
                .map(|(n, _)| n.clone())
                .collect()
        });
        list.sort();
        list
    });
    let bar_problem = RwSignal::new(None::<String>);
    let pending = StoredValue::new(None::<PendingSave>);

    // Saves every service with unsaved edits in one go, then deploys if asked.
    let save_all = move |deploy: bool| {
        bar_problem.set(None);
        let Some(mut next) = spec.get_untracked() else { return };
        let mut changes = Vec::new();
        let mut services = Vec::new();
        for service in unsaved.get_untracked() {
            let Some(editor) = editors.with_untracked(|all| all.get(&service).copied()) else { continue };
            let Some(build) = editor.build.get_value() else { continue };
            match build.run(()) {
                Ok((built, env)) => {
                    editor.problem.set(None);
                    if let Some(slot) = next.services.iter_mut().find(|s| s.name == built.name) {
                        *slot = built;
                    }
                    changes.extend(env);
                    services.push(service);
                }
                Err(message) => {
                    editor.problem.set(Some(message.clone()));
                    selected.set(Some(service.clone()));
                    bar_problem.set(Some(format!("{service}: {message}")));
                    return;
                }
            }
        }
        if let Err(p) = check_spec(&next) {
            bar_problem.set(Some(p.0));
            return;
        }
        if services.is_empty() {
            if deploy {
                apply.dispatch(ApplyStack { name: name.get_untracked() });
            }
            return;
        }
        pending.set_value(Some(PendingSave { spec: next.clone(), services, deploy }));
        save.dispatch(SaveStack { spec: next, changes });
    };

    Effect::new(move |_| {
        let result = save.value().get();
        if result.is_none() {
            return;
        }
        let Some(done) = pending.get_value() else { return };
        pending.set_value(None);
        match result {
            Some(Ok(())) => {
                // Load what was saved back into each form: new env values become masked.
                for service in &done.services {
                    let editor = editors.with_untracked(|all| all.get(service).copied());
                    let saved = done.spec.services.iter().find(|s| &s.name == service).cloned();
                    if let (Some(editor), Some(saved)) = (editor, saved) {
                        if let Some(reload) = editor.reload.get_value() {
                            reload.run(saved);
                        }
                        editor.dirty.set(false);
                    }
                }
                if done.deploy {
                    apply.dispatch(ApplyStack { name: name.get_untracked() });
                }
            }
            Some(Err(e)) => bar_problem.set(Some(e.to_string())),
            None => {}
        }
    });

    let discard_all = move || {
        bar_problem.set(None);
        let saved = spec.get_untracked();
        for service in unsaved.get_untracked() {
            let Some(editor) = editors.with_untracked(|all| all.get(&service).copied()) else { continue };
            let original = saved
                .as_ref()
                .and_then(|s| s.services.iter().find(|x| x.name == service).cloned());
            if let (Some(original), Some(reload)) = (original, editor.reload.get_value()) {
                reload.run(original);
            }
            editor.problem.set(None);
            editor.dirty.set(false);
        }
    };

    let on_added = Callback::new(move |service: String| selected.set(Some(service)));

    view! {
        <div class="page page--stack">
            <nav class="crumbs" aria-label="Breadcrumb">
                <a href="/stacks">"Stacks"</a>
                <span aria-hidden="true">"/"</span>
                <span>{move || name.get()}</span>
            </nav>

            <Show
                when=move || spec.with(Option::is_some)
                fallback=move || {
                    view! {
                        <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                            {move || stack.get().and_then(Result::err).map(|error| view! { <DockerProblem error /> })}
                        </Transition>
                    }
                }
            >
                <header class="page__head">
                    <div class="page__titles">
                        <div class="service-head__row">
                            <h1 class="page__title">{move || name.get()}</h1>
                            {move || state.get().map(|s| view! { <StateBadge state=s /> })}
                        </div>
                        <p class="page__meta"><span class="page__meta-mono">{move || folder.get()}</span></p>
                    </div>
                    <Show when=move || deployed.get()>
                        <a class="act" href=move || format!("/p/{}", name.get())>"Open project"</a>
                    </Show>
                </header>

                {move || refresh_error().map(|e| view! {
                    <p class="stale-note" role="status">
                        {format!("Couldn't refresh ({e}). Showing the last known state; your edits are safe.")}
                    </p>
                })}

                <div class="workbench">
                    <nav class="workbench__list" aria-label="Services">
                        <span class="workbench__label">"SERVICES"</span>
                        <For each=move || service_names.get() key=|n| n.clone() let:service>
                            {
                                let pick = service.clone();
                                let is_current = service.clone();
                                let for_dot = service.clone();
                                let for_dirty = service.clone();
                                let for_image = service.clone();
                                let dot = move || {
                                    status.with(|all| {
                                        all.iter()
                                            .find(|s| s.service == for_dot)
                                            .map(|s| s.health.class())
                                            .unwrap_or("is-idle")
                                    })
                                };
                                let image = move || {
                                    spec.with(|s| {
                                        s.as_ref()
                                            .and_then(|s| s.services.iter().find(|x| x.name == for_image))
                                            .map(|x| x.image.clone())
                                            .unwrap_or_default()
                                    })
                                };
                                view! {
                                    <button
                                        type="button"
                                        class="workbench__item"
                                        class:is-active=move || selected.get().as_deref() == Some(is_current.as_str())
                                        on:click=move |_| selected.set(Some(pick.clone()))
                                    >
                                        <span class="workbench__name">
                                            <span class=move || format!("dot {}", dot())></span>
                                            {service.clone()}
                                            <Show when=move || unsaved.with(|u| u.contains(&for_dirty))>
                                                <span class="workbench__edited" title="Unsaved changes">"edited"</span>
                                            </Show>
                                        </span>
                                        <span class="workbench__meta">{image}</span>
                                    </button>
                                }
                            }
                        </For>
                        <button
                            type="button"
                            class="workbench__item workbench__item--add"
                            class:is-active=move || selected.get().as_deref() == Some(NEW)
                            on:click=move |_| selected.set(Some(NEW.to_string()))
                        >
                            <Ico icon=icondata::LuPlus />
                            "Add service"
                        </button>
                    </nav>

                    <section class="workbench__main">
                        <Show when=move || selected.get().as_deref() == Some(NEW)>
                            <AddService spec save images on_added />
                        </Show>
                        <For
                            each=move || service_names.get()
                            key=|n| n.clone()
                            children=move |service: String| {
                                let initial = spec.with_untracked(|s| {
                                    s.as_ref().and_then(|s| s.services.iter().find(|x| x.name == service).cloned())
                                });
                                let Some(initial) = initial else { return ().into_any() };
                                let editor = Editor::new();
                                editors.update(|all| {
                                    all.insert(service.clone(), editor);
                                });
                                let gone = service.clone();
                                on_cleanup(move || {
                                    editors.try_update(|all| {
                                        all.remove(&gone);
                                    });
                                });
                                let for_active = service.clone();
                                let active = Signal::derive(move || selected.get().as_deref() == Some(for_active.as_str()));
                                let for_status = service.clone();
                                let service_status = Signal::derive(move || {
                                    status.with(|all| all.iter().find(|s| s.service == for_status).cloned())
                                });
                                view! {
                                    <div class:is-hidden=move || !active.get()>
                                        <ServiceInspector
                                            stack_name=name.get_untracked()
                                            initial
                                            latest=spec
                                            deployed
                                            status=service_status
                                            active
                                            save
                                            images
                                            editor
                                        />
                                    </div>
                                }
                                .into_any()
                            }
                        />
                    </section>
                </div>

                <div class="changes" class:is-dirty=move || !unsaved.with(Vec::is_empty) class:is-failed=move || bar_problem.with(Option::is_some)>
                    {move || {
                        let has_services = !service_names.with(Vec::is_empty);
                        let busy = save.pending().get() || apply.pending().get();
                        let edited = unsaved.get();
                        if let Some(p) = bar_problem.get() {
                            return view! {
                                <span class="changes__dot is-down"></span>
                                <span class="changes__text">{p}</span>
                                <button type="button" class="btn btn--ghost" on:click=move |_| bar_problem.set(None)>"OK"</button>
                            }
                            .into_any();
                        }
                        if !edited.is_empty() {
                            let text = if edited.len() == 1 {
                                format!("Unsaved changes in {}", edited[0])
                            } else {
                                format!("Unsaved changes in {}", edited.join(", "))
                            };
                            let saving = save.pending().get();
                            let deploying = apply.pending().get();
                            return view! {
                                <span class="changes__dot is-warn"></span>
                                <span class="changes__text">{text}</span>
                                <button type="button" class="btn btn--ghost" disabled=busy on:click=move |_| discard_all()>
                                    "Discard"
                                </button>
                                <button type="button" class="btn btn--secondary" disabled=busy on:click=move |_| save_all(false)>
                                    {if saving { "Saving…" } else { "Save" }}
                                </button>
                                <button type="button" class="btn btn--primary" disabled=busy on:click=move |_| save_all(true)>
                                    {if deploying { "Deploying…" } else { "Save & deploy" }}
                                </button>
                            }
                            .into_any();
                        }
                        // "Applied" is about the files; whether the containers actually run is separate.
                        let down = status.with(|all| all.iter().filter(|s| !s.running).count());
                        let failed = last_apply.with(|r| r.as_ref().is_some_and(|r| !r.ok));
                        let (dot, text, action): (&str, String, Option<&'static str>) = match state.get() {
                            _ if apply.pending().get() => ("is-warn", "Deploying…".into(), None),
                            Some(StackState::Pending | StackState::NotDeployed) if failed && has_services => ("is-down", "The last deploy failed. Fix it and deploy again.".into(), Some("Deploy again")),
                            Some(StackState::Pending) => ("is-warn", "Saved, but not live yet".into(), Some("Deploy")),
                            Some(StackState::NotDeployed) if has_services => ("is-neutral", "Not deployed yet".into(), Some("Deploy")),
                            Some(StackState::Deployed) if down == 1 => ("is-warn", "Deployed, but 1 service isn't running. Check its logs.".into(), Some("Redeploy")),
                            Some(StackState::Deployed) if down > 1 => ("is-warn", format!("Deployed, but {down} services aren't running. Check their logs."), Some("Redeploy")),
                            Some(StackState::Deployed) => ("is-ok", "Everything is live and running".into(), None),
                            _ => ("is-neutral", "Add a service to get started".into(), None),
                        };
                        view! {
                            <span class=format!("changes__dot {dot}")></span>
                            <span class="changes__text">{text}</span>
                            {action.map(|label| {
                                view! {
                                    <button
                                        type="button"
                                        class="btn btn--primary"
                                        disabled=busy
                                        on:click=move |_| save_all(true)
                                    >
                                        {label}
                                    </button>
                                }
                            })}
                        }
                        .into_any()
                    }}
                </div>

                // Survives refreshes: the reason for a failed deploy stays until the next one works.
                {move || {
                    if apply.pending().get() {
                        return None;
                    }
                    last_apply.get().map(|record| {
                        if record.ok {
                            view! {
                                <p class="last-deploy is-ok">{format!("Last deployed {}", record.at)}</p>
                            }
                            .into_any()
                        } else {
                            view! {
                                <section class="last-deploy is-failed" role="alert">
                                    <header class="last-deploy__head">
                                        <span class="dot is-down"></span>
                                        <strong>{format!("Last deploy failed · {}", record.at)}</strong>
                                        <span class="last-deploy__hint">"Your changes are saved. Fix what Docker says below, then deploy again."</span>
                                    </header>
                                    <pre class="last-deploy__output">{record.output}</pre>
                                </section>
                            }
                            .into_any()
                        }
                    })
                }}

                <details class="compose">
                    <summary>"Generated compose.yml"</summary>
                    <pre>{move || compose.get().unwrap_or_default()}</pre>
                </details>
            </Show>
        </div>
    }
}

#[component]
fn AddService(
    spec: Memo<Option<shared::stack::StackSpec>>,
    save: ServerAction<SaveStack>,
    images: Resource<Result<Vec<String>, shared::errors::docker::DockerError>>,
    on_added: Callback<String>,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let image = RwSignal::new(String::new());
    let problem = RwSignal::new(None::<String>);
    let adding = RwSignal::new(None::<String>);

    Effect::new(move |_| {
        let result = save.value().get();
        let Some(added) = adding.get_untracked() else {
            return;
        };
        match result {
            Some(Ok(())) => {
                adding.set(None);
                on_added.run(added);
            }
            Some(Err(e)) => {
                adding.set(None);
                problem.set(Some(e.to_string()));
            }
            None => {}
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let Some(mut next) = spec.get_untracked() else {
            return;
        };
        next.services.push(ServiceSpec {
            name: name.get_untracked().trim().to_string(),
            image: image.get_untracked(),
            restart: Default::default(),
            ports: Vec::new(),
            volumes: Vec::new(),
            env: Vec::new(),
            command: Vec::new(),
            user: None,
        });
        match check_spec(&next) {
            Err(p) => problem.set(Some(p.0)),
            Ok(()) => {
                problem.set(None);
                adding.set(Some(name.get_untracked().trim().to_string()));
                name.set(String::new());
                save.dispatch(SaveStack {
                    spec: next,
                    changes: Vec::new(),
                });
            }
        }
    };

    view! {
        <form class="add-service" on:submit=on_submit>
            <h2 class="inspector__title">"Add a service"</h2>
            <label class="field create__field">
                <span class="field__label">"SERVICE NAME"</span>
                <input
                    class="field__input field__input--plain"
                    type="text"
                    placeholder="web"
                    maxlength="40"
                    autocomplete="off"
                    spellcheck="false"
                    prop:value=move || name.get()
                    on:input=move |ev| name.set(event_target_value(&ev).to_lowercase())
                />
            </label>
            <label class="field create__field create__field--wide">
                <span class="field__label">"IMAGE"</span>
                <Transition fallback=|| view! { <select class="field__input field__input--plain" disabled=true></select> }>
                    {move || {
                        let list = images.get().and_then(Result::ok).unwrap_or_default();
                        let empty = list.is_empty();
                        view! {
                            <select
                                class="field__input field__input--plain"
                                prop:value=move || image.get()
                                on:change=move |ev| image.set(event_target_value(&ev))
                            >
                                <option value="">{if empty { "No images on this server" } else { "Pick an installed image" }}</option>
                                {list.into_iter().map(|i| view! { <option value=i.clone()>{i.clone()}</option> }).collect_view()}
                            </select>
                        }
                    }}
                </Transition>
            </label>
            <button
                class="btn btn--secondary"
                type="submit"
                disabled=move || name.get().trim().is_empty() || image.get().is_empty() || save.pending().get()
            >
                <Ico icon=icondata::LuPlus />
                "Add service"
            </button>
            <p class="create__hint">
                {move || problem.get().unwrap_or_else(|| "Only images already on this server are listed. Ports, storage and env come right after.".into())}
            </p>
        </form>
    }
}
