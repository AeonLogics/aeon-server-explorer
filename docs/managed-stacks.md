# Managed stacks

A managed stack is a compose project the app created and owns. You describe it in a form; the
app writes the files and runs `docker compose`. Stacks written by hand are never edited by the
app: you can view and operate them, nothing more.

## Files

Each stack is one folder in `STACKS_DIR` (locally `data/stacks`):

```
<stack>/
  explorer.json        the form's model, the last deploy result and a hash of what was applied
  compose.yml          generated from explorer.json; hand edits are overwritten
  env/<service>.env    KEY=value lines, mode 0600, only shown when you reveal them
```

Files are written to a temporary name and then renamed, so a crash never leaves half a file.

## What a service can have

| Setting | Rule | In compose.yml |
|---|---|---|
| Name | lowercase letters, digits, `-` and `_`, up to 40 | `services.<name>` |
| Image | one of the images already on the machine | `image:` |
| Restart | unless stopped (default), always, on failure, never | `restart:` |
| Command | optional; arguments written like on the command line | `command: ["start", "--log", "info"]` |
| Run as | optional; a user name or ID, like `0` or `1000:1000` | `user: "0"` |
| Ports | the port the app listens on; reachable inside the stack, also on `127.0.0.1:<port>`, or as a debug port on `0.0.0.0:<port>` | `expose:` or `ports: ["127.0.0.1:host:container"]` |
| Own storage | a name and an absolute path inside the container | a named volume `<stack>_<service>-<name>` |
| Shared storage | one of the stack's storages, a path, and read-only or not | `volumes: ["<storage>:<path>:ro"]` |
| Networks | `default` and any of the stack's extra networks | `networks: [...]` (left out when it's only `default`) |
| Start after | other services, each with *is running*, *is healthy* or *finished successfully* | `depends_on: {svc: {condition: ...}}` |
| Runs once | a helper that runs to the end and stays stopped | `restart: "no"` and a run-once label |
| Env | names `A-Z a-z 0-9 _`; values in the env file | `env_file: {path, format: raw}` |

There is no setting for privileged mode, host networking, devices, capabilities or bind mounts,
so the generated file can't contain them. Ports are published on `127.0.0.1`, unless you
explicitly pick a debug port.

## Shared storage, networks and start order

Besides services, a stack has **storages** and **networks**, each in its own group in the list.

- **A shared storage** is a named volume that belongs to the stack (`<stack>_<name>`). Services
  mount it from their **Storage** tab, read-write or read-only, so a helper or a backup service
  can work on the same data. A storage that any service uses can't be removed, and removing
  one never deletes the volume's data.
- **Networks.** Every stack has `default`, and all services are on it unless you change that in
  their **General** tab. Extra networks keep services apart: services only reach each other by
  name on a network they share.
- **Start after** makes a service wait for others: until they run, until their health check
  passes, or until they finished successfully. Loops (A waits for B, B waits for A) are refused.
- **Runs once** is for setup helpers. When one exits with code 0, it shows as **Finished**, not as
  stopped, and isn't listed in Needs you.

A common use: a new volume belongs to root, and some images (SurrealDB, for example) run as
another user. A **Runs once** helper fixes the owner once, and the database starts after it:

```
storage  db-data
db-init  alpine:3, Runs once, command: chown -R 65532:65532 /data,  db-data at /data
db       surrealdb/surrealdb, start after db-init (finished successfully), db-data at /data
```

## Debug ports

**Debug: open on all networks** publishes a port on `0.0.0.0`, so every network the server is on
can reach it: your LAN, and the internet if your router forwards the port. It's meant for short
debugging sessions: the row turns amber, Needs you lists it as long as it's open, and saving it is
recorded in the audit log. To reach a port from your own PC, an SSH tunnel
(`ssh -L 6390:localhost:6390 you@server`) works without opening anything.

Env values are read with `format: raw`, so a `$` in a value is taken literally. In the command,
each part is escaped and `$` is doubled for the same reason.

## Ports, in plain words

- **The container port** is the port the program listens on inside its container. It must match
  the program's own config.
- **"Only inside this stack":** other services reach it by name, like `db:5432`. Nothing else can.
- **"Also on the server":** Docker also opens `127.0.0.1:<port>` on the server machine, which
  only programs on the server can reach: an SSH tunnel, or a Cloudflare tunnel running there.
  Your LAN and the internet still can't.
- **Inside a container, `127.0.0.1` means that container only.** A program that should be
  reachable by other services must listen on `0.0.0.0` (for example SurrealDB:
  `start --bind 0.0.0.0:8000 …`).

## Storage, in plain words

Adding storage creates a named volume and mounts it at the path you choose. The program still
has to be told to save its data there (through its command or env). Use a folder meant for data,
like `/data` or `/var/lib/postgresql/data`, never a path the image uses for its own program.

A new volume belongs to root unless the image already has that folder. A program that runs as
another user (SurrealDB does) then fails with *Permission denied*. Set **Run as** to `0` for it.

## Saving and applying

Each service keeps its own edits while you switch between services. The changes bar lists the
edited ones; **Save** saves all of them at once and **Save & deploy** also applies the stack.

1. **Save** checks the form (names, installed images, free ports across all containers and
   stacks), then writes `explorer.json`, `compose.yml` and the env files.
2. **Apply** runs `docker compose config -q`, then `docker compose up -d --remove-orphans
   --pull never` for the whole stack. Only one action runs per stack at a time.
3. The result is kept as the **last deploy**, with Docker's output, and the hash of the applied
   files is stored. If the files later differ from that hash, the stack shows
   **Changes pending**.

A failed deploy never removes your saved changes; it only means they aren't live yet.
