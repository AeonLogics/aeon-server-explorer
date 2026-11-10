# Aeon Deck: design

A private, Railway-style dashboard for the Docker services on one server
(`aeon@192.168.100.26`). One admin, one server, opened in the browser through an SSH tunnel.

## 1. Concepts

| Deck word | What it really is | Where it lives |
|---|---|---|
| **Project** | one compose project | the folder `~/services/<name>/` with its `compose.yml` |
| **Service** | one compose service (one or more containers) | a `services:` entry in that file |
| **Network** | a Docker network | `networks:` in the file; drawn as a line between the cards that share it |
| **Volume** | a named Docker volume or a bind mount | `volumes:` in the file |
| **Exposure** | how a port is reachable (see below) | `ports:` in the file, plus a Cloudflare ingress rule for *public* |

**The `compose.yml` files are the source of truth.** Deck never keeps its own copy of what a
project contains. It reads the running state from Docker and the desired state from the
files, and from phase 3 on it writes normal compose files and runs `docker compose`. Dockge,
Portainer and the existing `deploy.sh` scripts keep working on the same files.

How Deck finds projects:

1. Running/stopped containers, grouped by the label `com.docker.compose.project`. The label
   `com.docker.compose.project.working_dir` tells which folder it came from.
2. (Phase 3) folders under `SERVICES_DIR` (default `~/services`) that have a `compose.yml`
   but no containers yet, shown as "not deployed".

Containers that are not from compose show up under a virtual project **"(standalone)"**.

### Exposure levels (phase 3)

| Level | compose output | Reachable from |
|---|---|---|
| **Internal** | no `ports:`, only `networks:` | other services on the same network |
| **Server** | `ports: ["127.0.0.1:<host>:<container>"]` | the server itself (and SSH tunnels) |
| **Public** | same as *Server* + a Cloudflare Tunnel ingress + DNS record (phase 4) | the internet, via Cloudflare |

Deck never writes `0.0.0.0` / bare `"<host>:<container>"` port bindings. Existing files that
have them are flagged with a warning on the card. Host ports are suggested from the house
convention (one `30x0` block per project: aeonlogics 3010, cake-delight 3020, …) and checked
against ports already in use.

## 2. Architecture

```
 your PC browser ──ssh -L 5050:localhost:5050──▶ server 127.0.0.1:5050
                                                   │
                                     ┌─────────────┴─────────────┐
                                     │ aeon-deck (one binary)     │
                                     │  Leptos 0.8 SSR + hydrate  │
                                     │  Axum: server fns + SSE    │
                                     │  bollard ─▶ /var/run/docker.sock
                                     │  SurrealDB v3 (embedded)   │
                                     │  docker compose CLI (ph.3) │
                                     │  Cloudflare API (ph.4)     │
                                     └────────────────────────────┘
```

- **Rust, Leptos 0.8 + Axum**, built with `cargo-leptos` (SSR + hydrate), same layout as
  `cake-delight`.
- **Docker**: `bollard` over the local socket (named pipe `//./pipe/docker_engine` on
  Windows/Docker Desktop, `/var/run/docker.sock` on the server).
- **Live data**: logs and CPU/RAM stats are streamed with **Server-Sent Events** from plain
  Axum routes (`/api/logs/{id}`, `/api/stats/{id}`), checked against the session cookie. The
  browser uses `EventSource`; the stream stops when the panel closes. Lists (projects,
  cards) are normal Leptos server functions, refreshed every few seconds.
- **Storage**: SurrealDB v3, **embedded** (`surrealkv://data/deck.db`) by default, so Deck is a
  single process with no extra container. `SURREAL_URI` can point at a server
  (`ws://…`) instead. It holds: the admin user, sessions, the audit log and (phase 2) the
  vault. It never holds a project's config; that stays in the files.
- **Code layout**

  ```
  src/
    app.rs            shell, router
    model.rs          shared types (ProjectSummary, ServiceCard, ServiceDetail, …)
    auth/             login, sessions, AuthStore (copied from cake-delight, single admin)
    docker/           ssr only: bollard client, project/service queries, SSE routes
    guard.rs          ssr only: loopback-only bind check, Host-header check
    audit.rs          ssr only: audit log writes
    pages/            layout (sidebar), project canvas, service panel, login
    components/       icons (leptos_icons + icondata Lucide), status badges, log viewer
  surreal_flux/schema/*.surql   applied on every start (OVERWRITE)
  ```

- **On the server (later, only when you say so)**: a container with `network_mode: host`,
  the Docker socket and `~/services` mounted at the same path, `docker` CLI + compose plugin
  inside. The image is built on the PC (`docker build`), then `docker save | ssh … docker load`
  (the server has 1 CPU, so no Rust builds there). Data in `~/services/aeon-deck/data`.

## 3. Security model

**Controlling Docker = root on the server.** Anyone who can use Deck can mount `/` into a
container. Everything below follows from that.

