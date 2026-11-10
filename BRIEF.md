# Aeon Server Explorer: hand-off brief

Paste this into a new chat (or point it at this file). Read `DESIGN.md` next to it: that is the
agreed design.

## What it is
A private web UI that wraps Docker and Docker Compose on my own server. It is **not** a hosting
platform (no Git builds, no webhooks, no multi-server). The `compose.yml` files on disk are the
truth; the app reads Docker's live state, shows it Railway-style, and edits those files and runs
`docker compose`. On top of that: storage helpers, safety rules for compose files, and making
apps live through Cloudflare. Portainer and Dockge were confusing or too limited; Coolify is
too big and holds a root SSH key.

## Me
Aeon Logics. Rust/Leptos developer, I type fast and informally. Write code and files directly,
but give me the slow commands (`cargo add`, builds, docker) to run myself. Never deploy to the
server unless I say so. Keep explanations short and concrete.

## The server
- `aeon@192.168.100.26`, SSH key `~/.ssh/id_ed25519`, Ubuntu x86_64, Docker 29,
  **1 CPU core**, 7.7 GB RAM, **no passwordless sudo**. Don't build Rust on it; build images on
  the PC, then `docker save | ssh … docker load`.
- Today every service has its own folder `~/services/<name>/` with a `compose.yml`:
  - `aeonlogics`: app + SurrealDB + Caddy on `127.0.0.1:3010`
  - `cake-delight`: app + Caddy on `127.0.0.1:3020` (db being added)
  - `portainer`: LAN-only `:9443`
  - `dockge`: `127.0.0.1:5001`, reached with an SSH tunnel
- **Target layout** (one-time `sudo` to create, owned by `aeon`):
  - `/srv/aeon/stacks/<project>/compose.yml`: the stacks (a git repo, every save is a commit)
  - `/srv/aeon/data/<project>/<service>/<name>/`: bind-mounted storage, one folder to back up
  - Existing `~/services` stacks are still found through Docker labels and can be adopted
    (moved) in phase 3. Nothing moves automatically.
- Public sites go through a **Cloudflare Tunnel** (cloudflared systemd service, config in
  `~/.cloudflared/config.yml` mapping hostname → localhost port).

## Project
- Folder: `R:\aeon-deck` (git repo; the folder keeps its old name for now).
- Stack: Rust, **Leptos 0.8 + Axum** (SSR + hydrate, `cargo-leptos`), **`bollard`** for the
  Docker API (status, logs, stats), the **`docker compose` CLI** for applying stacks (called
  directly, never through a shell), **SurrealDB v3** (SDK `surrealdb = "3.2"`,
  `#[derive(SurrealValue)]` on result types), embedded `surrealkv://data/explorer.db` by default.
- **Reference implementation: `R:\aeonlogics`** (my own project). Copy its structure and
  patterns: queries as Rust consts in `AuthQuery`-style structs, an `AuthStore` with
  `Resource::new_blocking`, a `__Host-` session cookie storing only the SHA-256 of the
  token, argon2 done in SurrealQL, schema files applied on start. Don't use `R:\cake-delight`:
  it's a client project.

## Lessons learned (avoid)
- No `inner_html` on `<svg>` (broke hydration). Use `leptos_icons` + `icondata` (Lucide).
- SurrealDB v3: `ORDER BY` fields must be in the SELECT list (sort in a subquery);
  `array<T, N>` means exactly N items (use ASSERT for limits); regexes in SurrealQL strings
  need `\\.`.

## Security (non-negotiable)
Controlling Docker = root on the server, and so does editing a compose file.
- Bind to loopback only, reach it via `ssh -L 5050:localhost:5050 aeon@192.168.100.26`.
  Never expose publicly, never in the Cloudflare tunnel.
- Host-header check, strong single-admin login with lockout, `SameSite=Strict` session cookie
  (`__Host-explorer_session`), audit log of every action, type-the-name confirmation for
  anything destructive.
- Compose files saved by the app must pass the safety rules in `DESIGN.md` §3 (no host paths
  outside `/srv/aeon/data`, no `docker.sock`, no `privileged`, no host network/pid, no extra
  capabilities).
- Secrets: encrypted vault keyed from a master password (Argon2id → AES-256-GCM), never in the
  browser, masked in the UI, auto-lock. Details in `DESIGN.md` §5.

## Roadmap
1. **Phase 1, read-only (do now):** login for a single admin; sidebar with compose projects;
   grid of service cards per project (image, status/health, ports, networks); service detail
   panel with **live streamed logs** and CPU/RAM stats (SSE).
2. Phase 2, operate: start/stop/restart/pull/redeploy, edit env vars, the encrypted vault,
   audit log page.
3. Phase 3, stacks on disk: adopt existing stacks, add services from images, edit compose
   (form or raw YAML) with validation + safety rules, diff before save, git history and
   rollback, one deploy per project at a time.
4. Phase 4, storage and go live: storage helper (named volumes vs. bind mounts under
   `/srv/aeon/data`, sizes); "Go live": list my Cloudflare zones, add a tunnel ingress + DNS
   record **via the Cloudflare API** (needs a one-time migration of the tunnel to
   dashboard-managed).

## Done for the next chat
Phase 1 running locally against **Docker Desktop on my Windows PC** with real containers and
live logs, plus a `README.md` with exactly how to run it. (Docker Desktop must be started
first; Docker's Windows pipe is `//./pipe/docker_engine`.) Nothing deployed to the server.
