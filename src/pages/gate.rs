use leptos::prelude::*;
use leptos_router::components::Outlet;
use leptos_router::hooks::use_location;
use shared::admin::Admin;
use shared::docker::Overview;
use shared::errors::docker::DockerError;

use super::change_password::ChangePasswordPage;
use super::login::LoginPage;
use super::sidebar::Sidebar;
use crate::api::docker::get_overview;
use crate::components::auth::AuthState;
use crate::components::icon::Ico;
use crate::components::poll::{watch_session, Tick};

#[derive(Clone, Copy)]
pub struct OverviewData(pub Resource<Result<Overview, DockerError>>);

impl OverviewData {
    pub fn get() -> Resource<Result<Overview, DockerError>> {
        expect_context::<Self>().0
    }
}

#[component]
pub fn Gate() -> impl IntoView {
    let auth = AuthState::get();

    view! {
        <Transition fallback=|| view! { <div class="boot"></div> }>
            {move || match auth.admin() {
                Some(Some(admin)) if admin.must_change_password => {
                    view! { <ChangePasswordPage email=admin.email /> }.into_any()
                }
                Some(Some(admin)) => view! { <AppShell admin /> }.into_any(),
                Some(None) => view! { <LoginPage /> }.into_any(),
                None => view! { <div class="boot"></div> }.into_any(),
            }}
        </Transition>
    }
}

#[component]
fn AppShell(admin: Admin) -> impl IntoView {
    let tick = Tick::provide();
    let overview = Resource::new(move || tick.get(), |_| get_overview());
    provide_context(OverviewData(overview));
    watch_session(move || overview.get().and_then(Result::err));

    let nav_open = RwSignal::new(false);
    let location = use_location();
    Effect::new(move |_| {
        location.pathname.track();
        nav_open.set(false);
    });

    view! {
        <div class="app" class:is-nav-open=move || nav_open.get()>
            <header class="topbar">
                <button
                    type="button"
                    class="icon-btn"
                    aria-label="Open navigation"
                    on:click=move |_| nav_open.update(|o| *o = !*o)
                >
                    <Ico icon=icondata::LuMenu />
                </button>
                <a class="topbar__brand" href="/">
                    <img src="/logo.svg" alt="" width="28" height="20" />
                    "Server Explorer"
                </a>
            </header>
            <div class="scrim" on:click=move |_| nav_open.set(false)></div>
            <Sidebar admin />
            <main class="main" id="main">
                <Outlet />
            </main>
        </div>
    }
}
