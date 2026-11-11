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

**Docker and the compose files are the source of truth.** The app reads the running state
from Docker. Hand-written stacks are only read; **managed stacks** (§4) are the ones the app
writes, from a form. Dockge, Portainer and deploy scripts keep working on the same files.

### Folders on the server

Every stack has its own folder in `STACKS_DIR` (server: `/home/aeon/services`, the existing
convention; locally: `data/stacks`). Hand-written stacks and managed stacks sit side by side;
a managed one is recognised by its `explorer.json`.

How the app finds projects:

1. Running/stopped containers, grouped by the label `com.docker.compose.project`. The label
   `com.docker.compose.project.working_dir` tells which folder it came from.
2. Managed stacks that have no containers yet are listed on the Stacks page as "not deployed".

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
  (`ws://…`) instead. It holds the admin user, sessions and the audit log,
  nothing else. It never holds a project's config; that stays in the files.
- **Code layout**

  ```
  src/
    app.rs            shell, router
    model.rs          shared types (ProjectSummary, ServiceCard, ServiceDetail, …)
    auth/             login, sessions, AuthStore (patterns from aeonlogics, single admin)
    docker/           ssr only: bollard client, project/service queries, SSE routes
    backend/stacks/   ssr only (phase 3): managed stacks, compose generator, apply
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
8. **Safe by construction** (phase 3). Managed stacks are generated from a form that has no
   field for `privileged`, host network/pid/ipc, devices, capabilities, bind mounts or
   `docker.sock`, and only publishes ports on `127.0.0.1` (§4). Hand-written stacks are never
   edited by the app.

## 4. Managed stacks (phase 3)

A **managed stack** is a compose project the app created and owns. You build it in a form;
the app writes the files and runs `docker compose`. Hand-written stacks (aeonlogics,
cake-delight, …) stay as they are: you can view, restart and redeploy them, never edit them
here, so the app can't wreck a file it didn't write.

### Files

```
STACKS_DIR/<stack>/             server: /home/aeon/services   local: data/stacks
  explorer.json                 the form's model (no secret values) + hash of what was applied
  compose.yml                   generated from explorer.json; edits by hand are overwritten
  env/<service>.env             KEY=value, one per line, 0600, never shown unless revealed
```

- `compose.yml` is output; `explorer.json` is the truth for managed stacks. A folder that
  already exists (managed or not) can't be used for a new stack, and neither can a name that a
  running compose project already has.
- Env values live only in `env/<service>.env`, referenced with `env_file: {path, format: raw}`
  so values are taken literally (no `$` interpolation). Values can't contain newlines.

### What a service can have

| Field | Rule | compose output |
|---|---|---|
| Name | `a-z 0-9 - _`, starts with a letter or digit, ≤ 40 | `services.<name>` |
| Image | one of the images already on the server (`docker images`) | `image:` |
| Restart | no / always / unless-stopped (default) / on-failure | `restart:` |
| Ports | *internal* (others in the stack only) or *server* `127.0.0.1:<host>` | `expose:` / `ports:` |
| Storage | a name + an absolute path inside the container | named volume `<stack>_<name>` |
| Env | names `A-Z a-z 0-9 _`, values in the env file | `env_file:` |

There is no field for `privileged`, host network or pid, devices, capabilities, or bind mounts,
so the generated file can't contain them. Host ports must be 1024–65535 and free (not used by
any container, nor by another managed stack).

### Applying

Save writes `explorer.json`, `compose.yml` and the env files. **Apply** validates with
`docker compose config -q`, then runs `docker compose up -d --remove-orphans` (one action per
stack at a time) and stores the hash of the applied files. When the files differ from that
hash, the stack shows **Changes pending** with an Apply button.

### Storage tab

- Storage is always a **named volume**: Docker sets the right owner from the image, so
  Postgres, Node, etc. can write without permission fixes.
- Per service: each volume's size (from Docker's disk usage) next to the server's free disk
  space, with a warning under 10 % free.
- File browser: a throwaway `busybox` container mounts the volume **read-only** and lists it
  (`ls`, `du`), then is removed, so browsing can never change anything. Download comes later;
  editing or deleting files from the UI is not planned.

### Secrets

No vault for now: values sit in the 0600 env files and are never part of page data. The
Environment tab shows `••••`; one click fetches that single value for 30 seconds, and the
reveal is audited. A vault can be added later behind the same UI if backups start leaving
the server.

## 5. Roadmap

### Phase 1: read-only dashboard (done)
Login, sessions, overview, projects, service pages, live logs and CPU/RAM.

### Phase 2: operate (done)
Start / stop / restart / redeploy, type-the-name stop, audit log page, and the app can't stop
or redeploy itself.

### Phase 3: managed stacks (§4)
1. Stack model, compose generator, create / save / apply.
2. Service form: image from installed ones, env, ports, storage, restart policy.
3. Changes pending, port-clash check, reveal one env value on click.
4. Storage tab: volume sizes, free disk, read-only file browser.
5. Polish, then deploy (the app's container needs write access to `/home/aeon/services`).

Later: recipes (Postgres, Redis, … presets that ask only for what matters), git history with
rollback, pulling new images.

### Phase 4: go live
- "Go live" on a port: pick one of your Cloudflare zones (listed from the API, e.g.
  `aeonlogics.dev`) and a subdomain or the apex.
- The app calls the Cloudflare API: adds an ingress rule to the tunnel's **remote
  configuration** (`PUT /accounts/{acct}/cfd_tunnel/{id}/configurations`) pointing at
  `http://localhost:<host port>`, and a proxied `CNAME <name> → <tunnel-id>.cfargotunnel.com`
  DNS record. No editing of `~/.cloudflared/config.yml`, no sudo.
- Needs a one-time switch of the tunnel from locally-managed to **dashboard-managed**
  (Cloudflare dashboard → Tunnels → Migrate). To be tested on a spare hostname first.
- API token scopes: *Account → Cloudflare Tunnel: Edit*, *Zone → Zone: Read*,
  *Zone → DNS: Edit* (only the zones you pick), kept in a 0600 file on the server.
- "Take offline" removes the rule + DNS record (type-the-name confirm).

## 6. Not doing
- A hosting platform: building images from source, Git integration, webhooks, preview
  deployments.
- Multi-user / teams, multiple servers, Kubernetes, a web terminal.
- Exposing the app itself to the internet, ever.
