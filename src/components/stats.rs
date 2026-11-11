use leptos::prelude::*;
use shared::docker::StatsSample;

use crate::components::icon::Ico;
use crate::components::sparkline::Sparkline;
use crate::components::sse::{connect, SseEvent};

const HISTORY: usize = 60;

#[derive(Clone, Copy, PartialEq)]
enum Feed {
    Connecting,
    Live,
    Stopped,
}

#[component]
pub fn StatsStrip(container: String) -> impl IntoView {
    let samples = RwSignal::new(Vec::<StatsSample>::new());
    let feed = RwSignal::new(Feed::Connecting);

    let url = format!("/api/stats/{container}");
    Effect::new(move |_| {
        let handle = connect(&url, move |event| match event {
            SseEvent::Open => feed.set(Feed::Live),
            SseEvent::Data(json) => {
                if let Ok(sample) = serde_json::from_str::<StatsSample>(&json) {
                    samples.update(|s| {
                        s.push(sample);
                        if s.len() > HISTORY {
                            s.remove(0);
                        }
                    });
                }
            }
            SseEvent::Failure(_) | SseEvent::End | SseEvent::Lost { closed: true } => {
                feed.set(Feed::Stopped)
            }
            SseEvent::Lost { closed: false } => feed.set(Feed::Connecting),
        });
        on_cleanup(move || drop(handle));
    });

    let latest = move || samples.with(|s| s.last().copied());
    let cpu_values = Signal::derive(move || samples.with(|s| s.iter().map(|x| x.cpu).collect::<Vec<f64>>()));
    let mem_values =
        Signal::derive(move || samples.with(|s| s.iter().map(|x| x.mem as f64).collect::<Vec<f64>>()));
    let cpu_max = Signal::derive(move || {
        let peak = samples.with(|s| s.iter().map(|x| x.cpu).fold(0.0_f64, f64::max));
        nice_ceiling(peak.max(5.0))
    });
    let mem_max = Signal::derive(move || {
        latest()
            .map(|s| s.mem_limit as f64)
            .filter(|l| *l > 0.0)
            .unwrap_or_else(|| samples.with(|s| s.iter().map(|x| x.mem as f64).fold(1.0, f64::max)))
    });

    let cpu_text = move || latest().map(|s| format!("{:.1}", s.cpu)).unwrap_or_else(|| "—".into());
    let cores = move || {
        latest()
            .map(|s| format!("of {} core{}", s.cpus, if s.cpus == 1 { "" } else { "s" }))
            .unwrap_or_default()
    };
    let mem_text = move || latest().map(|s| bytes(s.mem as f64)).unwrap_or_else(|| "—".into());
    let mem_of = move || {
        latest()
            .filter(|s| s.mem_limit > 0)
            .map(|s| format!("of {}", bytes(s.mem_limit as f64)))
            .unwrap_or_default()
    };
    let mem_pct = move || {
        latest()
            .filter(|s| s.mem_limit > 0)
            .map(|s| (s.mem as f64 / s.mem_limit as f64 * 100.0).clamp(0.0, 100.0))
            .unwrap_or(0.0)
    };

    view! {
        <div class="stats" class:is-stopped=move || feed.get() == Feed::Stopped>
            <div class="stat-tile">
                <div class="stat-tile__head">
                    <span class="stat-tile__label"><Ico icon=icondata::LuCpu />"CPU"</span>
                    <span class="stat-tile__live" class:is-live=move || feed.get() == Feed::Live>
                        {move || match feed.get() {
                            Feed::Live => "LIVE",
                            Feed::Connecting => "CONNECTING",
                            Feed::Stopped => "STOPPED",
                        }}
                    </span>
                </div>
                <div class="stat-tile__value">
                    <strong>{cpu_text}</strong><span class="stat-tile__unit">"%"</span>
                    <span class="stat-tile__sub">{cores}</span>
                </div>
                <Sparkline values=cpu_values max=cpu_max capacity=HISTORY format=fmt_cpu label="CPU usage, last minute" />
            </div>

            <div class="stat-tile">
                <div class="stat-tile__head">
                    <span class="stat-tile__label"><Ico icon=icondata::LuMemoryStick />"Memory"</span>
                    <span class="stat-tile__pct">{move || format!("{:.0}%", mem_pct())}</span>
                </div>
                <div class="stat-tile__value">
                    <strong>{mem_text}</strong>
                    <span class="stat-tile__sub">{mem_of}</span>
                </div>
                <Sparkline values=mem_values max=mem_max capacity=HISTORY format=fmt_bytes label="Memory used, last minute" />
            </div>
        </div>
    }
}

fn fmt_cpu(v: f64) -> String {
    format!("{v:.1}% CPU")
}

fn fmt_bytes(v: f64) -> String {
    bytes(v)
}

fn nice_ceiling(v: f64) -> f64 {
    [5.0, 10.0, 25.0, 50.0, 100.0, 200.0, 400.0, 800.0, 1600.0]
        .into_iter()
        .find(|step| v <= *step)
        .unwrap_or(v)
}

pub fn bytes(v: f64) -> String {
    const UNITS: [&str; 5] = ["B", "KB", "MB", "GB", "TB"];
    let mut value = v;
    let mut unit = 0;
    while value >= 1024.0 && unit < UNITS.len() - 1 {
        value /= 1024.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{value:.0} {}", UNITS[unit])
    } else {
        format!("{value:.1} {}", UNITS[unit])
    }
}
