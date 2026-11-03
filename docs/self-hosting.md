# Self-hosting OpenPomodoro

OpenPomodoro is one program with the web app built into it, and one SQLite file for the data. You run it on a
machine you control; the people you give accounts to use it from their browsers (and can install it as an app).

## Quick start

### The program

```sh
scripts/build-release.sh              # needs Rust and Node; produces target/release/op-server
OP_PUBLIC_URL=http://localhost:3000 ./target/release/op-server
```

Open the address, create the administrator account (the first account is always the admin), and you are running.
Copy `op-server` anywhere you like; it needs nothing next to it. It creates its data folder on first start
(`data/openpomodoro.db` beside where you run it, or wherever `OP_DB` says).

### Docker

```sh
OP_PUBLIC_URL=https://pomodoro.example.com docker compose up -d
```

This builds the image and keeps the data in a Docker volume. The container is reachable on `127.0.0.1:3000` of the
host only, so put a reverse proxy in front (below). **The `Dockerfile` has not yet been built in CI** and was written
on a machine without a running Docker; if the build fails, please open an issue.

## Settings

All settings are environment variables.

| Variable | Default | Meaning |
|---|---|---|
| `OP_PUBLIC_URL` | none | The address people type, such as `https://pomodoro.example.com` (scheme, host, optional port; no path). The server only answers requests for this host and refuses others. **Required** unless it listens on loopback only. With `https` the session cookie is marked `Secure`. |
| `OP_BIND` | `127.0.0.1` | Address to listen on. Use `0.0.0.0` to accept connections from other machines (then `OP_PUBLIC_URL` is required). |
| `OP_PORT` | `3000` | Port. |
| `OP_DB` | `data/openpomodoro.db` | The SQLite file. Its folder is created if missing. |
| `OP_TRUST_PROXY` | off | Set to `true` when a reverse proxy is in front, so sign-in rate limits use the client address the proxy reports. Leave it off otherwise: a client could claim any address. |
| `OP_WEB_DIR` | none | Serve the web app from this folder instead of the copy inside the program (for development). |

`op-server healthcheck` asks a running server whether it is healthy and exits 0 or 1. The Docker image uses it.

## TLS and a reverse proxy

The server speaks plain HTTP. Put a reverse proxy in front for HTTPS, which is needed for the browser to offer
installation and keep notifications working. Two things matter: pass the `Host` header through unchanged (it must
equal `OP_PUBLIC_URL`'s host), and allow WebSocket upgrades (live updates between devices use `/api/ws`).

**Caddy** (gets certificates automatically):

```
pomodoro.example.com {
    reverse_proxy 127.0.0.1:3000
}
```

**nginx**:

```nginx
server {
    listen 443 ssl;
    server_name pomodoro.example.com;
    # ssl_certificate and ssl_certificate_key as usual

    client_max_body_size 32m;                 # restoring a large backup posts one big request

    location / {
        proxy_pass http://127.0.0.1:3000;
        proxy_set_header Host $host;
        proxy_set_header X-Forwarded-For $remote_addr;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

With either, set `OP_PUBLIC_URL=https://pomodoro.example.com` and `OP_TRUST_PROXY=true`.

## Accounts

- The first account created on a fresh install is the administrator.
- New people can be added by the admin (Settings, then Manage users), or the admin can switch on "Let people sign
  up". Registration is closed by default.
- There is no email: if someone forgets their password, the admin sets a temporary one for them.
- Everyone's data is private to their account. The admin can manage accounts but never reads anyone's sessions.

## Backups

- **Each person** can download a JSON backup from Settings ("Your data") and restore it into any account on any
  server. Restoring the same file twice changes nothing.
- **The admin** can download the whole database file from the Manage users screen. It holds every account's data
  and the password hashes, so keep it as safe as the server.
- You can also copy the file yourself while the server runs: use `sqlite3 data/openpomodoro.db ".backup backup.db"`
  (a plain file copy can miss recent writes, because the database uses a write-ahead log).

## Upgrading

Stop the old program, start the new one against the same data. The database is migrated on start. A database
written by a newer version is refused (the server says so and exits) rather than damaged, so keep the old program
if you might go back, and take a backup before upgrading.

People using the app are offered an "Update now" prompt the next time they open it.

## Good to know

- Alerts (sounds and notifications) work while the app is open in a tab or an installed window. A closed app
  cannot alert anyone; browsers do not allow it without a push service, which this version does not include.
- A device that loses the connection keeps timing locally and sends what happened when it is back.
- The server logs to the standard output: a start-up line, and internal errors. It records nothing about what
  people do and sends nothing anywhere.

## If something does not work

- **403 on every request, or a blank page:** the address in the browser does not match `OP_PUBLIC_URL`, or the
  proxy is not passing the `Host` header through.
- **"OP_PUBLIC_URL is required":** you set `OP_BIND` to something other than loopback without saying what address
  people will use.
- **Signed out all the time over HTTPS:** `OP_PUBLIC_URL` says `http://` while people use `https://` (or the
  reverse), so the cookie is not kept.
- **Live updates between devices do not arrive:** the proxy is not allowing WebSocket upgrades on `/api/ws`.
- **Everyone is rate limited together:** `OP_TRUST_PROXY` is off behind a proxy, so every request seems to come
  from the proxy's address.
