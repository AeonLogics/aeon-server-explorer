use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("Something went wrong. Please try again.")]
pub struct InternalServerError {
    #[serde(skip)]
    cause: Option<String>,
}

impl InternalServerError {
    #[track_caller]
    pub fn new(cause: impl std::fmt::Display) -> Self {
        let cause = cause.to_string();
        #[cfg(feature = "ssr")]
        log(&cause, std::panic::Location::caller());
        Self { cause: Some(cause) }
    }

    pub fn cause(&self) -> Option<&str> {
        self.cause.as_deref()
    }
}

#[cfg(feature = "ssr")]
fn log(cause: &str, at: &std::panic::Location<'_>) {
    use colored::Colorize;

    // cargo-leptos pipes the server's output, so `colored` would switch itself off.
    static INIT: std::sync::Once = std::sync::Once::new();
    INIT.call_once(|| {
        if std::env::var_os("NO_COLOR").is_none() {
            colored::control::set_override(true);
        }
        #[cfg(windows)]
        let _ = colored::control::set_virtual_terminal(true);
    });

    eprintln!(
        "{} {} {}",
        "[INTERNAL ERROR]".red().bold(),
        cause.red(),
        format!("at {}:{}", at.file(), at.line()).dimmed()
    );
}
