use leptos::prelude::*;
use shared::errors::{auth::AuthError, FieldError};
use shared::inputs::LoginInput;

use crate::api::auth::LoginUser;
use crate::components::auth::AuthState;
use crate::components::icon::Ico;

#[component]
pub fn LoginPage() -> impl IntoView {
    let auth = AuthState::get();
    let login = ServerAction::<LoginUser>::new();

    let email = RwSignal::new(String::new());
    let password = RwSignal::new(String::new());
    let reveal = RwSignal::new(false);

    let pending = move || login.pending().get();
    let error = move || login.value().get().and_then(Result::err);
    let invalid = move |field: FieldError| matches!(error(), Some(AuthError::Invalid(f)) if f == field);
    let general_error = move || error().filter(|e| !matches!(e, AuthError::Invalid(_)));
    let can_submit = move || !pending() && email.get().contains('@') && !password.get().is_empty();

    Effect::new(move |_| {
        if matches!(login.value().get(), Some(Ok(_))) {
            password.set(String::new());
            auth.refresh();
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        login.dispatch(LoginUser {
            input: LoginInput {
                email: email.get_untracked(),
                password: password.get_untracked(),
            },
        });
    };

    view! {
        <AuthFrame>
            <h1 class="auth__title">"Sign in"</h1>
            <p class="auth__lead">"Sign in to see everything running on your server."</p>

            <form class="auth__form" on:submit=on_submit>
                <div class="field" class:is-invalid=move || invalid(FieldError::Email)>
                    <label class="field__label" for="login-email">"EMAIL"</label>
                    <div class="field__control">
                        <Ico icon=icondata::LuMail />
                        <input
                            id="login-email"
                            class="field__input"
                            type="email"
                            autocomplete="username"
                            maxlength="254"
                            placeholder="you@example.com"
                            prop:value=move || email.get()
                            on:input=move |ev| email.set(event_target_value(&ev))
                        />
                    </div>
                    <Show when=move || invalid(FieldError::Email)>
                        <p class="field__error">{FieldError::Email.to_string()}</p>
                    </Show>
                </div>

                <div class="field" class:is-invalid=move || invalid(FieldError::Password)>
                    <label class="field__label" for="login-password">"PASSWORD"</label>
                    <div class="field__control">
                        <Ico icon=icondata::LuKeyRound />
                        <input
                            id="login-password"
                            class="field__input"
                            type=move || if reveal.get() { "text" } else { "password" }
                            autocomplete="current-password"
                            maxlength="128"
                            prop:value=move || password.get()
                            on:input=move |ev| password.set(event_target_value(&ev))
                        />
                        <RevealToggle reveal />
                    </div>
                </div>

                <Show when=move || general_error().is_some()>
                    <p class="alert" role="alert">
                        <Ico icon=icondata::LuCircleAlert />
                        {move || general_error().map(|e| e.to_string()).unwrap_or_default()}
                    </p>
                </Show>

                <button class="btn btn--primary btn--block btn--lg" type="submit" disabled=move || !can_submit()>
                    {move || if pending() { "Signing in…" } else { "Sign in" }}
                </button>
            </form>
        </AuthFrame>
    }
}

#[component]
pub fn RevealToggle(reveal: RwSignal<bool>) -> impl IntoView {
    view! {
        <button
            type="button"
            class="field__reveal"
            aria-label=move || if reveal.get() { "Hide password" } else { "Show password" }
            on:click=move |_| reveal.update(|r| *r = !*r)
        >
            {move || {
                if reveal.get() {
                    view! { <Ico icon=icondata::LuEyeOff /> }.into_any()
                } else {
                    view! { <Ico icon=icondata::LuEye /> }.into_any()
                }
            }}
        </button>
    }
}

#[component]
pub fn AuthFrame(children: Children) -> impl IntoView {
    view! {
        <div class="auth">

            <main class="auth__wrap">
                <div class="auth__brand">
                    <span class="brand__mark brand__mark--lg">
                        <img src="/logo.svg" alt="" width="40" height="28" />
                    </span>
                    <span class="brand__text">
                        <strong>"Server Explorer"</strong>
                        <span class="label">"AEON LOGICS"</span>
                    </span>
                </div>

                <section class="auth__card">{children()}</section>

                <p class="auth__foot">
                    <Ico icon=icondata::LuShieldCheck />
                    "Loopback only · reach it through an SSH tunnel"
                </p>
            </main>
        </div>
    }
}
