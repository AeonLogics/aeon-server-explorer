# Aeon Server Explorer: design

A private web UI that wraps Docker and Docker Compose on one server
(`aeon@192.168.100.26`). One admin, one server, opened in the browser through an SSH tunnel.
It is a Docker wrapper with extras, not a hosting platform: no Git builds, no webhooks, no
multiple servers.

## 1. Concepts

| UI word | What it really is | Where it lives |
|---|---|---|
| **Project** | one compose project | the folder `STACKS_DIR/<name>/` with its `compose.yml` |
| **Service** | one compose service (one or more containers) | a `services:` entry in that file |
| **Network** | a Docker network | `networks:` in the file; drawn as a line between the cards that share it |
| **Storage** | a named Docker volume or a bind mount | `volumes:` in the file (see §4) |
| **Exposure** | how a port is reachable (see below) | `ports:` in the file, plus a Cloudflare ingress rule for *public* |

**The `compose.yml` files are the source of truth.** The app never keeps its own copy of what
a project contains. It reads the running state from Docker and the desired state from the
files, and from phase 3 on it writes normal compose files and runs `docker compose`. Dockge,
Portainer and deploy scripts keep working on the same files.

### Folders on the server

```
/srv/aeon/                      one-time: sudo mkdir -p /srv/aeon/{stacks,data}
  stacks/                                 sudo chown -R aeon:aeon /srv/aeon
    .git/                       every save by the app is a commit (history + rollback)
    aeonlogics/
      compose.yml
    cake-delight/
      compose.yml
  data/
    aeonlogics/web/uploads/     bind-mounted storage (§4)
```

`STACKS_DIR` (default `/srv/aeon/stacks`) and `DATA_DIR` (default `/srv/aeon/data`) can be
overridden by env vars; locally they point into the repo's `data/` folder.

How the app finds projects:

1. Running/stopped containers, grouped by the label `com.docker.compose.project`. The label
   `com.docker.compose.project.working_dir` tells which folder it came from.
2. (Phase 3) folders under `STACKS_DIR` that have a `compose.yml` but no containers yet, shown
   as "not deployed".
