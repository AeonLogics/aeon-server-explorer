# Deployment

The image is built on your own machine and copied to the server, so the server never compiles
Rust. Replace `user@your-server` below with your SSH login.

## 1. Build the image

```powershell
docker build -t aeon-server-explorer:latest .
```

The first build takes a while (SurrealDB is large); later builds reuse the cache.

## 2. Copy it to the server

Use a file rather than a pipe: PowerShell 5.1 corrupts binary pipes.

```powershell
docker save -o explorer.tar aeon-server-explorer:latest
scp explorer.tar user@your-server:/tmp/explorer.tar
ssh user@your-server "docker load -i /tmp/explorer.tar && rm /tmp/explorer.tar"
Remove-Item explorer.tar
```

## 3. First time: compose file and settings

```powershell
ssh user@your-server "mkdir -p ~/services/aeon-server-explorer"
scp deploy/compose.yml user@your-server:~/services/aeon-server-explorer/compose.yml
scp deploy/.env.example user@your-server:~/services/aeon-server-explorer/.env
```

On the server:

```bash
cd ~/services/aeon-server-explorer
chmod 600 .env
getent group docker | cut -d: -f3   # put this number in DOCKER_GID
nano .env                           # ADMIN_EMAIL, ADMIN_PASSWORD, DOCKER_GID, STACKS_DIR
```

`STACKS_DIR` is the folder with one subfolder per compose stack (for example `/home/you/services`).
It's mounted at the same path inside the container, so compose files and their relative paths
keep working. For stacks created in the app it must be writable by the container's user
(uid 10001).

## 4. Start it

```bash
cd ~/services/aeon-server-explorer
docker compose up -d
docker compose logs -f
```

Wait for `listening on http://localhost:5050`.

## 5. Open it from your PC

```powershell
ssh -N -L 5050:localhost:5050 user@your-server
```

Then browse to **http://localhost:5050**. Use 5050 on both sides: the app rejects any other
`Host`. Sign in with `ADMIN_EMAIL` / `ADMIN_PASSWORD` and choose a new password; after that you
can delete `ADMIN_PASSWORD` from `.env`.

### An always-on tunnel (Windows)

```powershell
powershell -ExecutionPolicy Bypass -File deploy\install-tunnel.ps1 -Server user@your-server
```

This registers a hidden task that starts at every login and keeps the tunnel open, reconnecting
after sleep or network drops. Remove it with
`Unregister-ScheduledTask -TaskName "Aeon Explorer tunnel" -Confirm:$false`.

## Updating

Repeat steps 1, 2 and 4. The database lives in the `explorer-data` volume and survives updates.

## Settings

| Variable | Default | What it does |
|---|---|---|
| `ADMIN_EMAIL` / `ADMIN_PASSWORD` | | the first admin; only read while no admin exists |
| `DOCKER_GID` | | the server's docker group, so the app may use the socket |
| `STACKS_DIR` | `data/stacks` | the folder with one subfolder per stack |
| `SURREAL_URI` | `surrealkv://data/explorer.db` | the database; a `ws://` URI also needs `SURREAL_USER` / `SURREAL_PASS` |
| `LEPTOS_SITE_ADDR` | `127.0.0.1:5050` in the image | listen address; must be loopback |
