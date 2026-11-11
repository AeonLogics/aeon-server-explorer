#![recursion_limit = "256"]

// Embedded SurrealDB needs far more stack than Windows gives the main thread (1 MB).
#[cfg(feature = "ssr")]
const STACK_SIZE: usize = 16 * 1024 * 1024;

#[cfg(feature = "ssr")]
fn main() {
    let server = std::thread::Builder::new()
        .name("server".into())
        .stack_size(STACK_SIZE)
        .spawn(|| {
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .thread_stack_size(STACK_SIZE)
                .build()
                .expect("tokio runtime")
                .block_on(serve())
        })
        .expect("server thread");
    if server.join().is_err() {
        std::process::exit(1);
    }
}

#[cfg(feature = "ssr")]
async fn serve() {
    use std::sync::Arc;

    use axum::routing::get;
    use axum::Router;
    use explorer_lib::app::*;
    use explorer_lib::state::AppState;
    use explorer_lib::{middleware, sse, startup};
    use leptos::logging::log;
    use leptos::prelude::*;
    use leptos_axum::{generate_route_list, LeptosRoutes};

    fn fail(message: impl std::fmt::Display) -> ! {
        eprintln!("error: {message}");
        std::process::exit(1);
    }

    dotenvy::dotenv().ok();

    let conf = get_configuration(None).unwrap_or_else(|e| fail(e));
    let addr = conf.leptos_options.site_addr;
    let leptos_options = conf.leptos_options;

    if !addr.ip().is_loopback() {
        fail(format!(
            "refusing to start: {addr} is not a loopback address (use 127.0.0.1 and an SSH tunnel)"
        ));
    }

    let port = addr.port();
    let allowed_hosts: Arc<[String]> =
        Arc::from([format!("localhost:{port}"), format!("127.0.0.1:{port}")]);

    let uri = std::env::var("SURREAL_URI")
        .ok()
        .filter(|v| !v.trim().is_empty())
        .unwrap_or_else(|| backend::db::DEFAULT_URI.to_string());
    if uri.starts_with("surrealkv://") {
        let _ = std::fs::create_dir_all("data");
    }
    let db = backend::db::connect(&uri).await.unwrap_or_else(|e| fail(e));
    startup::apply_schema(&db).await.unwrap_or_else(|e| fail(e));
    startup::ensure_admin(&db).await.unwrap_or_else(|e| fail(e));
    log!("database ready ({uri})");

    let docker = backend::docker::connect().unwrap_or_else(|e| fail(format!("docker client: {e}")));
    match docker.ping().await {
        Ok(_) => log!("docker is reachable"),
        Err(e) => log!("docker is not reachable yet ({e}); the UI will show it as offline"),
    }

    let stacks = backend::stacks::Stacks::from_env();
    log!("stacks folder: {}", stacks.root().display());

    let app_state = AppState {
        leptos_options,
        db,
        docker,
        stacks,
        allowed_hosts,
    };
    let routes = generate_route_list(App);

    let app = Router::new()
        .route("/api/logs/{container}", get(sse::logs))
        .route("/api/stats/{container}", get(sse::stats))
        .route("/api/pull", get(sse::pull))
        .leptos_routes(&app_state, routes, {
            let leptos_options = app_state.leptos_options.clone();
            move || shell(leptos_options.clone())
        })
        .fallback(leptos_axum::file_and_error_handler::<AppState, _>(shell))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::auth_layer,
        ))
        .layer(axum::middleware::from_fn_with_state(
            app_state.clone(),
            middleware::host_guard,
        ))
        .with_state(app_state);

    log!("listening on http://localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .unwrap_or_else(|e| fail(format!("could not bind {addr}: {e}")));
    axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .await
    .unwrap_or_else(|e| fail(e));
}

#[cfg(not(feature = "ssr"))]
pub fn main() {}
