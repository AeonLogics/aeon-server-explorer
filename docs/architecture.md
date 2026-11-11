# Architecture

One Rust binary serves the pages, the API and the live streams. It talks to the Docker engine
through its local socket and keeps its own small state in an embedded SurrealDB file.

```
browser ──SSH tunnel──▶ 127.0.0.1:5050 ─┐
                                         ▼
                ┌────────── aeon-server-explorer ──────────┐
                │ guards: Host check → session check        │
                │ Leptos pages (SSR + hydrate)              │
                │ server functions   (/api/...)             │
                │ live streams (SSE) (/api/logs|stats|pull) │
                │ backend: db · docker · stacks · images    │
                └──────┬───────────────┬───────────────┬────┘
                       ▼               ▼               ▼
                explorer.db     Docker engine     stack folders
             (users, audit)  (/var/run/docker.sock) (STACKS_DIR)
```

## Crates

| Crate | What it holds |
|---|---|
| root (`src/`) | the web app: Axum server, Leptos pages and components, server functions, guards |
| `backend/` | everything that touches the outside world: SurrealDB, Docker (bollard), stacks on disk, Docker Hub |
| `shared/` | types and rules used by both server and browser: `ServiceCard`, `StackSpec`, errors, validation |

The root crate is compiled twice: once for the server (feature `ssr`) and once to WebAssembly
for the browser (feature `hydrate`). `#[cfg(feature = "ssr")]` code only exists on the server.

## A request, end to end

1. **Host guard** ([middleware.rs](../src/middleware.rs)) answers 403 unless the `Host` header
   is `localhost:<port>` or `127.0.0.1:<port>`. This stops DNS-rebinding tricks.
2. **Session layer** reads the `__Host-explorer_session` cookie, looks up the SHA-256 of the
   token in SurrealDB, and puts the admin (or nothing) on the request. Background paths
   (`/api/poll/…`, logs, stats) don't extend the 30-minute idle timeout.
3. **Pages** render on the server first, then the WebAssembly bundle hydrates them, so the first
   paint is complete and the page is interactive right after.
4. **Server functions** (`#[server]` in [src/api](../src/api)) call `require_admin()`, then the
   backend, and write an audit entry for anything that changes something.

## Live data

Logs, CPU/RAM and image pulls use **Server-Sent Events**: one long HTTP response the server keeps
writing to.

```
Docker stream → background task → channel → SSE events → EventSource in the browser → view
```

- [backend/src/docker/streams.rs](../backend/src/docker/streams.rs) runs each Docker stream in
  its own Tokio task and stops it as soon as the browser goes away (`select!` on the channel
  closing). Image pulls are the exception: they finish even if the tab closes.
- [src/sse.rs](../src/sse.rs) turns items into SSE events and ends with an `end` event, so the
  browser's `EventSource` doesn't reconnect forever after a container stops.
- [src/components/sse.rs](../src/components/sse.rs) wraps `EventSource` for Leptos;
  [logs.rs](../src/components/logs.rs) batches incoming lines every 60 ms so a burst of 300
  lines doesn't freeze the page.

## Lists and refreshes

Pages read server functions through `Resource`s keyed on a shared 5-second tick, inside
`<Transition>` so old content stays visible while new data loads. Forms that you type into (the
stack workbench) live outside the refreshing part and read the latest data only when you save,
so a refresh can never wipe what you're typing.

## Data

- **SurrealDB, embedded** (`surrealkv://data/explorer.db`, or `SURREAL_URI`): the admin, sessions
  and the audit log. The schema in [surql_flux/schema](../surql_flux/schema) is applied on every
  start. Login, lockout and password hashing (argon2) happen in SurrealQL.
- **Docker** is the truth about what runs.
- **Stack folders** are the truth about managed stacks: `explorer.json`, the generated
  `compose.yml` and private env files. See [managed-stacks.md](managed-stacks.md).

## Startup

[src/main.rs](../src/main.rs) runs the server on its own thread with a 16 MB stack (embedded
SurrealDB needs more than Windows' 1 MB main thread), refuses to start on anything but a
loopback address, applies the schema, creates the admin on first start, and connects to Docker.
If Docker isn't running yet, the app still starts and shows Docker as offline until it is.
