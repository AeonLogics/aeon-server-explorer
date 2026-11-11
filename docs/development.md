# Development

## Run it locally

1. Install Rust (stable), the WebAssembly target and cargo-leptos:
   ```powershell
   rustup target add wasm32-unknown-unknown
   cargo install cargo-leptos --locked
   ```
2. Start a Docker engine (Docker Desktop on Windows/macOS).
3. Create `.env` from the example and set the first admin:
   ```powershell
   Copy-Item .env.example .env
   ```
4. Run with live reload:
   ```powershell
   cargo leptos watch
   ```
5. Open **http://localhost:5055** (local dev uses 5055, so it never clashes with a tunnel to a
   server on 5050).

Data lives in `data/` (git-ignored): the database in `data/explorer.db`, managed stacks in
`data/stacks`. Delete `data/explorer.db` to start over; the admin is created again from `.env`.

For a release build: `cargo leptos serve --release`. On Windows, `.cargo/config.toml` uses
`rust-lld` as the linker.

## Layout

```
src/
  main.rs            startup: big-stack thread, loopback check, schema, admin, routes
  middleware.rs      Host check and session lookup
  sse.rs             live streams: logs, stats, image pulls
  startup.rs         schema and first admin
  api/               server functions (auth, docker, stacks, images, audit) and SurrealQL queries
  components/        log viewer, stats, sparkline, action bar, chips, SSE client
  pages/             overview, project, service, stacks, inspector, images, audit, login
backend/src/
  db.rs              SurrealDB connection
  docker/            containers, details, actions, live streams
  stacks/            managed stacks: files, compose generator, apply
  images.rs          local images, Docker Hub search and tags, pulls
shared/src/          types and validation used on both sides
style/               SCSS: a light, matte palette with hairlines and small radii
surql_flux/schema/   SurrealDB schema, applied on every start
deploy/              server compose file, env example, SSH tunnel scripts
```

## Gotchas worth knowing

- **Braces around comparisons in `view!`.** `disabled=move || n > 0` breaks: the `>` ends the
  tag, the rest becomes text, and any `{ … }` after it runs during rendering. Write
  `disabled={move || n > 0}`.
- **No `window()` in components.** Components also render on the server, where browser APIs
  panic. Use in-app confirmations instead of `confirm()`.
- **No `StoredValue::new_local`** for data that is also created on the server: it panics when a
  request finishes on another worker thread. Use `StoredValue::new`.
- **Forms stay outside the refresh.** Anything you type into must not live inside a part of the
  page that re-renders on the 5-second tick, or it resets.
- **Server functions with lists use JSON** (`#[server(input = Json)]`): the default form encoding
  drops empty lists.
- **Release builds need `#![recursion_limit = "256"]`** because of deeply nested view types.
- **SurrealDB v3:** `ORDER BY` fields must be in the `SELECT` list; regexes in strings need `\\.`.
- **Icons** come from `leptos_icons` + `icondata` (Lucide). Don't use `inner_html` on `<svg>`.
