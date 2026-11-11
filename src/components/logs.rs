use leptos::html::Div;
use leptos::prelude::*;

use crate::components::icon::Ico;

const MAX_LINES: usize = 3_000;

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    seq: u64,
    ts: String,
    err: bool,
    text: String,
}

#[derive(Debug, Clone, PartialEq)]
enum Feed {
    Connecting,
    Live,
    Reconnecting,
    Ended,
    Failed(String),
}

#[component]
pub fn LogViewer(container: String, #[prop(optional)] reconnect: Option<Callback<()>>) -> impl IntoView {
    let lines = RwSignal::new(Vec::<Line>::new());
    let held = StoredValue::new(Vec::<Line>::new());
    let held_count = RwSignal::new(0usize);
    let paused = RwSignal::new(false);
    let follow = RwSignal::new(true);
    let wrap = RwSignal::new(true);
    let filter = RwSignal::new(String::new());
    let feed = RwSignal::new(Feed::Connecting);
    let scroller = NodeRef::<Div>::new();

    let scroll_to_end = move || {
        request_animation_frame(move || {
            if let Some(el) = scroller.get_untracked() {
                el.set_scroll_top(el.scroll_height());
            }
        });
    };

    let append = move |mut batch: Vec<Line>| {
        lines.update(|all| {
            all.append(&mut batch);
            if all.len() > MAX_LINES {
                let extra = all.len() - MAX_LINES;
                all.drain(..extra);
            }
        });
        if follow.get_untracked() {
            scroll_to_end();
        }
    };

    let url = format!("/api/logs/{container}");
    Effect::new(move |_| {
        use std::cell::{Cell, RefCell};
        use std::rc::Rc;

        use crate::components::sse::{connect, SseEvent};

        let queue = Rc::new(RefCell::new(Vec::<Line>::new()));
        let scheduled = Rc::new(Cell::new(false));
        let seq = Rc::new(Cell::new(0u64));

        // Lines arrive in bursts (300 at once on connect), so render them in batches.
        let flush = {
            let queue = queue.clone();
            let scheduled = scheduled.clone();
            move || {
                scheduled.set(false);
                let batch: Vec<Line> = queue.borrow_mut().drain(..).collect();
                if batch.is_empty() {
                    return;
                }
                if paused.get_untracked() {
                    held.update_value(|h| h.extend(batch));
                    held_count.set(held.with_value(Vec::len));
                } else {
                    append(batch);
                }
            }
        };

        let handle = connect(&url, move |event| match event {
            SseEvent::Open => {
                lines.set(Vec::new());
                held.set_value(Vec::new());
                held_count.set(0);
                feed.set(Feed::Live);
            }
            SseEvent::Data(json) => {
                let Ok(line) = serde_json::from_str::<shared::docker::LogLine>(&json) else {
                    return;
                };
                seq.set(seq.get() + 1);
                queue.borrow_mut().push(Line {
                    seq: seq.get(),
                    ts: line.ts,
                    err: line.err,
                    text: line.text,
                });
                if !scheduled.get() {
                    scheduled.set(true);
                    let flush = flush.clone();
                    set_timeout(flush, std::time::Duration::from_millis(60));
                }
            }
            SseEvent::Failure(message) => feed.set(Feed::Failed(message)),
            SseEvent::End => feed.set(Feed::Ended),
            SseEvent::Lost { closed: true } => {
                feed.set(Feed::Failed("The log stream was closed.".into()))
            }
            SseEvent::Lost { closed: false } => feed.set(Feed::Reconnecting),
        });
        on_cleanup(move || drop(handle));
    });

    let toggle_pause = move |_| {
        if paused.get_untracked() {
            paused.set(false);
            let batch = held.with_value(|h| h.clone());
            held.set_value(Vec::new());
            held_count.set(0);
            append(batch);
        } else {
            paused.set(true);
        }
    };

    let on_scroll = move |_| {
        if let Some(el) = scroller.get_untracked() {
            let gap = el.scroll_height() - el.scroll_top() - el.client_height();
            follow.set(gap < 24);
        }
    };

    let jump_to_end = move |_| {
        follow.set(true);
        scroll_to_end();
    };

    let visible = move || {
        let needle = filter.get().to_lowercase();
        lines.with(|all| {
            if needle.is_empty() {
                all.clone()
            } else {
                all.iter()
                    .filter(|l| l.text.to_lowercase().contains(&needle))
                    .cloned()
                    .collect()
            }
        })
    };

    let status = move || match feed.get() {
        Feed::Connecting => ("is-warn", "Connecting…".to_string()),
        Feed::Live if paused.get() => ("is-warn", "Paused".to_string()),
        Feed::Live => ("is-ok", "Live".to_string()),
        Feed::Reconnecting => ("is-warn", "Reconnecting…".to_string()),
        Feed::Ended => ("is-down", "Container stopped · stream ended".to_string()),
        Feed::Failed(message) => ("is-down", message),
    };

    view! {
        <div class="logs" class:is-nowrap=move || !wrap.get()>
            <div class="logs__bar">
                <span class="logs__status">
                    <span class=move || format!("dot {}", status().0)></span>
                    {move || status().1}
                </span>
                {reconnect.map(|reconnect| {
                    view! {
                        <Show when=move || matches!(feed.get(), Feed::Ended | Feed::Failed(_))>
                            <button type="button" class="tool-btn" on:click=move |_| reconnect.run(())>
                                <Ico icon=icondata::LuRefreshCw />
                                "Reconnect"
                            </button>
                        </Show>
                    }
                })}

                <label class="logs__filter">
                    <Ico icon=icondata::LuSearch />
                    <span class="sr-only">"Filter log lines"</span>
                    <input
                        type="search"
                        placeholder="Filter lines"
                        prop:value=move || filter.get()
                        on:input=move |ev| filter.set(event_target_value(&ev))
                    />
                </label>

                <div class="logs__actions">
                    <button
                        type="button"
                        class="tool-btn"
                        class:is-active=move || paused.get()
                        on:click=toggle_pause
                        title=move || if paused.get() { "Resume" } else { "Pause" }
                    >
                        {move || {
                            if paused.get() {
                                view! { <Ico icon=icondata::LuPlay /> }.into_any()
                            } else {
                                view! { <Ico icon=icondata::LuPause /> }.into_any()
                            }
                        }}
                        <span>{move || if paused.get() { "Resume" } else { "Pause" }}</span>
                        <Show when=move || { held_count.get() > 0 }>
                            <span class="tool-btn__count">{move || format!("+{}", held_count.get())}</span>
                        </Show>
                    </button>
                    <button
                        type="button"
                        class="tool-btn"
                        class:is-active=move || wrap.get()
                        on:click=move |_| wrap.update(|w| *w = !*w)
                        title="Wrap long lines"
                    >
                        <Ico icon=icondata::LuWrapText />
                        <span class="sr-only">"Wrap"</span>
                    </button>
                    <button
                        type="button"
                        class="tool-btn"
                        on:click=move |_| lines.set(Vec::new())
                        title="Clear the view (the container's logs stay)"
                    >
                        <Ico icon=icondata::LuEraser />
                        <span class="sr-only">"Clear"</span>
                    </button>
                </div>
            </div>

            <div class="logs__screen" node_ref=scroller on:scroll=on_scroll>
                <Show
                    when=move || !lines.with(Vec::is_empty)
                    fallback=move || {
                        view! {
                            <p class="logs__empty">
                                {move || match feed.get() {
                                    Feed::Connecting | Feed::Reconnecting => "Waiting for logs…",
                                    Feed::Live => "No output yet. New lines appear here as they're written.",
                                    _ => "Nothing to show.",
                                }}
                            </p>
                        }
                    }
                >
                    <For each=visible key=|line| line.seq let:line>
                        <div class="log" class:is-err=line.err>
                            <span class="log__ts">{line.ts}</span>
                            <span class="log__msg">{line.text}</span>
                        </div>
                    </For>
                </Show>
            </div>

            <Show when=move || !follow.get()>
                <button type="button" class="logs__jump" on:click=jump_to_end>
                    <Ico icon=icondata::LuArrowDownToLine />
                    "Jump to latest"
                </button>
            </Show>

            <div class="logs__foot">
                <span>{move || format!("{} lines", lines.with(Vec::len))}</span>
                <span>"last 300 on connect · follows live"</span>
            </div>
        </div>
    }
}
