# Features

Everything refreshes on its own every 5 seconds; logs and CPU/RAM are live.

## Overview

- **Headline:** one sentence about the server, with the key part coloured: "**5 containers** are
  stopped.", "**Everything** is running." Under it, health and port warnings in plain words.
- **Server facts:** Docker version, cores, memory, and counts of running containers, projects
  and images.
- **Container strip:** one square per container, grouped by project. Green is running, amber is
  starting or paused, a red outline is stopped or failing. Hover for its status, click to open.
- **Needs you:** stopped containers (with a **Start** button right there), failing health
  checks, ports open beyond localhost, and managed stacks with changes that aren't live.
- **Recent activity:** the last few audit entries, linking to the full log.
- **Project tiles:** running count, a small square per container, and the service names.

## Projects and services

Projects are compose projects, grouped by Docker's `com.docker.compose.project` label.
Containers without it are listed under **(standalone)**.

- **Project page:** one card per service with image, status, ports and networks. A port bound to
  all interfaces (`0.0.0.0`) gets a warning chip.
- **Service page:** live logs, live CPU/RAM sparklines, container facts, env variable **names**
  (never values), mounts, networks with IPs.
- **Actions:** Start, Stop (type the container's name to confirm), Restart, and Redeploy
  (`docker compose pull` + `up -d` for that service, for hand-written stacks).
- **Logs:** last 300 lines on connect, then live; pause, follow, filter, wrap, clear. Error lines
  are marked. The stream ends on its own when the container stops.

Stacks created in the app open in the stack workbench instead (below).

## Managed stacks

Stacks the app creates and owns. See [managed-stacks.md](managed-stacks.md) for how they work.

- **Stacks page:** every managed stack and its state: Not deployed, Deployed, Changes pending.
- **Workbench:** services on the left with live status dots; an inspector on the right with:
  - **General:** live status, Start/Stop/Restart, image, restart policy, start command, live
    CPU/RAM and container facts, and Remove service.
  - **Ports:** the port the app listens on, and who may reach it: only the other services in
    the stack, or also `127.0.0.1:<port>` on the server. A sentence under each row says exactly
    who can connect.
  - **Storage:** named volumes by name and path inside the container.
  - **Env:** values hidden until revealed (30 seconds, audited), or **Edit as file** to see,
    edit or paste the whole `.env` at once.
  - **Logs:** that service's live logs.
- **Changes bar:** always visible. Unsaved edits → Save or Discard. Saved but not live → Apply.
  Live but a container isn't running → says so.
- **Last deploy:** the time and Docker's full output of the last deploy. A failure stays visible
  until the next deploy works, even after a refresh.

## Images

- **On this server:** every image with size, created date and how many containers use it.
  Unused images can be removed (Docker refuses to remove images any container still uses,
  running or stopped).
- **Docker Hub:** search with official images first, pick a tag (the 30 most recent, with
  sizes) or type one, and **Pull to server** with a live progress bar. Typing `name:tag` goes
  straight to the pull. A pull keeps running on the server even if you close the tab.
- **Upload an image:** build an image on your own machine, save it with
  `docker save -o app.tar app:latest`, and upload the `.tar` (or a `.tar.gz`). The file is
  streamed straight into Docker with a progress bar, so the server needs no space for a copy.
  Afterwards the app shows each loaded image with its platform and warns when it was built for
  a different CPU than the server's (for example `linux/arm64` on an `amd64` server).

## Audit log

Append-only, newest first: sign-ins (ok, failed, locked), sign-outs, password changes, container
actions, stack saves and deploys, revealed env values, image pulls and removals, each with who,
from which IP, the target and the result. Filter by All, Problems, Changes or Sign-ins, and
search.

## Account

One admin, created on first start from `ADMIN_EMAIL` / `ADMIN_PASSWORD`, who must choose a new
password (12+ characters) before anything else. See [security.md](security.md).
