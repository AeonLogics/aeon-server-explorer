use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use shared::stack::{check_spec, valid_name, ServiceSpec, StackState, StackSummary};

use super::common::DockerProblem;
use super::service_editor::ServiceInspector;
use crate::api::stacks::{get_images, get_stack, get_stacks, ApplyStack, CreateStack, SaveStack};
use crate::components::icon::Ico;
use crate::components::poll::{watch_session, Tick};

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

    // The workbench lives outside the 5-second refresh: these memos only notify when the
    // data really changed, so nothing you're typing gets rebuilt.
    let spec = Memo::new(move |_| stack.get().and_then(Result::ok).map(|v| v.spec));
    let compose = Memo::new(move |_| stack.get().and_then(Result::ok).map(|v| v.compose));
    let folder = Memo::new(move |_| {
        stack
            .get()
            .and_then(Result::ok)
            .map(|v| v.folder)
            .unwrap_or_default()
    });
    let state = Memo::new(move |_| stack.get().and_then(Result::ok).map(|v| v.state));
    let deployed =
        Signal::derive(move || state.get().is_some_and(|s| s != StackState::NotDeployed));
    let status = Memo::new(move |_| {
        stack
            .get()
            .and_then(Result::ok)
            .map(|v| v.status)
            .unwrap_or_default()
    });

    let selected = RwSignal::new(None::<String>);
    let selected_status = Signal::derive(move || {
        let current = selected.get();
        status.with(|all| {
            all.iter()
                .find(|s| Some(&s.service) == current.as_ref())
                .cloned()
        })
    });
    let dirty = RwSignal::new(false);
    let save_now = RwSignal::new(0u32);
    let epoch = RwSignal::new(0u32);

    let selected_spec = Memo::new(move |_| {
        let current = selected.get();
        spec.with(|s| {
            s.as_ref().and_then(|s| {
                s.services
                    .iter()
                    .find(|x| Some(&x.name) == current.as_ref())
                    .cloned()
            })
        })
    });

    Effect::new(move |_| {
        let names: Option<Vec<String>> = spec.with(|s| {
            s.as_ref()
                .map(|s| s.services.iter().map(|x| x.name.clone()).collect())
        });
        let Some(names) = names else { return };
        let current = selected.get_untracked();
        let still_there = current
            .as_ref()
            .is_some_and(|c| c == NEW || names.contains(c));
        if !still_there {
            dirty.set(false);
            selected.set(Some(
                names.first().cloned().unwrap_or_else(|| NEW.to_string()),
            ));
        }
    });

    // With unsaved edits, switching is refused and the changes bar asks for a decision.
    let nudge = RwSignal::new(false);
    let choose = move |next: String| {
        if selected.get_untracked().as_deref() == Some(next.as_str()) {
            return;
        }
        if dirty.get_untracked() {
            nudge.set(true);
            set_timeout(move || nudge.set(false), std::time::Duration::from_millis(1800));
            return;
        }
        selected.set(Some(next));
    };

    let on_added = Callback::new(move |service: String| {
        dirty.set(false);
        selected.set(Some(service));
    });
    let on_saved = Callback::new(move |_: ()| epoch.update(|e| *e += 1));

    let last_apply = Memo::new(move |_| stack.get().and_then(Result::ok).and_then(|v| v.last_apply));

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

                <div class="workbench">
                    <nav class="workbench__list" aria-label="Services">
                        <span class="workbench__label">"SERVICES"</span>
                        {move || {
                            spec.with(|s| s.as_ref().map(|s| s.services.clone()))
                                .unwrap_or_default()
                                .into_iter()
                                .map(|svc| {
                                    let pick = svc.name.clone();
                                    let is_current = svc.name.clone();
                                    let for_dot = svc.name.clone();
                                    let dot = move || {
                                        status.with(|all| {
                                            all.iter()
                                                .find(|s| s.service == for_dot)
                                                .map(|s| s.health.class())
                                                .unwrap_or("is-idle")
                                        })
                                    };
                                    view! {
                                        <button
                                            type="button"
                                            class="workbench__item"
                                            class:is-active=move || selected.get().as_deref() == Some(is_current.as_str())
                                            on:click=move |_| choose(pick.clone())
                                        >
                                            <span class="workbench__name">
                                                <span class=move || format!("dot {}", dot())></span>
                                                {svc.name.clone()}
                                            </span>
                                            <span class="workbench__meta">{svc.image.clone()}</span>
                                        </button>
                                    }
                                })
                                .collect_view()
                        }}
                        <button
                            type="button"
                            class="workbench__item workbench__item--add"
                            class:is-active=move || selected.get().as_deref() == Some(NEW)
                            on:click=move |_| choose(NEW.to_string())
                        >
                            <Ico icon=icondata::LuPlus />
                            "Add service"
                        </button>
                    </nav>

                    <section class="workbench__main">
                        {move || {
                            epoch.track();
                            selected_spec.track();
                            let current = selected.get()?;
                            if current == NEW {
                                return Some(view! { <AddService spec save images on_added /> }.into_any());
                            }
                            let stack = spec.get_untracked()?;
                            Some(
                                view! {
                                    <ServiceInspector
                                        stack
                                        latest=spec
                                        service=current
                                        deployed
                                        status=selected_status
                                        save
                                        images
                                        dirty
                                        save_now
                                        on_saved
                                    />
                                }
                                .into_any(),
                            )
                        }}
                    </section>
                </div>

                <div class="changes" class:is-dirty=move || dirty.get() class:is-nudged=move || nudge.get()>
                    {move || {
                        let has_services = spec.with(|s| s.as_ref().is_some_and(|s| !s.services.is_empty()));
                        let busy = save.pending().get() || apply.pending().get();
                        if dirty.get() {
                            return view! {
                                <span class="changes__dot is-warn"></span>
                                <span class="changes__text">
                                    {move || {
                                        let service = selected.get().unwrap_or_default();
                                        if nudge.get() {
                                            format!("Save or discard your changes to {service} first")
                                        } else {
                                            format!("Unsaved changes to {service}")
                                        }
                                    }}
                                </span>
                                <button
                                    type="button"
                                    class="btn btn--ghost"
                                    on:click=move |_| {
                                        dirty.set(false);
                                        epoch.update(|e| *e += 1);
                                    }
                                >
                                    "Discard"
                                </button>
                                <button
                                    type="button"
                                    class="btn btn--primary"
                                    disabled=busy
                                    on:click=move |_| save_now.update(|n| *n += 1)
                                >
                                    {if save.pending().get() { "Saving…" } else { "Save" }}
                                </button>
                            }
                            .into_any();
                        }
                        // "Applied" is about the files; whether the containers actually run is separate.
                        let down = status.with(|all| all.iter().filter(|s| !s.running).count());
                        let (dot, text, action): (&str, String, Option<&'static str>) = match state.get() {
                            Some(StackState::Pending) => ("is-warn", "Saved, but not live yet".into(), Some("Apply changes")),
                            Some(StackState::NotDeployed) if has_services => ("is-neutral", "Not deployed yet".into(), Some("Deploy")),
                            Some(StackState::Deployed) if down == 1 => ("is-warn", "Applied, but 1 service isn't running. Check its logs.".into(), None),
                            Some(StackState::Deployed) if down > 1 => ("is-warn", format!("Applied, but {down} services aren't running. Check their logs."), None),
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
                                        on:click=move |_| {
                                            apply.dispatch(ApplyStack { name: name.get_untracked() });
                                        }
                                    >
                                        {if apply.pending().get() { "Deploying…" } else { label }}
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
                                        <span class="last-deploy__hint">"Fix it, Save, then Apply again. Your saved changes are kept."</span>
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
