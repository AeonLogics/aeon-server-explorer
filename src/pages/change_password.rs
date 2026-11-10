use leptos::prelude::*;
use shared::errors::{auth::AuthError, FieldError};
use shared::inputs::{ChangePasswordInput, MIN_PASSWORD};

use super::login::{AuthFrame, RevealToggle};
use crate::api::auth::{ChangePassword, LogoutUser};
use crate::components::auth::AuthState;
use crate::components::icon::Ico;

#[component]
pub fn ChangePasswordPage(email: String) -> impl IntoView {
    let auth = AuthState::get();
    let change = ServerAction::<ChangePassword>::new();
    let logout = ServerAction::<LogoutUser>::new();

    let current = RwSignal::new(String::new());
    let next = RwSignal::new(String::new());
    let confirm = RwSignal::new(String::new());
    let reveal = RwSignal::new(false);

    let length = move || next.with(|p| p.chars().count());
    let long_enough = move || length() >= MIN_PASSWORD;
    let matches = move || !confirm.get().is_empty() && confirm.get() == next.get();
    let mismatch = move || !confirm.get().is_empty() && confirm.get() != next.get();
    let pending = move || change.pending().get();
    let can_submit =
        move || !pending() && !current.get().is_empty() && long_enough() && matches();

    let error = move || change.value().get().and_then(Result::err);
    let general_error = move || {
        error().map(|e| match e {
            AuthError::Invalid(FieldError::NewPassword) => FieldError::NewPassword.to_string(),
            other => other.to_string(),
        })
    };

    Effect::new(move |_| {
        let changed = matches!(change.value().get(), Some(Ok(_)));
        let left = logout.value().get().is_some();
        let expired = matches!(change.value().get(), Some(Err(AuthError::Unauthorized)));
        if changed || left || expired {
            auth.refresh();
        }
    });

    let on_submit = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        change.dispatch(ChangePassword {
            input: ChangePasswordInput {
                current_password: current.get_untracked(),
                new_password: next.get_untracked(),
            },
        });
    };

    let meter = move || (length() as f64 / MIN_PASSWORD as f64 * 100.0).min(100.0);
    let kind = move || if reveal.get() { "text" } else { "password" };

    view! {
        <AuthFrame>
            <h1 class="auth__title">"Choose your own password"</h1>
            <p class="auth__lead">
                "The starting password came from "<code>"ADMIN_PASSWORD"</code>". Replace it before you continue, "
                <strong>{email}</strong>"."
            </p>

            <form class="auth__form" on:submit=on_submit>
                <div class="field">
                    <label class="field__label" for="pw-current">"CURRENT PASSWORD"</label>
                    <div class="field__control">
                        <Ico icon=icondata::LuKeyRound />
                        <input
                            id="pw-current"
                            class="field__input"
                            type=kind
                            autocomplete="current-password"
                            maxlength="128"
                            prop:value=move || current.get()
                            on:input=move |ev| current.set(event_target_value(&ev))
                        />
                        <RevealToggle reveal />
                    </div>
                </div>

                <div class="field">
                    <label class="field__label" for="pw-new">"NEW PASSWORD"</label>
                    <div class="field__control">
                        <Ico icon=icondata::LuFingerprint />
                        <input
                            id="pw-new"
                            class="field__input"
                            type=kind
                            autocomplete="new-password"
                            maxlength="128"
                            prop:value=move || next.get()
                            on:input=move |ev| next.set(event_target_value(&ev))
                        />
                    </div>
                    <div class="meter" class:is-done=long_enough aria-hidden="true">
                        <span class="meter__fill" style=move || format!("width: {:.0}%", meter())></span>
                    </div>
                    <p class="field__hint">
                        {move || {
                            let n = length();
                            if n >= MIN_PASSWORD {
                                format!("{n} characters. Good.")
                            } else {
                                format!("{n} of at least {MIN_PASSWORD} characters. A short sentence works well.")
                            }
                        }}
                    </p>
                </div>

                <div class="field" class:is-invalid=mismatch>
                    <label class="field__label" for="pw-confirm">"REPEAT NEW PASSWORD"</label>
                    <div class="field__control">
                        <Ico icon=icondata::LuShieldCheck />
                        <input
                            id="pw-confirm"
                            class="field__input"
                            type=kind
                            autocomplete="new-password"
                            maxlength="128"
                            prop:value=move || confirm.get()
                            on:input=move |ev| confirm.set(event_target_value(&ev))
                        />
                    </div>
                    <Show when=mismatch>
                        <p class="field__error">"The two new passwords don't match."</p>
                    </Show>
                </div>

                <Show when=move || general_error().is_some()>
                    <p class="alert" role="alert">
                        <Ico icon=icondata::LuCircleAlert />
                        {move || general_error().unwrap_or_default()}
                    </p>
                </Show>

                <button class="btn btn--primary btn--block btn--lg" type="submit" disabled=move || !can_submit()>
                    {move || if pending() { "Saving…" } else { "Save and continue" }}
                </button>
                <button
                    class="btn btn--ghost btn--block"
                    type="button"
                    on:click=move |_| {
                        logout.dispatch(LogoutUser {});
                    }
                >
                    "Log out"
                </button>
            </form>
        </AuthFrame>
    }
}
