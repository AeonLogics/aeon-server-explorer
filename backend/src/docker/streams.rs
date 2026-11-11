use bollard::Docker;
use bollard::container::LogOutput;
use bollard::models::ContainerStatsResponse;
use bollard::query_parameters::{LogsOptionsBuilder, StatsOptionsBuilder};
use chrono::{DateTime, Local};
use futures::{Stream, StreamExt};
use shared::docker::{LogLine, StatsSample};
use shared::errors::docker::DockerError;
use tokio::sync::mpsc;

use super::map_err;

const MAX_LINE: usize = 4_000;

pub fn logs(
    docker: Docker,
    id: String,
    tail: u32,
) -> impl Stream<Item = Result<LogLine, DockerError>> {
    pump(move |tx| async move {
        let options = LogsOptionsBuilder::new()
            .follow(true)
            .stdout(true)
            .stderr(true)
            .timestamps(true)
            .tail(&tail.to_string())
            .build();
        let mut source = docker.logs(&id, Some(options));
        loop {
            let next = tokio::select! {
                next = source.next() => next,
                _ = tx.closed() => return,
            };
            let Some(item) = next else { return };
            let lines = match item {
                Ok(output) => split(output),
                Err(e) => {
                    let _ = tx.send(Err(map_err(e))).await;
                    return;
                }
            };
            for line in lines {
                if tx.send(Ok(line)).await.is_err() {
                    return;
                }
            }
        }
    })
}

pub fn stats(docker: Docker, id: String) -> impl Stream<Item = Result<StatsSample, DockerError>> {
    pump(move |tx| async move {
        let options = StatsOptionsBuilder::new().stream(true).build();
        let mut source = std::pin::pin!(docker.stats(&id, Some(options)));
        loop {
            let next = tokio::select! {
                next = source.next() => next,
                _ = tx.closed() => return,
            };
            let Some(item) = next else { return };
            let sample = item.map(|s| sample(&s)).map_err(map_err);
            let failed = sample.is_err();
            if tx.send(sample).await.is_err() || failed {
                return;
            }
        }
    })
}

// The Docker stream runs in its own task and stops as soon as the browser goes away.
fn pump<T, F, Fut>(run: F) -> impl Stream<Item = T>
where
    T: Send + 'static,
    F: FnOnce(mpsc::Sender<T>) -> Fut,
    Fut: Future<Output = ()> + Send + 'static,
{
    let (tx, rx) = mpsc::channel::<T>(512);
    tokio::spawn(run(tx));
    futures::stream::unfold(rx, |mut rx| async move {
        rx.recv().await.map(|item| (item, rx))
    })
}

fn split(output: LogOutput) -> Vec<LogLine> {
    let err = matches!(output, LogOutput::StdErr { .. });
    let bytes = output.into_bytes();
    let text = String::from_utf8_lossy(&bytes);

    text.split('\n')
        .map(|raw| raw.trim_end_matches('\r'))
        .filter(|raw| !raw.is_empty())
        .map(|raw| {
            let (ts, rest) = match raw.split_once(' ') {
                Some((ts, rest)) if looks_like_timestamp(ts) => (local_time(ts), rest),
                _ => (String::new(), raw),
            };
            let mut text = strip_ansi(rest);
            if text.len() > MAX_LINE {
                let mut cut = MAX_LINE;
                while !text.is_char_boundary(cut) {
                    cut -= 1;
                }
                text.truncate(cut);
                text.push('…');
            }
            LogLine { ts, err, text }
        })
        .collect()
}

fn looks_like_timestamp(s: &str) -> bool {
    s.len() >= 20 && s.as_bytes()[4] == b'-' && s.as_bytes()[10] == b'T'
}

fn local_time(ts: &str) -> String {
    DateTime::parse_from_rfc3339(ts)
        .map(|t| t.with_timezone(&Local).format("%H:%M:%S").to_string())
        .unwrap_or_default()
}

fn strip_ansi(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\u{1b}' {
            if c == '\t' || !c.is_control() {
                out.push(c);
            }
            continue;
        }
        match chars.next() {
            Some('[') => {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') => {
                while let Some(c) = chars.next() {
                    if c == '\u{7}' {
                        break;
                    }
                    if c == '\u{1b}' && chars.peek() == Some(&'\\') {
                        chars.next();
                        break;
                    }
                }
            }
            _ => {}
        }
    }
    out
}

fn sample(s: &ContainerStatsResponse) -> StatsSample {
    let cpu = s.cpu_stats.as_ref();
    let pre = s.precpu_stats.as_ref();
    let total = |c: Option<&bollard::models::ContainerCpuStats>| {
        c.and_then(|c| c.cpu_usage.as_ref())
            .and_then(|u| u.total_usage)
            .unwrap_or(0)
    };
    let system = |c: Option<&bollard::models::ContainerCpuStats>| {
        c.and_then(|c| c.system_cpu_usage).unwrap_or(0)
    };

    let cpus = cpu
        .and_then(|c| c.online_cpus)
        .filter(|n| *n > 0)
        .or_else(|| {
            cpu.and_then(|c| c.cpu_usage.as_ref())
                .and_then(|u| u.percpu_usage.as_ref())
                .map(|p| p.len() as u32)
                .filter(|n| *n > 0)
        })
        .unwrap_or(1);

    let cpu_delta = total(cpu).saturating_sub(total(pre)) as f64;
    let system_delta = system(cpu).saturating_sub(system(pre)) as f64;
    let cpu_percent = if system_delta > 0.0 && cpu_delta > 0.0 {
        cpu_delta / system_delta * f64::from(cpus) * 100.0
    } else {
        0.0
    };

    // Same as `docker stats`: page cache doesn't count as used memory.
    let memory = s.memory_stats.as_ref();
    let usage = memory.and_then(|m| m.usage).unwrap_or(0);
    let cache = memory
        .and_then(|m| m.stats.as_ref())
        .and_then(|st| {
            st.get("inactive_file")
                .or_else(|| st.get("total_inactive_file"))
                .or_else(|| st.get("cache"))
                .copied()
        })
        .unwrap_or(0);

    StatsSample {
        cpu: cpu_percent,
        cpus,
        mem: usage.saturating_sub(cache),
        mem_limit: memory.and_then(|m| m.limit).unwrap_or(0),
    }
}
