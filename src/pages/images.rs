use std::collections::BTreeMap;

use leptos::prelude::*;
use shared::images::{HubResult, LocalImage, PullEvent, is_docker_hub, valid_repo, valid_tag};

use super::common::DockerProblem;
use crate::api::images::{get_local_images, hub_tags, search_hub, DeleteImage, SearchHub};
use crate::components::icon::Ico;
use crate::components::poll::{watch_session, Tick};
use crate::components::sse::{connect, SseEvent};
use crate::components::stats::bytes;

#[component]
pub fn ImagesPage() -> impl IntoView {
    let tick = Tick::signal();
    let remove = ServerAction::<DeleteImage>::new();
    let refresh = RwSignal::new(0u32);
    let images = Resource::new(
        move || (tick.get(), remove.version().get(), refresh.get()),
        |_| get_local_images(),
    );
    watch_session(move || images.get().and_then(Result::err));

    let removed_error = move || remove.value().get().and_then(Result::err).map(|e| e.to_string());

    view! {
        <div class="page">
            <header class="page__head">
                <div class="page__titles">
                    <h1 class="page__title">"Images"</h1>
                    <p class="page__lead">"What's on this server, and Docker Hub to get more. New images show up in Add service right away."</p>
                </div>
            </header>

            <Store on_pulled=Callback::new(move |_: ()| refresh.update(|r| *r += 1)) />

            <h2 class="label board-label">"On this server"</h2>
            {move || removed_error().map(|e| view! { <p class="flash is-err" role="alert">{e}</p> })}
            <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                {move || {
                    images
                        .get()
                        .map(|result| match result {
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                            Ok(list) if list.is_empty() => {
                                view! { <p class="tab-empty">"No images yet. Search Docker Hub above."</p> }.into_any()
                            }
                            Ok(list) => view! { <LocalTable list remove /> }.into_any(),
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn LocalTable(list: Vec<LocalImage>, remove: ServerAction<DeleteImage>) -> impl IntoView {
    let total: i64 = list.iter().map(|i| i.size).sum();
    // Which row is asking "Remove?" right now; only one at a time.
    let confirming = RwSignal::new(None::<String>);
    view! {
        <section class="table images-table" aria-label="Images on this server">
            <div class="table__row table__row--head" aria-hidden="true">
                <span>"Image"</span>
                <span>"Size"</span>
                <span>"Created"</span>
                <span>"Used by"</span>
                <span></span>
            </div>
            {list
                .into_iter()
                .map(|image| {
                    let reference = image.reference.clone();
                    let asking = image.reference.clone();
                    let confirm_ref = image.reference.clone();
                    let used = image.used_by;
                    let is_asking = move || confirming.with(|c| c.as_deref() == Some(asking.as_str()));
                    view! {
                        <div class="table__row">
                            <span class="table__name">
                                <Ico icon=icondata::LuPackage />
                                <span class="images-table__ref">{image.reference.clone()}</span>
                            </span>
                            <span class="table__mono">{bytes(image.size as f64)}</span>
                            <span class="table__mono">{image.created.clone()}</span>
                            <span class="table__note" class:is-calm=used == 0>
                                {match used {
                                    0 => "not used".to_string(),
                                    1 => "1 container".to_string(),
                                    n => format!("{n} containers"),
                                }}
                            </span>
                            <span class="table__actions">
                                <Show
                                    when=is_asking.clone()
                                    fallback=move || {
                                        let reference = reference.clone();
                                        let hint = if used > 0 {
                                            "In use. Remove the containers that use it first."
                                        } else {
                                            "Remove this image from the server"
                                        };
                                        view! {
                                            <button
                                                type="button"
                                                class="icon-btn"
                                                title=hint
                                                aria-label="Remove image"
                                                disabled={move || used > 0 || remove.pending().get()}
                                                on:click=move |_| confirming.set(Some(reference.clone()))
                                            >
                                                <Ico icon=icondata::LuTrash2 />
                                            </button>
                                        }
                                    }
                                >
                                    <span class="images-table__confirm">
                                        "Remove? "
                                        <button
                                            type="button"
                                            class="tool-btn is-danger"
                                            on:click={
                                                let confirm_ref = confirm_ref.clone();
                                                move |_| {
                                                    confirming.set(None);
                                                    remove.dispatch(DeleteImage { reference: confirm_ref.clone() });
                                                }
                                            }
                                        >
                                            "Remove"
                                        </button>
                                        <button type="button" class="tool-btn" on:click=move |_| confirming.set(None)>
                                            "Cancel"
                                        </button>
                                    </span>
                                </Show>
                            </span>
                        </div>
                    }
                })
                .collect_view()}
            <div class="table__row images-table__total">
                <span class="table__note is-calm">"Total"</span>
                <span class="table__mono">{bytes(total as f64)}</span>
            </div>
        </section>
    }
}

#[derive(Clone, PartialEq)]
enum PullState {
    Idle,
    Running { status: String, percent: Option<f64> },
    Done(String),
    Failed(String),
}

#[component]
fn Store(on_pulled: Callback<()>) -> impl IntoView {
    let search = ServerAction::<SearchHub>::new();
    let term = RwSignal::new(String::new());
    let picked = RwSignal::new(None::<String>);
    let tag = RwSignal::new("latest".to_string());
    let pull_state = RwSignal::new(PullState::Idle);
    let layers = RwSignal::new(BTreeMap::<String, (i64, i64)>::new());
    let stream = StoredValue::new(None::<crate::components::sse::SseHandle>);

    let tags = Resource::new(move || picked.get(), |repo| async move {
        match repo {
            Some(repo) if is_docker_hub(&repo) => Some(hub_tags(repo).await),
            _ => None,
        }
    });

    let on_search = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let typed = term.get_untracked().trim().to_lowercase();
        if typed.is_empty() {
            return;
        }
        // "name:tag" typed straight in: skip the search and go to the pull step.
        if let Some((repo, t)) = typed.split_once(':').filter(|(r, t)| valid_repo(r) && valid_tag(t)) {
            picked.set(Some(repo.to_string()));
            tag.set(t.to_string());
            return;
        }
        picked.set(None);
        search.dispatch(SearchHub { term: typed });
    };

    let start_pull = move |_| {
        let Some(repo) = picked.get_untracked() else { return };
        let t = tag.get_untracked().trim().to_string();
        if !valid_repo(&repo) || !valid_tag(&t) {
            pull_state.set(PullState::Failed("Pick a valid tag first.".into()));
            return;
        }
        layers.set(BTreeMap::new());
        pull_state.set(PullState::Running { status: "Starting…".into(), percent: None });
        let label = format!("{repo}:{t}");
        let url = format!("/api/pull?image={repo}&tag={t}");
        let handle = connect(&url, move |event| match event {
            SseEvent::Data(json) => {
                let Ok(step) = serde_json::from_str::<PullEvent>(&json) else { return };
                if step.done {
                    match step.error {
                        Some(e) => pull_state.set(PullState::Failed(e)),
                        None => {
                            pull_state.set(PullState::Done(format!("{label} is on the server.")));
                            on_pulled.run(());
                        }
                    }
                    return;
                }
                if let Some(e) = step.error {
                    pull_state.set(PullState::Failed(e));
                    return;
                }
                if let (Some(layer), Some(current), Some(total)) = (step.layer.clone(), step.current, step.total) {
                    if total > 0 {
                        layers.update(|l| {
                            l.insert(layer, (current.min(total), total));
                        });
                    }
                }
                let percent = layers.with_untracked(|l| {
                    let (done, all) = l.values().fold((0i64, 0i64), |(d, a), (c, t)| (d + c, a + t));
                    (all > 0).then(|| done as f64 / all as f64 * 100.0)
                });
                let status = match step.layer {
                    Some(layer) if !step.status.is_empty() => format!("{} {}", step.status, layer),
                    _ => step.status,
                };
                pull_state.set(PullState::Running { status, percent });
            }
            SseEvent::Failure(message) => pull_state.set(PullState::Failed(message)),
            SseEvent::Lost { closed: true } => {
                if matches!(pull_state.get_untracked(), PullState::Running { .. }) {
                    pull_state.set(PullState::Failed("Lost the connection. The pull may still finish on the server.".into()));
                }
            }
            SseEvent::End | SseEvent::Open | SseEvent::Lost { .. } => {}
        });
        stream.set_value(handle);
    };

    let pulling = move || matches!(pull_state.get(), PullState::Running { .. });

    view! {
        <section class="store" aria-label="Docker Hub">
            <form class="store__search" on:submit=on_search>
                <label class="store__field">
                    <Ico icon=icondata::LuSearch />
                    <span class="sr-only">"Search Docker Hub"</span>
                    <input
                        type="search"
                        placeholder="Search Docker Hub, or type an image like redis:7-alpine"
                        autocomplete="off"
                        spellcheck="false"
                        prop:value=move || term.get()
                        on:input=move |ev| term.set(event_target_value(&ev))
                    />
                </label>
                <button class="btn btn--primary" type="submit" disabled=move || search.pending().get()>
                    {move || if search.pending().get() { "Searching…" } else { "Search" }}
                </button>
            </form>

            {move || {
                picked.get().map(|repo| {
                    view! {
                        <div class="store__pull">
                            <div class="store__pull-head">
                                <span class="store__pull-name">{repo.clone()}</span>
                                <button type="button" class="tool-btn" on:click=move |_| picked.set(None)>"Back to results"</button>
                            </div>
                            <div class="store__pull-row">
                                <label class="field">
                                    <span class="field__label">"TAG"</span>
                                    <input
                                        class="field__input field__input--plain"
                                        type="text"
                                        spellcheck="false"
                                        prop:value=move || tag.get()
                                        on:input=move |ev| tag.set(event_target_value(&ev))
                                    />
                                </label>
                                <button type="button" class="btn btn--primary" disabled=pulling on:click=start_pull>
                                    <Ico icon=icondata::LuArrowDownToLine />
                                    {move || if pulling() { "Pulling…" } else { "Pull to server" }}
                                </button>
                            </div>
                            <Transition fallback=|| view! { <p class="store__hint">"Loading tags…"</p> }>
                                {move || match tags.get() {
                                    Some(Some(Ok(list))) => view! {
                                        <div class="store__tags">
                                            {list
                                                .into_iter()
                                                .map(|t| {
                                                    let name = t.name.clone();
                                                    let is_current = t.name.clone();
                                                    let size = t.size.map(|s| bytes(s as f64)).unwrap_or_default();
                                                    view! {
                                                        <button
                                                            type="button"
                                                            class="store__tag"
                                                            class:is-active=move || tag.get() == is_current
                                                            title=t.updated.clone().map(|u| format!("Updated {u}")).unwrap_or_default()
                                                            on:click=move |_| tag.set(name.clone())
                                                        >
                                                            <span>{t.name.clone()}</span>
                                                            <small>{size}</small>
                                                        </button>
                                                    }
                                                })
                                                .collect_view()}
                                        </div>
                                    }
                                    .into_any(),
                                    Some(Some(Err(e))) => view! { <p class="store__hint">{e.to_string()}</p> }.into_any(),
                                    _ => view! { <p class="store__hint">"Not a Docker Hub image, so type the tag yourself."</p> }.into_any(),
                                }}
                            </Transition>
                            {move || match pull_state.get() {
                                PullState::Idle => ().into_any(),
                                PullState::Running { status, percent } => view! {
                                    <div class="store__progress">
                                        <div class="meter"><span class="meter__fill" style=format!("width: {:.0}%", percent.unwrap_or(4.0))></span></div>
                                        <span class="store__status">
                                            {percent.map(|p| format!("{p:.0}% · ")).unwrap_or_default()}
                                            {status}
                                        </span>
                                    </div>
                                }
                                .into_any(),
                                PullState::Done(text) => view! { <p class="flash is-ok">{text}</p> }.into_any(),
                                PullState::Failed(text) => view! { <p class="flash is-err">{text}</p> }.into_any(),
                            }}
                        </div>
                    }
                })
            }}

            <Show when=move || picked.with(Option::is_none)>
                {move || match search.value().get() {
                    None => view! {
                        <p class="store__hint">"Official images (postgres, redis, surrealdb/surrealdb, …) are marked. Pulled images are ready for Add service."</p>
                    }
                    .into_any(),
                    Some(Err(e)) => view! { <p class="flash is-err">{e.to_string()}</p> }.into_any(),
                    Some(Ok(results)) if results.is_empty() => view! { <p class="store__hint">"Nothing found."</p> }.into_any(),
                    Some(Ok(results)) => view! {
                        <ul class="store__results">
                            {results.into_iter().map(|r| view! { <ResultRow result=r picked tag pull_state /> }).collect_view()}
                        </ul>
                    }
                    .into_any(),
                }}
            </Show>
        </section>
    }
}

#[component]
fn ResultRow(
    result: HubResult,
    picked: RwSignal<Option<String>>,
    tag: RwSignal<String>,
    pull_state: RwSignal<PullState>,
) -> impl IntoView {
    let name = result.name.clone();
    view! {
        <li>
            <button
                type="button"
                class="store__result"
                on:click=move |_| {
                    tag.set("latest".into());
                    pull_state.set(PullState::Idle);
                    picked.set(Some(name.clone()));
                }
            >
                <span class="store__result-head">
                    <strong>{result.name.clone()}</strong>
                    {result.official.then(|| view! { <span class="badge is-ok">"Official"</span> })}
                    <span class="store__stars">{format!("★ {}", result.stars)}</span>
                </span>
                <span class="store__desc">{result.description.clone()}</span>
            </button>
        </li>
    }
}
