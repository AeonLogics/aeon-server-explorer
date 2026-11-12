# Aeon Server Explorer

**A friendly web UI for managing Docker on your own server.**

Open it in your browser and see every compose project and container at a glance, read live logs
and CPU/RAM, and start, stop or restart services. Build new stacks from a form instead of
hand-editing YAML, pull images from Docker Hub, keep env values in private files, and get an
audit log of everything that changed.

It's made for one admin on one machine. It listens only on loopback and is reached through an
SSH tunnel, never exposed to a network.

**Stack:** Rust · Leptos 0.8 (SSR + hydrate) · Axum · bollard · SurrealDB 3 (embedded) · SCSS

## What it does

- **Overview:** a status sentence, one square per container, a "Needs you" list with fix
  buttons, recent activity and project tiles.
- **Projects and services:** health, ports (with a warning for anything beyond localhost),
  networks, mounts, env names, live logs and CPU/RAM; Start, Stop, Restart, Redeploy.
- **Managed stacks:** create a stack, add services from installed images, and set ports,
  storage, env (one by one or as a whole file), restart policy, start command and the user
  it runs as. Edit several services, then **Save & deploy** once. The app writes the compose
  file, deploys it, and keeps the last deploy's full output.
- **Images:** what's on the server, Docker Hub search with tags, pulls with live progress.
- **Audit log:** sign-ins, actions, deploys, pulls and revealed values.

More in [docs/features.md](docs/features.md).

## Roadmap

- [ ] **Smarter ports:** fill in the port an image listens on, and check that a server port is
  free outside Docker too
- [ ] **Cloudflare Tunnel:** run `cloudflared` from a tunnel token and join it to your stacks'
  networks; later, a "public on the internet" switch per port that creates the route and DNS
  record. The explorer itself always stays SSH-only.
- [ ] Delete a stack from the UI
- [ ] Storage browser: what's inside a volume and how full it is
- [ ] Show which containers use an image before removing it

Ideas and issues are welcome.

## Quick start (local)

You need Rust (stable), [`cargo-leptos`](https://github.com/leptos-rs/cargo-leptos), the
`wasm32-unknown-unknown` target and a running Docker engine (Docker Desktop on Windows/macOS).

```powershell
Copy-Item .env.example .env      # then set ADMIN_EMAIL and ADMIN_PASSWORD
cargo leptos watch
```

Open **http://localhost:5055**, sign in, and choose a new password (12+ characters).
Details in [docs/development.md](docs/development.md).

## Documentation

| Doc | What's in it |
|---|---|
| [Features](docs/features.md) | Every page and what you can do there |
| [Architecture](docs/architecture.md) | Crates, request flow, live streams, data |
| [Managed stacks](docs/managed-stacks.md) | How the form becomes a compose file and gets deployed |
| [Security](docs/security.md) | The threat model and every guard in place |
| [Deployment](docs/deployment.md) | Building the image, running it on a server, the SSH tunnel |
| [Development](docs/development.md) | Running locally, project layout, gotchas |

## License

[MIT](LICENSE) © 2026 Asad Ali, Aeon Logics
