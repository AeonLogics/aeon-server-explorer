# Aeon Deck: hand-off brief

Paste this into a new chat (or point it at this file). Read `DESIGN.md` next to it: that is the
agreed design.

## What it is
A private, Railway-style web dashboard for the Docker services on my own server. I create
projects and services from images, choose port exposure (internal / server-localhost /
public), manage networks and volumes, restart services, read logs, and make apps live through
Cloudflare automatically. Portainer and Dockge were confusing or too limited.

## Me
Aeon Logics. Rust/Leptos developer, I type fast and informally. Write code and files directly,
but give me the slow commands (`cargo add`, builds, docker) to run myself. Never deploy to the
server unless I say so. Keep explanations short and concrete.

## The server
- `aeon@192.168.100.26`, SSH key `~/.ssh/id_ed25519`, Ubuntu x86_64, Docker 29,
  **1 CPU core**, 7.7 GB RAM, **no passwordless sudo**. Don't build Rust on it; build images on
  the PC, then `docker save | ssh … docker load`.
- Every service has its own folder `~/services/<name>/` with a `compose.yml`:
  - `aeonlogics`: app + SurrealDB + Caddy on `127.0.0.1:3010`
  - `cake-delight`: app + Caddy on `127.0.0.1:3020` (db being added)
  - `portainer`: LAN-only `:9443`
  - `dockge`: `127.0.0.1:5001`, reached with an SSH tunnel
- Public sites go through a **Cloudflare Tunnel** (cloudflared systemd service, config in
  `~/.cloudflared/config.yml` mapping hostname → localhost port).

## Project
- Folder: `R:\aeon-deck` (git repo, `DESIGN.md` committed).
- Stack: Rust, **Leptos 0.8 + Axum** (SSR + hydrate, `cargo-leptos`), **`bollard`** for the
  Docker API, **SurrealDB v3** (SDK `surrealdb = "3.2"`, `#[derive(SurrealValue)]` on result
  types), embedded `surrealkv://` by default.
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
Controlling Docker = root on the server.
- Bind to loopback only, reach it via `ssh -L 5050:localhost:5050 aeon@192.168.100.26`.
  Never expose publicly, never in the Cloudflare tunnel.
- Host-header check, strong single-admin login with lockout, `SameSite=Strict` session cookie,
  audit log of every action, type-the-name confirmation for anything destructive.
- Secrets: encrypted vault keyed from a master password (Argon2id → AES-256-GCM), never in the
  browser, masked in the UI, auto-lock. Details in `DESIGN.md` §4.

## Roadmap
1. **Phase 1, read-only (do now):** login for a single admin; sidebar with compose projects;
   grid of service cards per project (image, status/health, ports, networks); service detail
   panel with **live streamed logs** and CPU/RAM stats (SSE).
2. Phase 2: restart/stop/redeploy, edit env vars, the encrypted vault.
3. Phase 3: builder: add services from Docker Hub images; ports as internal / server-localhost /
   public; create and attach networks and volumes. Writes **normal `compose.yml` files** into
   `~/services/<name>/` (files stay the source of truth, so Dockge, Portainer and deploy
   scripts keep working) and runs `docker compose`.
4. Phase 4: "Go live": pick a subdomain, register a Cloudflare Tunnel ingress + DNS record
   **via the Cloudflare API** (no hand-editing cloudflared config, no sudo). Needs a one-time
   migration of the tunnel to dashboard-managed.

## Done for the next chat
Phase 1 running locally against **Docker Desktop on my Windows PC** with real containers and
live logs, plus a `README.md` with exactly how to run it. (Docker Desktop must be started
first; Docker's Windows pipe is `//./pipe/docker_engine`.) Nothing deployed to the server.
