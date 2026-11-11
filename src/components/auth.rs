use leptos::prelude::*;
use shared::admin::Admin;

use crate::api::auth::{current_admin, LogoutUser};
use crate::components::icon::Ico;

// Blocking: the server resolves this before sending HTML, and the browser reuses that value,
// so the signed-in UI never flickers.
#[derive(Clone, Copy)]
pub struct AuthState {
    admin: Resource<Option<Admin>>,
    version: RwSignal<u32>,
}

impl AuthState {
    pub fn provide() {
        let version = RwSignal::new(0);
        let admin = Resource::new_blocking(
            move || version.get(),
            |_| async { current_admin().await.ok().flatten() },
        );
        provide_context(Self { admin, version });
    }

    pub fn get() -> Self {
        expect_context::<Self>()
    }

    pub fn admin(&self) -> Option<Option<Admin>> {
        self.admin.get()
    }

    pub fn refresh(&self) {
        self.version.update(|v| *v += 1);
    }
}

#[component]
pub fn LogoutButton() -> impl IntoView {
    let auth = AuthState::get();
    let logout = ServerAction::<LogoutUser>::new();

    Effect::new(move |_| {
        if logout.value().get().is_some() {
            auth.refresh();
        }
    });

    view! {
        <button
            class="icon-btn"
            type="button"
            title="Log out"
            aria-label="Log out"
            disabled=move || logout.pending().get()
            on:click=move |_| {
                logout.dispatch(LogoutUser {});
            }
        >
            <Ico icon=icondata::LuLogOut />
        </button>
    }
}
