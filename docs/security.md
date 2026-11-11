# Security

**Whoever controls Docker controls the server.** Anyone using this app could start a container
that mounts the whole disk. Everything below follows from that.

## Reachability

- **Loopback only.** The app refuses to start unless it listens on `127.0.0.1` / `::1`. On a
  server it runs with host networking, so that is the server's own loopback. You reach it with
  an SSH tunnel (`ssh -L 5050:localhost:5050 user@your-server`). It is never meant for a LAN,
  a reverse proxy or the internet.
- **Host check.** Every request whose `Host` header isn't `localhost:<port>` or
  `127.0.0.1:<port>` gets a 403. A malicious website can point its own domain at `127.0.0.1`
  (DNS rebinding), but it can't make your browser send that `Host`.

## Signing in

- **One admin, no sign-up.** Created on first start from `ADMIN_EMAIL` / `ADMIN_PASSWORD`; the
  password must be changed on first login (12 to 128 characters).
- **Argon2id** hashing, done in SurrealQL. Unknown emails still run a hash, so timing doesn't
  reveal which emails exist.
- **Lockout:** 5 wrong passwords lock sign-in for 15 minutes.

## Sessions

- A random 256-bit token in a `__Host-explorer_session` cookie: `HttpOnly; Secure;
  SameSite=Strict; Path=/`. The browser never attaches it to requests started by another site.
- The database stores only the token's SHA-256.
- Sessions last at most 12 hours and end after 30 minutes without activity. Background refreshes
  and live streams don't count as activity, so an open tab still times out.
- Changing the password signs out every other session.

## Actions

- Every server function and stream checks the session first, and refuses while a password
  change is still required.
- Stop needs the container's name typed, and the server checks it too.
- The browser only ever sends a container or stack name. The project, compose files and paths
  are looked up from Docker and from disk on the server.
- The app won't stop or redeploy its own container (labelled `dev.aeonlogics.explorer.self`),
  so it can't switch itself off mid-action.
- `docker compose` runs with explicit arguments, never through a shell.

## Managed stacks

- Generated from a form with no option for privileged mode, host networking, devices,
  capabilities, bind mounts or `docker.sock`. Ports are only published on `127.0.0.1`.
- Env values live in files with mode 0600, are never part of page data, and are only sent to
  the browser when you reveal them, which is audited.
- Values in the command line are visible in `compose.yml` and `docker inspect`, so secrets
  belong in env.

## Audit log

Append-only in the database (no update or delete in the app): sign-ins, failures, lockouts,
sign-outs, password changes, container actions, stack saves and deploys, revealed values, image
pulls and removals, each with the admin, IP, user agent, target and result.

## The container on a server

`deploy/compose.yml` runs the image as a non-root user, with a read-only filesystem,
`no-new-privileges`, and only the Docker socket, the stacks folder and its data volume mounted.
