# Aeon Server Explorer

A small, self-hosted dashboard for the Docker containers running on my server.

It groups containers by Compose project and shows each service at a glance: image, status and
health, published ports (with a warning when something is exposed beyond localhost) and the
networks it's attached to. Open a service to see its details, mounts and environment variable
names (never the values), plus live logs and CPU/RAM usage streamed straight from Docker.

It's built for one person: a single admin account, strict session cookies, rate-limited login
and an audit log. It only listens on loopback, so you reach it through an SSH tunnel and never
expose it to the internet.

**Stack:** Rust · Leptos 0.8 · Axum · bollard · SurrealDB (embedded)

> Status: phase 1, read-only. See `DESIGN.md` for what comes next.

## Run it locally (Windows + Docker Desktop)

You need: Rust (stable), `cargo-leptos`, the `wasm32-unknown-unknown` target, and Docker Desktop.

1. **Start Docker Desktop** and wait until it says *Engine running*.

2. **Create `.env`** in the project folder (it is git-ignored):

   ```powershell
   Copy-Item .env.example .env
   ```

   Then edit `.env` and set `ADMIN_EMAIL` and `ADMIN_PASSWORD`. They are only used on the very
   first start, to create the admin account.

3. **Optional: start the demo stack**, so there is something to look at (4 services, live logs,
   a health check and one deliberately exposed port):

   ```powershell
   docker compose -f examples/demo/compose.yml up -d
   ```

4. **Run the app:**

   ```powershell
   cargo leptos watch
   ```

   The first build takes a while (SurrealDB is big). When you see
   `listening on http://localhost:5050`, open **http://localhost:5050**.

5. **Sign in** with `ADMIN_EMAIL` / `ADMIN_PASSWORD`. You'll be asked to choose a new password
   (at least 12 characters) before anything else.

Stop the demo stack again with:

```powershell
docker compose -f examples/demo/compose.yml down -v
```

### Release build

```powershell
cargo leptos serve --release
```

## Configuration

| Variable | Default | What it does |
|---|---|---|
| `ADMIN_EMAIL` | | Admin login, only read when no admin exists yet |
| `ADMIN_PASSWORD` | | First password, must be changed on first login |
| `SURREAL_URI` | `surrealkv://data/explorer.db` | Database. A `ws://` URI also needs `SURREAL_USER` / `SURREAL_PASS` |

Docker is reached through its standard local socket: the named pipe `//./pipe/docker_engine` on
Windows, `/var/run/docker.sock` on Linux.

The address is `127.0.0.1:5050` (`site-addr` in `Cargo.toml`, or `LEPTOS_SITE_ADDR`). The app
refuses to start on anything that isn't a loopback address.

**Forgot the password / start over:** stop the app, delete the `data/` folder, and start again;
the admin is re-created from `.env`.

## Security notes

- Controlling Docker is root on the host. The app binds to loopback only, and rejects any request
  whose `Host` header isn't `localhost:5050` or `127.0.0.1:5050` (stops DNS rebinding).
- On a server, reach it with `ssh -L 5050:localhost:5050 <user>@<server>`. Never put it behind
  a public proxy or tunnel.
- Session cookie `__Host-explorer_session` is `HttpOnly; Secure; SameSite=Strict`. The database
  only stores the SHA-256 of the token. Sessions last at most 12 h and end after 30 min idle
  (background refreshes and live streams don't count as activity).
- 5 wrong passwords lock the login for 15 minutes. Logins (ok and failed), lockouts, logouts and
  password changes go to the `audit` table.
- Environment variable values are never sent to the browser, only their names.

## Layout

```
backend/            database connection, Docker queries and live streams (bollard)
shared/             types and errors shared by server and browser
src/api/            server functions + SurrealQL queries
src/sse.rs          /api/logs/{container} and /api/stats/{container} (Server-Sent Events)
src/middleware.rs   Host-header guard, session lookup
src/components/     log viewer, stats, sparkline, chips, badges
src/pages/          login, password change, overview, project, service panel
style/              SCSS (Night Amethyst theme)
surql_flux/schema/  schema, applied on every start
examples/demo/      a small compose stack for trying it out
```