3. Projects whose folder is outside `STACKS_DIR` (today's `~/services/<name>`) are shown
   normally but marked **"not adopted"**: read-only until adopted (phase 3).

Containers that are not from compose show up under a virtual project **"(standalone)"**.

### Networks

Compose creates `<project>_default` for every stack, so each project already has its own
internal network and services reach each other by name (`db:5432`). Extra networks are only
for services that must talk across projects.

### Exposure levels (phase 3)

| Level | compose output | Reachable from |
|---|---|---|
| **Internal** | no `ports:`, only `networks:` | other services on the same network |
| **Server** | `ports: ["127.0.0.1:<host>:<container>"]` | the server itself (and SSH tunnels) |
| **Public** | same as *Server* + a Cloudflare Tunnel ingress + DNS record (phase 4) | the internet, via Cloudflare |

Internal is the default. The app never writes `0.0.0.0` / bare `"<host>:<container>"` port
bindings. Existing files that have them are flagged with a warning on the card. Host ports are
suggested from the house convention (one `30x0` block per project: aeonlogics 3010,
cake-delight 3020, …) and checked against ports already in use.

## 2. Architecture

```
 your PC browser ──ssh -L 5050:localhost:5050──▶ server 127.0.0.1:5050
                                                   │
                                     ┌─────────────┴──────────────┐
                                     │ aeon-server-explorer        │
                                     │  Leptos 0.8 SSR + hydrate   │
                                     │  Axum: server fns + SSE     │
                                     │  bollard ─▶ /var/run/docker.sock
                                     │  SurrealDB v3 (embedded)    │
                                     │  docker compose CLI (ph.2+) │
                                     │  git (ph.3)                 │
                                     │  Cloudflare API (ph.4)      │
                                     └─────────────────────────────┘
```

- **Rust, Leptos 0.8 + Axum**, built with `cargo-leptos` (SSR + hydrate), same layout and
  patterns as `R:\aeonlogics` (the reference project).
- **Docker**: `bollard` over the local socket (named pipe `//./pipe/docker_engine` on
  Windows/Docker Desktop, `/var/run/docker.sock` on the server) for status, inspect, logs and
  stats. Applying stacks uses the `docker compose` CLI (bollard can't run compose files),
  started with `tokio::process::Command` and explicit arguments, never through a shell.
- **One job per project at a time**: a per-project lock around validate → write → commit →
  `docker compose up`, so two clicks can't clash.
- **Live data**: logs and CPU/RAM stats are streamed with **Server-Sent Events** from plain
  Axum routes (`/api/logs/{id}`, `/api/stats/{id}`), checked against the session cookie. The
  browser uses `EventSource`; the stream stops when the panel closes. Lists (projects,
  cards) are normal Leptos server functions, refreshed every few seconds.
- **Storage**: SurrealDB v3, **embedded** (`surrealkv://data/explorer.db`) by default, so the
  app is a single process with no extra container. `SURREAL_URI` can point at a server
  (`ws://…`) instead. It holds: the admin user, sessions, the audit log and (phase 2) the
  vault. It never holds a project's config; that stays in the files.
- **Code layout**

  ```
  src/
    app.rs            shell, router
    model.rs          shared types (ProjectSummary, ServiceCard, ServiceDetail, …)
    auth/             login, sessions, AuthStore (patterns from aeonlogics, single admin)
    docker/           ssr only: bollard client, project/service queries, SSE routes
    compose/          ssr only (phase 3): read/write files, safety rules, compose CLI, git
    guard.rs          ssr only: loopback-only bind check, Host-header check
    audit.rs          ssr only: audit log writes
    pages/            layout (sidebar), project canvas, service panel, login
    components/       icons (leptos_icons + icondata Lucide), status badges, log viewer
  surreal_flux/schema/*.surql   applied on every start (OVERWRITE)
  ```

- **On the server (later, only when you say so)**: a container with `network_mode: host`,
  the Docker socket and `/srv/aeon` mounted at the same path, `docker` CLI + compose plugin and
  `git` inside. The image is built on the PC (`docker build`), then
  `docker save | ssh … docker load` (the server has 1 CPU, so no Rust builds there). App data
  in `/srv/aeon/data/aeon-server-explorer/`.

## 3. Security model

**Controlling Docker = root on the server**, and so does editing a compose file (one line like
`- /:/host` is enough). Everything below follows from that.

1. **Localhost only.** The app refuses to start unless it binds a loopback address
   (`127.0.0.1` / `::1`). On the server it runs with `network_mode: host` so the bind really is
   the host's loopback. You reach it with:
   `ssh -L 5050:localhost:5050 aeon@192.168.100.26` → `http://localhost:5050`.
   It is **never** added to the Cloudflare Tunnel or published on the LAN.
2. **Host-header check** (stops DNS-rebinding): requests whose `Host` is not
   `localhost:<port>` / `127.0.0.1:<port>` are rejected with 403.
3. **Login**: one admin, no sign-up. The account is created on first start from
   `ADMIN_EMAIL` / `ADMIN_PASSWORD` and the password must be changed on first login
   (min 12 chars). Argon2id in SurrealQL. 5 wrong tries → locked 15 min.
4. **Sessions**: random 256-bit token in a `__Host-explorer_session` cookie
   (`HttpOnly; Secure; SameSite=Strict`); the DB stores only its SHA-256. 12 h maximum,
   ends after 30 min idle. Every server function and SSE route checks it.
5. **Audit log**: every login (ok and failed), logout, and (phase 2+) every action that changes
   something: who, what, which project/service, when, result. Read-only page in the UI;
   never edited or deleted from the UI.
6. **Destructive actions** (phase 2+: stop, remove, delete volume, delete project, take
   offline): a dialog where you **type the name** of the thing to confirm.
7. **No secrets in the browser**: no localStorage/sessionStorage, no secrets in server-fn
   responses except an explicit 30-second "reveal", env values shown masked.
8. **Compose safety rules** (phase 3). Before saving, the file goes through
   `docker compose config` (the normalised result is what gets checked) and the app refuses:
   - bind mounts whose host path is outside `DATA_DIR` (after resolving `..` and symlinks)
   - anything mounting `docker.sock` (or any path under `/var/run`, `/run`, `/proc`, `/sys`,
     `/etc`, `/root`, `/home`)
   - `privileged: true`, `network_mode: host`, `pid: host`, `ipc: host`, `userns_mode: host`
   - `cap_add`, `devices`, `security_opt` that disables AppArmor/seccomp
   - port bindings on anything but `127.0.0.1` (warning on adopted files, refused on new ones)

   Adopted files that break a rule stay read-only in the app until fixed; the card says why.

## 4. Storage (phase 4)

You choose what the container needs; the app picks the host side.

| Kind | Used for | compose output |
|---|---|---|
| **Volume** | database data (Postgres, SurrealDB, Redis) | named volume `<project>_<name>`, Docker manages it |
| **Folder** | files you want to see or back up (uploads, media) | bind mount `DATA_DIR/<project>/<service>/<name>:<container path>` |
| **Config** | config files the app reads | bind mount as above, `:ro` |

- You only type the container path (e.g. `/app/uploads`). The app creates the host folder.
- Ownership: the folder is created for the container's user (from the image's `USER`, or a
  UID you set), so uploads don't fail with "permission denied".
- The service panel shows each mount with its size.
- Backup = `DATA_DIR` plus a dump of each named volume. Deleting storage is a type-the-name
  action and never happens as a side effect of removing a service.

## 5. Secrets vault (phase 2)

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
- **How secrets reach containers**: compose files reference `${NAME}`; the app runs
  `docker compose up` with those values in the process environment, so no plaintext `.env` is
  written. (Per project you can switch to "also write `.env` (0600)" so deploy scripts/Dockge
  can redeploy without the app; off by default. `.env` is always in the stacks repo's
  `.gitignore`.) Note: Docker itself shows container env in `docker inspect`; that's outside
  the app's control.
- The Cloudflare API token (phase 4) is a vault secret too.

## 6. Roadmap

### Phase 1: read-only dashboard (this session)
- Login for the single admin, forced password change, logout, audit of logins.
- Sidebar: compose projects (with running/total count and a status dot), plus "(standalone)".
- Project view: a grid of service cards: name, image, status/health, published ports
  (with a warning for non-localhost binds), networks as coloured chips.
- Service panel: container details, env **names** (values hidden), mounts, **live logs**
  (SSE, follow, last 300 lines, auto-scroll, pause) and **live CPU/RAM**.
- Runs locally against Docker Desktop.

### Phase 2: operate
- Start / stop / restart / pull / redeploy (`docker compose pull && up -d <service>`).
- Edit env vars (plain and vault-backed).
- The encrypted vault (§5), audit log page, type-the-name confirmations.

### Phase 3: stacks on disk
- **Adopt**: move a stack from `~/services/<name>` into `STACKS_DIR` (copy, `docker compose
  down` in the old folder, `up -d` in the new one, a few seconds of downtime, type-the-name
  confirm). Same project name, so volumes and networks are kept.
- "Add service" from a Docker Hub image (search + tags), env, storage, exposure level.
- Edit a stack as a form or as raw YAML. Every save: safety rules (§3.8) → **diff before
  saving** → write → `git commit` in `STACKS_DIR` → `docker compose up -d`. History page with
  one-click rollback (a revert commit + `up -d`). Comments in a hand-written file are lost on
  the first form edit; raw YAML edits keep them, and the diff warns you.
- Create/attach extra networks; canvas with networks drawn as lines between cards.

### Phase 4: storage and go live
- Storage helper (§4).
- "Go live" on a port: pick one of your Cloudflare zones (listed from the API, e.g.
  `aeonlogics.dev`) and a subdomain or the apex.
- The app calls the Cloudflare API: adds an ingress rule to the tunnel's **remote
  configuration** (`PUT /accounts/{acct}/cfd_tunnel/{id}/configurations`) pointing at
  `http://localhost:<host port>`, and a proxied `CNAME <name> → <tunnel-id>.cfargotunnel.com`
  DNS record. No editing of `~/.cloudflared/config.yml`, no sudo.
- Needs a one-time switch of the tunnel from locally-managed to **dashboard-managed**
  (Cloudflare dashboard → Tunnels → Migrate), which imports today's ingress rules. To be
  tested on a spare hostname first.
- API token scopes: *Account → Cloudflare Tunnel: Edit*, *Zone → Zone: Read*,
  *Zone → DNS: Edit* (only the zones you pick). Stored in the vault.
- "Take offline" removes the rule + DNS record (type-the-name confirm).

## 7. Not doing
- A hosting platform: building images from source, Git integration, webhooks, preview
  deployments.
- Multi-user / teams, multiple servers, Kubernetes, a web terminal.
- Exposing the app itself to the internet, ever.