1. **Localhost only.** Deck refuses to start unless it binds a loopback address
   (`127.0.0.1` / `::1`). On the server it runs with `network_mode: host` so the bind really is
   the host's loopback. You reach it with:
   `ssh -L 5050:localhost:5050 aeon@192.168.100.26` → `http://localhost:5050`.
   It is **never** added to the Cloudflare Tunnel or published on the LAN.
2. **Host-header check** (stops DNS-rebinding): requests whose `Host` is not
   `localhost:<port>` / `127.0.0.1:<port>` are rejected with 403.
3. **Login**: one admin, no sign-up. The account is created on first start from
   `ADMIN_EMAIL` / `ADMIN_PASSWORD` and the password must be changed on first login
   (min 12 chars). Argon2id in SurrealQL. 5 wrong tries → locked 15 min.
4. **Sessions**: random 256-bit token in a `__Host-deck_session` cookie
   (`HttpOnly; Secure; SameSite=Strict`); the DB stores only its SHA-256. 12 h maximum,
   ends after 30 min idle. Every server function and SSE route checks it.
5. **Audit log**: every login (ok and failed), logout, and (phase 2+) every action that changes
   something: who, what, which project/service, when, result. Read-only page in the UI;
   never edited or deleted from the UI.
6. **Destructive actions** (phase 2+: stop, remove, delete volume, delete project, take
   offline): a dialog where you **type the name** of the thing to confirm.
7. **No secrets in the browser**: no localStorage/sessionStorage, no secrets in server-fn
   responses except an explicit 30-second "reveal", env values shown masked.

## 4. Secrets vault (phase 2)

- **Master password** (separate from the login password), set once in the UI.
- Key setup: `Argon2id(master, salt, m=64 MiB, t=3, p=1)` → 256-bit **key-encryption key**.
  A random 256-bit **data key** is encrypted with it using AES-256-GCM and stored. Changing
  the master password re-wraps only the data key.
- Each secret: AES-256-GCM with the data key, a fresh 96-bit nonce, and
  `project/service/NAME` as associated data (a ciphertext can't be moved to another service).
  Stored in SurrealDB as `{ project, service, name, nonce, ciphertext }`.
- **Unlock** puts the data key in server memory only (`zeroize` on drop). **Auto-lock** after
  15 min without vault use, on logout, and on restart. Locked → deploying a service that needs
  secrets asks for the master password first.
- **How secrets reach containers**: compose files reference `${NAME}`; Deck runs
  `docker compose up` with those values in the process environment, so no plaintext `.env` is
  written. (Per project you can switch to "also write `.env` (0600)" so `deploy.sh`/Dockge can
  redeploy without Deck; off by default.) Note: Docker itself shows container env in
  `docker inspect`; that's outside Deck's control.
- The Cloudflare API token (phase 4) is a vault secret too.

## 5. Roadmap

### Phase 1: read-only dashboard (this session)
- Login for the single admin, forced password change, logout, audit of logins.
- Sidebar: compose projects (with running/total count and a status dot).
- Project view: a grid of service cards: name, image, status/health, published ports
  (with a warning for non-localhost binds), networks as coloured chips.
- Service panel: container details, env **names** (values hidden), mounts, **live logs**
  (SSE, follow, last 300 lines, auto-scroll, pause) and **live CPU/RAM**.
- Runs locally against Docker Desktop.

### Phase 2: operate
- Restart / stop / start / redeploy (`docker compose pull && up -d <service>`).
- Edit env vars (plain and vault-backed) → writes the compose file / vault.
- The encrypted vault (section 4), audit log page, type-the-name confirmations.

### Phase 3: builder
- Canvas: cards you can place; networks drawn as lines between cards.
- "Add service" from a Docker Hub image (search + tags), env, volumes, exposure level.
- Create/attach networks and volumes.
- Writes normal `compose.yml` into `~/services/<name>/`, shows a **diff before saving**, keeps
  the previous file in `~/services/<name>/.deck/history/`, then runs `docker compose up -d`.
  (Comments in a hand-written file are lost on the first Deck edit; the diff warns you.)

### Phase 4: go live
- "Go live" on a port: pick a subdomain of a zone (e.g. `aeonlogics.dev`).
- Deck calls the Cloudflare API: adds an ingress rule to the tunnel's **remote
  configuration** (`PUT /accounts/{acct}/cfd_tunnel/{id}/configurations`) and a proxied
  `CNAME <sub> → <tunnel-id>.cfargotunnel.com` DNS record. No editing of
  `~/.cloudflared/config.yml`, no sudo.
- Needs a one-time switch of the tunnel from locally-managed to **dashboard-managed**
  (Cloudflare dashboard → Tunnels → Migrate), which imports today's ingress rules. To be
  tested on a spare hostname first.
- API token scopes: *Account → Cloudflare Tunnel: Edit*, *Zone → DNS: Edit* (only the zones you
  pick). Stored in the vault.
- "Take offline" removes the rule + DNS record (type-the-name confirm).

## 6. Not doing
- Multi-user / teams, multiple servers, building images from source, Kubernetes.
- Exposing Deck itself to the internet, ever.
