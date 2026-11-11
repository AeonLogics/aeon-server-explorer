use leptos::prelude::*;
use shared::docker::{network_hue, PortInfo};

use crate::components::icon::Ico;

#[component]
pub fn NetworkChip(name: String) -> impl IntoView {
    let hue = network_hue(&name);
    view! {
        <span class="chip chip--net" style=format!("--hue: {hue}") title=format!("Network {name}")>
            <span class="chip__swatch" aria-hidden="true"></span>
            {name.clone()}
        </span>
    }
}

#[component]
pub fn PortChip(port: PortInfo) -> impl IntoView {
    let (class, icon, title) = match (port.host_port, port.exposed) {
        (Some(_), true) => (
            "chip chip--port is-exposed",
            icondata::LuTriangleAlert,
            "Published on every interface. Bind it to 127.0.0.1 unless the LAN really needs it.",
        ),
        (Some(_), false) => (
            "chip chip--port",
            icondata::LuLock,
            "Published on localhost only",
        ),
        (None, _) => (
            "chip chip--port is-internal",
            icondata::LuWaypoints,
            "Not published; reachable inside its networks",
        ),
    };
    view! {
        <span class=class title=title>
            <Ico icon=icon />
            {port.label()}
        </span>
    }
}
