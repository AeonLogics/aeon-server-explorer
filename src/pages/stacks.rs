use leptos::prelude::*;
use leptos_router::hooks::{use_location, use_navigate};
use leptos_router::NavigateOptions;
use shared::stack::{check_spec, valid_name, ServiceSpec, StackState, StackSummary, StackView};

use super::common::DockerProblem;
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
    let stacks = Resource::new(move || (tick.get(), create.version().get()), |_| get_stacks());
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
            create.dispatch(CreateStack { name: name.get_untracked().trim().to_string() });
        }
    };
    let error = move || create.value().get().and_then(Result::err).map(|e| e.to_string());

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
        move || (name.get(), tick.get(), save.version().get(), apply.version().get()),
        |(n, ..)| get_stack(n),
    );
    let images = Resource::new(|| (), |_| get_images());
    watch_session(move || stack.get().and_then(Result::err));

    let message = move || {
        let saved = save.value().get().and_then(Result::err).map(|e| (false, e.to_string()));
        let applied = apply.value().get().map(|r| match r {
            Ok(()) => (true, "Deployed. The containers are starting.".to_string()),
            Err(e) => (false, e.to_string()),
        });
        applied.or(saved)
    };

    view! {
        <div class="page">
            <nav class="crumbs" aria-label="Breadcrumb">
                <a href="/stacks">"Stacks"</a>
                <span aria-hidden="true">"/"</span>
                <span>{move || name.get()}</span>
            </nav>
            <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                {move || {
                    stack
                        .get()
                        .map(|result| match result {
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                            Ok(data) => view! { <StackBody data save apply images /> }.into_any(),
                        })
                }}
            </Transition>
            {move || {
                message()
                    .map(|(ok, text)| {
                        view! { <p class="flash" class:is-ok=ok class:is-err=!ok role="status">{text}</p> }
                    })
            }}
        </div>
    }
}

#[component]
fn StackBody(
    data: StackView,
    save: ServerAction<SaveStack>,
    apply: ServerAction<ApplyStack>,
    images: Resource<Result<Vec<String>, shared::errors::docker::DockerError>>,
) -> impl IntoView {
    let spec = StoredValue::new(data.spec.clone());
    let deployed = data.state != StackState::NotDeployed;
    let stack_name = data.spec.name.clone();
    let can_apply = !data.spec.services.is_empty() && data.state != StackState::Deployed;
    let apply_label = match data.state {
        StackState::NotDeployed => "Deploy",
        StackState::Pending => "Apply changes",
        StackState::Deployed => "Up to date",
    };

    let remove = move |service: String| {
        let mut next = spec.get_value();
        next.services.retain(|s| s.name != service);
        save.dispatch(SaveStack { spec: next, changes: Vec::new() });
    };

    view! {
        <header class="page__head">
            <div class="page__titles">
                <div class="service-head__row">
                    <h1 class="page__title">{stack_name.clone()}</h1>
                    <StateBadge state=data.state.clone() />
                </div>
                <p class="page__meta"><span class="page__meta-mono">{data.folder.clone()}</span></p>
            </div>
            <div class="actions">
                {deployed.then(|| view! {
                    <a class="act" href=format!("/p/{stack_name}")>"Open project"</a>
                })}
                <button
                    type="button"
                    class="btn btn--primary"
                    disabled=move || !can_apply || apply.pending().get() || save.pending().get()
                    on:click={
                        let name = data.spec.name.clone();
                        move |_| {
                            apply.dispatch(ApplyStack { name: name.clone() });
                        }
                    }
                >
                    {move || if apply.pending().get() { "Deploying…" } else { apply_label }}
                </button>
            </div>
        </header>

        <section class="table" aria-label="Services">
            <div class="table__row table__row--head table__row--services" aria-hidden="true">
                <span>"Service"</span>
                <span>"Image"</span>
                <span>"Restart"</span>
                <span></span>
            </div>
            {if data.spec.services.is_empty() {
                view! { <p class="table__empty">"No services yet. Add the first one below."</p> }.into_any()
            } else {
                data.spec
                    .services
                    .iter()
                    .cloned()
                    .map(|s| {
                        let service = s.name.clone();
                        view! {
                            <div class="table__row table__row--services">
                                <span class="table__name"><span>{s.name.clone()}</span></span>
                                <span class="table__mono">{s.image.clone()}</span>
                                <span class="table__note is-calm">{s.restart.label()}</span>
                                <button
                                    type="button"
                                    class="icon-btn"
                                    title="Remove from the stack (applies on the next deploy)"
                                    aria-label=format!("Remove {}", s.name)
                                    disabled=move || save.pending().get()
                                    on:click=move |_| remove(service.clone())
                                >
                                    <Ico icon=icondata::LuX />
                                </button>
                            </div>
                        }
                    })
                    .collect_view()
                    .into_any()
            }}
        </section>

        <AddService spec save images />

        <details class="compose">
            <summary>"Generated compose.yml"</summary>
            <pre>{data.compose.clone()}</pre>
        </details>
    }
}

#[component]
fn AddService(
    spec: StoredValue<shared::stack::StackSpec>,
    save: ServerAction<SaveStack>,
    images: Resource<Result<Vec<String>, shared::errors::docker::DockerError>>,
) -> impl IntoView {
    let name = RwSignal::new(String::new());
    let image = RwSignal::new(String::new());
    let problem = RwSignal::new(None::<String>);

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let mut next = spec.get_value();
        next.services.push(ServiceSpec {
            name: name.get_untracked().trim().to_string(),
            image: image.get_untracked(),
            restart: Default::default(),
            ports: Vec::new(),
            volumes: Vec::new(),
            env: Vec::new(),
        });
        match check_spec(&next) {
            Err(p) => problem.set(Some(p.0)),
            Ok(()) => {
                problem.set(None);
                name.set(String::new());
                save.dispatch(SaveStack { spec: next, changes: Vec::new() });
            }
        }
    };

    view! {
        <form class="create create--service" on:submit=on_submit>
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
                {move || problem.get().unwrap_or_else(|| "Only images already on this server are listed. Ports, storage and env come next.".into())}
            </p>
        </form>
    }
}
