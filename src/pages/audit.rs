use leptos::prelude::*;
use shared::docker::AuditEntry;

use super::common::DockerProblem;
use crate::api::audit::get_audit;
use crate::components::icon::Ico;
use crate::components::poll::{watch_session, Tick};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Filter {
    All,
    Problems,
    Containers,
    Access,
}

impl Filter {
    fn keeps(self, e: &AuditEntry) -> bool {
        match self {
            Self::All => true,
            Self::Problems => e.failed(),
            Self::Containers => is_change(e),
            Self::Access => !is_change(e),
        }
    }
}

fn is_change(e: &AuditEntry) -> bool {
    ["container.", "stack.", "env.", "image."].iter().any(|p| e.action.starts_with(p))
}

#[component]
pub fn AuditPage() -> impl IntoView {
    let tick = Tick::signal();
    let entries = Resource::new(move || tick.get(), |_| get_audit());
    watch_session(move || entries.get().and_then(Result::err));

    let filter = RwSignal::new(Filter::All);
    let search = RwSignal::new(String::new());

    let visible = move |all: Vec<AuditEntry>| {
        let needle = search.get().trim().to_lowercase();
        let f = filter.get();
        all.into_iter()
            .filter(|e| f.keeps(e))
            .filter(|e| {
                needle.is_empty()
                    || [Some(&e.action), e.actor.as_ref(), e.target.as_ref(), e.ip.as_ref(), e.detail.as_ref()]
                        .into_iter()
                        .flatten()
                        .any(|v| v.to_lowercase().contains(&needle))
                    || e.describe().to_lowercase().contains(&needle)
            })
            .collect::<Vec<_>>()
    };

    view! {
        <div class="page">
            <header class="page__head">
                <div class="page__titles">
                    <h1 class="page__title">"Audit log"</h1>
                    <p class="page__lead">
                        "Every sign-in and container action, newest first. Entries can't be edited or deleted."
                    </p>
                </div>
            </header>

            <div class="toolbar">
                <div class="segmented" role="tablist">
                    <FilterTab filter which=Filter::All label="All" />
                    <FilterTab filter which=Filter::Problems label="Problems" />
                    <FilterTab filter which=Filter::Containers label="Changes" />
                    <FilterTab filter which=Filter::Access label="Sign-ins" />
                </div>
                <label class="search search--wide">
                    <Ico icon=icondata::LuSearch />
                    <span class="sr-only">"Search the audit log"</span>
                    <input
                        type="search"
                        placeholder="Search events, IPs, containers"
                        prop:value=move || search.get()
                        on:input=move |ev| search.set(event_target_value(&ev))
                    />
                </label>
            </div>

            <Transition fallback=|| view! { <div class="skeleton skeleton--block"></div> }>
                {move || {
                    entries
                        .get()
                        .map(|result| match result {
                            Err(error) => view! { <DockerProblem error /> }.into_any(),
                            Ok(all) => {
                                let rows = visible(all);
                                if rows.is_empty() {
                                    return view! {
                                        <div class="empty-state empty-state--small">
                                            <span class="empty-state__icon"><Ico icon=icondata::LuScrollText /></span>
                                            <h2>"Nothing matches"</h2>
                                            <p>"Try another filter."</p>
                                        </div>
                                    }
                                    .into_any();
                                }
                                view! {
                                    <div class="audit">
                                        <div class="audit__row audit__row--head" aria-hidden="true">
                                            <span>"TIME"</span>
                                            <span>"EVENT"</span>
                                            <span>"TARGET"</span>
                                            <span>"BY"</span>
                                            <span>"FROM"</span>
                                        </div>
                                        {rows.into_iter().map(|e| view! { <AuditRow entry=e /> }).collect_view()}
                                    </div>
                                }
                                .into_any()
                            }
                        })
                }}
            </Transition>
        </div>
    }
}

#[component]
fn FilterTab(filter: RwSignal<Filter>, which: Filter, label: &'static str) -> impl IntoView {
    view! {
        <button
            type="button"
            role="tab"
            class="segmented__item"
            class:is-active=move || filter.get() == which
            aria-selected=move || (filter.get() == which).to_string()
            on:click=move |_| filter.set(which)
        >
            {label}
        </button>
    }
}

#[component]
fn AuditRow(entry: AuditEntry) -> impl IntoView {
    let failed = entry.failed();
    let icon = if is_change(&entry) {
        icondata::LuBox
    } else if entry.action.starts_with("login") || entry.action == "logout" {
        icondata::LuLogIn
    } else {
        icondata::LuKeyRound
    };

    view! {
        <div class="audit__row" class:is-failed=failed>
            <span class="audit__time">{entry.at.clone()}</span>
            <span class="audit__event">
                <span class="audit__icon" class:is-failed=failed><Ico icon=icon /></span>
                <span class="audit__what">
                    <strong>{entry.describe()}</strong>
                    {entry.detail.clone().map(|d| view! { <span class="audit__detail">{d}</span> })}
                </span>
            </span>
            <span class="audit__mono" title=entry.target.clone().unwrap_or_default()>
                {entry.target.clone().unwrap_or_else(|| "—".into())}
            </span>
            <span class="audit__actor">{entry.actor.clone().unwrap_or_else(|| "—".into())}</span>
            <span class="audit__mono">{entry.ip.clone().unwrap_or_else(|| "—".into())}</span>
        </div>
    }
}
