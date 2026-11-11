use leptos::prelude::*;
use leptos_icons::Icon;

#[component]
pub fn Ico(icon: icondata::Icon, #[prop(optional)] class: &'static str) -> impl IntoView {
    view! {
        <span class=format!("ico {class}") aria-hidden="true">
            <Icon icon=icon />
        </span>
    }
}
