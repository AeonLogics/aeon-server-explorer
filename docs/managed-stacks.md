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
| Ports | the port the app listens on; reachable inside the stack, or also on `127.0.0.1:<port>` | `expose:` or `ports: ["127.0.0.1:host:container"]` |
| Storage | a name and an absolute path inside the container | a named volume `<stack>_<service>-<name>` |
| Env | names `A-Z a-z 0-9 _`; values in the env file | `env_file: {path, format: raw}` |

There is no setting for privileged mode, host networking, devices, capabilities or bind mounts,
and ports are only ever published on `127.0.0.1`. The generated file can't contain them.

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
