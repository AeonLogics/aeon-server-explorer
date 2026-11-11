use leptos::prelude::*;
use shared::docker::{Action, ServiceDetail};

use crate::api::docker::ContainerAction;
use crate::components::auth::AuthState;
use crate::components::icon::Ico;

#[component]
pub fn ActionBar(
    #[prop(into)] detail: Signal<Option<ServiceDetail>>,
    dialog: RwSignal<Option<Action>>,
    on_done: Callback<Action>,
) -> impl IntoView {
    let auth = AuthState::get();
    let act = ServerAction::<ContainerAction>::new();
    let last = RwSignal::new(None::<Action>);
    let flash = RwSignal::new(None::<(bool, String)>);
    let typed = RwSignal::new(String::new());

    let container = move || {
        detail.with_untracked(|d| d.as_ref().map(|d| d.card.container_name.clone()).unwrap_or_default())
    };
    // A container stuck restarting counts as active: Stop is what breaks the loop.
    let running = move || {
        detail.with(|d| d.as_ref().is_some_and(|d| matches!(d.card.state.as_str(), "running" | "restarting")))
    };
    let blocker = move || detail.with(|d| d.as_ref().and_then(|d| d.redeploy_blocker.clone()));
    let is_self = move || detail.with(|d| d.as_ref().is_some_and(|d| d.is_self));
    let busy = move || act.pending().get();

    let dispatch = move |action: Action, confirm: Option<String>| {
        last.set(Some(action));
        flash.set(None);
        act.dispatch(ContainerAction {
            container: container(),
            action,
            confirm,
        });
    };

    Effect::new(move |_| {
        let Some(result) = act.value().get() else { return };
        let Some(action) = last.get_untracked() else { return };
        let message = match result {
            Ok(()) => {
                on_done.run(action);
                (true, format!("{} {}", action.done(), container()))
            }
            Err(e) => {
                if e.ends_session() {
                    auth.refresh();
                }
                (false, e.to_string())
            }
        };
        flash.set(Some(message));
        set_timeout(move || flash.set(None), std::time::Duration::from_secs(6));
    });

    let open = move |action: Action| {
        typed.set(String::new());
        dialog.set(Some(action));
    };

    let confirm = move |_| {
        let Some(action) = dialog.get_untracked() else { return };
        let confirm = action.needs_typed_name().then(|| typed.get_untracked());
        dialog.set(None);
        dispatch(action, confirm);
    };

    let button_label = move |action: Action| {
        if busy() && last.get() == Some(action) {
            match action {
                Action::Start => "Starting…",
                Action::Stop => "Stopping…",
                Action::Restart => "Restarting…",
                Action::Redeploy => "Redeploying…",
            }
        } else {
            action.label()
        }
    };

    view! {
        <div class="actions" class:is-busy=busy>
            <Show
                when=running
                fallback=move || {
                    view! {
                        <button
                            type="button"
                            class="act act--go"
                            disabled=busy
                            on:click=move |_| dispatch(Action::Start, None)
                        >
                            <Ico icon=icondata::LuPlay />
                            {move || button_label(Action::Start)}
                        </button>
                    }
                }
            >
                <button type="button" class="act" disabled=busy on:click=move |_| open(Action::Restart)>
                    <Ico icon=icondata::LuRotateCcw />
                    {move || button_label(Action::Restart)}
                </button>
                <Show when=move || !is_self()>
                    <button type="button" class="act act--danger" disabled=busy on:click=move |_| open(Action::Stop)>
                        <Ico icon=icondata::LuSquare />
                        {move || button_label(Action::Stop)}
                    </button>
                </Show>
            </Show>
            <button
                type="button"
                class="act"
                disabled=move || busy() || blocker().is_some()
                title=move || blocker().unwrap_or_else(|| "Pull the newest image and recreate with docker compose".into())
                on:click=move |_| open(Action::Redeploy)
            >
                <Ico icon=icondata::LuRefreshCw />
                {move || button_label(Action::Redeploy)}
            </button>

            {move || {
                flash
                    .get()
                    .map(|(ok, text)| {
                        view! {
                            <span class="actions__flash" class:is-ok=ok class:is-err=!ok role="status">
                                {if ok {
                                    view! { <Ico icon=icondata::LuCircleCheck /> }.into_any()
                                } else {
                                    view! { <Ico icon=icondata::LuCircleAlert /> }.into_any()
                                }}
                                {text}
                            </span>
                        }
                    })
            }}
        </div>

        {move || {
            dialog
                .get()
                .map(|action| {
                    let name = container();
                    let service = detail
                        .with_untracked(|d| d.as_ref().and_then(|d| d.service.clone()))
                        .unwrap_or_else(|| name.clone());
                    let (title, body) = match action {
                        Action::Restart => (
                            format!("Restart {name}?"),
                            "It stops and starts again. Expect a few seconds of downtime.".to_string(),
                        ),
                        Action::Stop => (
                            format!("Stop {name}?"),
                            "It stays offline until you start it again, and anything that depends on it will fail.".to_string(),
                        ),
                        Action::Redeploy => (
                            format!("Redeploy {service}?"),
                            "Pulls the newest image for its tag and recreates the container with docker compose. Volumes are kept; expect a short downtime.".to_string(),
                        ),
                        Action::Start => (format!("Start {name}?"), String::new()),
                    };
                    let needs_name = action.needs_typed_name();
                    let expected = name.clone();
                    let can_confirm = move || !needs_name || typed.get().trim() == expected;
                    view! {
                        <div class="modal-scrim" on:click=move |_| dialog.set(None)></div>
                        <div class="modal" role="dialog" aria-modal="true" aria-labelledby="modal-title">
                            <span class={if action == Action::Stop { "modal__icon is-danger" } else { "modal__icon" }}>
                                {match action {
                                    Action::Stop => view! { <Ico icon=icondata::LuSquare /> }.into_any(),
                                    Action::Redeploy => view! { <Ico icon=icondata::LuRefreshCw /> }.into_any(),
                                    _ => view! { <Ico icon=icondata::LuRotateCcw /> }.into_any(),
                                }}
                            </span>
                            <h2 id="modal-title" class="modal__title">{title}</h2>
                            <p class="modal__body">{body}</p>
                            <Show when=move || needs_name>
                                <label class="field">
                                    <span class="field__label">
                                        "TYPE " <code class="modal__name">{name.clone()}</code> " TO CONFIRM"
                                    </span>
                                    <input
                                        class="field__input field__input--plain"
                                        type="text"
                                        autocomplete="off"
                                        spellcheck="false"
                                        autofocus=true
                                        prop:value=move || typed.get()
                                        on:input=move |ev| typed.set(event_target_value(&ev))
                                    />
                                </label>
                            </Show>
                            <div class="modal__actions">
                                <button type="button" class="btn btn--ghost" on:click=move |_| dialog.set(None)>
                                    "Cancel"
                                </button>
                                <button
                                    type="button"
                                    class={if action == Action::Stop { "btn btn--danger" } else { "btn btn--primary" }}
                                    disabled=move || !can_confirm()
                                    on:click=confirm
                                >
                                    {action.label()}
                                </button>
                            </div>
                        </div>
                    }
                })
        }}
    }
}
