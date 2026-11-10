#![recursion_limit = "256"]

pub mod api;
pub mod app;
mod components;
mod pages;

#[cfg(feature = "ssr")]
pub mod helpers;
#[cfg(feature = "ssr")]
pub mod middleware;
#[cfg(feature = "ssr")]
pub mod sse;
#[cfg(feature = "ssr")]
pub mod startup;
#[cfg(feature = "ssr")]
pub mod state;

#[cfg(feature = "hydrate")]
#[wasm_bindgen::prelude::wasm_bindgen]
pub fn hydrate() {
    use crate::app::*;
    console_error_panic_hook::set_once();
    leptos::mount::hydrate_body(App);
}
