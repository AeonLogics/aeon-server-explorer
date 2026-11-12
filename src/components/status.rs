use leptos::prelude::*;
use shared::docker::{Health, ProjectDetail, ProjectSummary};

// Docker's status counts seconds ("Up 45 seconds", "Restarting (1) 10 seconds ago"), so it's
// different on every refresh and would redraw the page every 5 seconds. Under a minute is
// all a person needs to know; minutes and hours pass through unchanged.
pub fn calm_status(text: &str) -> String {
    let words: Vec<&str> = text.split(' ').collect();
    let mut out: Vec<&str> = Vec::with_capacity(words.len());
    let mut i = 0;
    while i < words.len() {
        let next = words.get(i + 1).copied().unwrap_or_default();
        let counts_seconds = matches!(next, "second" | "seconds")
            && (words[i].parse::<u32>().is_ok() || words[i].eq_ignore_ascii_case("a"));
        if counts_seconds {
            out.extend(["less", "than", "a", "minute"]);
            i += 2;
        } else if words[i].eq_ignore_ascii_case("less") && next == "than" && words.get(i + 3) == Some(&"second") {
            out.extend(["less", "than", "a", "minute"]);
            i += 4;
        } else {
            out.push(words[i]);
            i += 1;
        }
    }
    out.join(" ")
}

pub fn calm_projects(mut projects: Vec<ProjectSummary>) -> Vec<ProjectSummary> {
    for m in projects.iter_mut().flat_map(|p| p.members.iter_mut()) {
        m.status_text = calm_status(&m.status_text);
    }
    projects
}

pub fn calm_detail(mut detail: ProjectDetail) -> ProjectDetail {
    for m in &mut detail.summary.members {
        m.status_text = calm_status(&m.status_text);
    }
    for card in &mut detail.services {
        card.status_text = calm_status(&card.status_text);
    }
    detail
}

#[component]
pub fn StatusDot(health: Health) -> impl IntoView {
    view! { <span class=format!("dot {}", health.class()) aria-hidden="true"></span> }
}

#[component]
pub fn HealthBadge(health: Health, label: String) -> impl IntoView {
    view! {
        <span class=format!("badge {}", health.class())>
            <StatusDot health />
            {label}
        </span>
    }
}
