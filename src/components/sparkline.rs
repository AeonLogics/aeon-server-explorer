use leptos::prelude::*;

const W: f64 = 240.0;
const H: f64 = 56.0;
const PAD: f64 = 4.0;

// One series, so no legend: the tile's label names it. Each sample gets a hover target
// wider than the line, and the tooltip shows its value.
#[component]
pub fn Sparkline(
    #[prop(into)] values: Signal<Vec<f64>>,
    #[prop(into)] max: Signal<f64>,
    capacity: usize,
    format: fn(f64) -> String,
    label: &'static str,
) -> impl IntoView {
    let hover = RwSignal::new(None::<usize>);
    let step = W / (capacity.max(2) - 1) as f64;

    let points = move || {
        let values = values.get();
        let max = max.get().max(f64::EPSILON);
        let offset = capacity.saturating_sub(values.len()) as f64 * step;
        values
            .iter()
            .enumerate()
            .map(|(i, v)| {
                let x = offset + i as f64 * step;
                let y = PAD + (H - 2.0 * PAD) * (1.0 - (v / max).clamp(0.0, 1.0));
                (x, y, *v)
            })
            .collect::<Vec<_>>()
    };

    let line = move || {
        points()
            .iter()
            .map(|(x, y, _)| format!("{x:.1},{y:.1}"))
            .collect::<Vec<_>>()
            .join(" ")
    };

    let area = move || {
        let pts = points();
        match (pts.first(), pts.last()) {
            (Some(first), Some(last)) => {
                let mut d = format!("M{:.1},{H} ", first.0);
                for (x, y, _) in &pts {
                    d.push_str(&format!("L{x:.1},{y:.1} "));
                }
                d.push_str(&format!("L{:.1},{H} Z", last.0));
                d
            }
            _ => String::new(),
        }
    };

    let head = move || {
        let pts = points();
        let i = hover.get().filter(|i| *i < pts.len()).unwrap_or(pts.len().saturating_sub(1));
        pts.get(i).copied()
    };

    view! {
        <div class="spark" on:mouseleave=move |_| hover.set(None)>
            <svg viewBox=format!("0 0 {W} {H}") preserveAspectRatio="none" role="img" aria-label=label>
                <line class="spark__base" x1="0" y1=H.to_string() x2=W.to_string() y2=H.to_string() />
                <path class="spark__area" d=area />
                <polyline class="spark__line" points=line />
                {move || {
                    hover
                        .get()
                        .and(head())
                        .map(|(x, _, _)| view! { <line class="spark__cross" x1=format!("{x:.1}") y1="0" x2=format!("{x:.1}") y2=H.to_string() /> })
                }}
                {move || {
                    points()
                        .into_iter()
                        .enumerate()
                        .map(|(i, (x, _, _))| {
                            view! {
                                <rect
                                    class="spark__hit"
                                    x=format!("{:.1}", x - step / 2.0)
                                    y="0"
                                    width=format!("{step:.2}")
                                    height=H.to_string()
                                    on:mouseenter=move |_| hover.set(Some(i))
                                />
                            }
                        })
                        .collect_view()
                }}
            </svg>
            {move || {
                head()
                    .map(|(x, y, _)| {
                        view! {
                            <span
                                class="spark__head"
                                style=format!("left: {:.2}%; top: {:.2}%", x / W * 100.0, y / H * 100.0)
                            ></span>
                        }
                    })
            }}
            {move || {
                hover
                    .get()
                    .and(head())
                    .map(|(x, _, v)| {
                        let left = (x / W * 100.0).clamp(12.0, 88.0);
                        view! {
                            <span class="spark__tip" style=format!("left: {left:.2}%")>
                                {format(v)}
                            </span>
                        }
                    })
            }}
        </div>
    }
}
