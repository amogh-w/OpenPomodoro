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
| `OP_HOOKS_FILE` | none | A JSON file of lifecycle hooks (see [Hooks](#hooks)). |
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

## Hooks

Hooks run a program on the server when a person's timer changes phase, for example to switch a light or post to
a chat. They run programs, so only you (the server's admin) set them, in a JSON file that `OP_HOOKS_FILE` points
to. The web app can show them and test them (Admin screen) but never change them.

```json
{
  "hooks": [
    { "event": "focus_started", "command": ["/usr/local/bin/lights", "red"] },
    { "event": "focus_ended", "command": ["/usr/local/bin/lights", "green"], "timeout_s": 5 },
    { "event": "*", "command": ["/usr/local/bin/log-event"], "enabled": false }
  ]
}
```

- `event` is one of `focus_started`, `focus_ended` (time ran out), `focus_stopped` (finished by hand),
  `focus_abandoned`, `paused`, `resumed`, `break_started`, `break_ended` (time ran out), `break_stopped`, or `*`
  for all of them.
- `command` is the program and its arguments. It is run directly, never through a shell.
- `timeout_s` (1 to 60, default 10): the program is killed after this long. `enabled: false` switches a hook off
  without deleting it.
- The program gets the event as JSON on standard input (`event`, `at` in Unix milliseconds, `user.id`,
  `user.username`, and the user's `timer` state) and the event name in the `OP_EVENT` environment variable.
- Events are queued; if a program is slow they wait, and if the queue overflows new events are dropped (with a
  line in the server's output). A hook can never delay or block the timer.
- A bad file stops the server from starting, with the reason printed. Every user's timer triggers the hooks, so
  only list programs you trust with everyone's activity.

On a server with Docker, the program has to exist inside the container.

### Webhooks

The same file can send events to a web address instead of running a program:

```json
{
  "webhooks": [
    { "event": "focus_started", "url": "https://example.com/hooks/focus", "secret": "a long random string" }
  ],
  "allow_private_webhooks": false
}
```

- The body is the same JSON the hooks get, sent as a `POST` with `Content-Type: application/json`, an
  `X-OpenPomodoro-Event` header and a `User-Agent` of `OpenPomodoro/<version>`.
- With a `secret`, each request also has `X-OpenPomodoro-Signature: sha256=<hex>`, the HMAC-SHA256 of the exact
  body bytes using the secret as the key. Check it before trusting a request.
- `timeout_s` and `enabled` work as for hooks. A network error or a 5xx answer is tried once more after two
  seconds; nothing else is retried, and failures only appear in the server's output.
- Addresses on this machine or your local network (`127.0.0.1`, `10.x`, `192.168.x`, `169.254.x`, unique-local
  IPv6 and similar) are refused, including names that resolve to them, unless `allow_private_webhooks` is `true`.
  Redirects are never followed.
- The Admin screen lists the webhooks (never their secrets) and has a Test button for each.

## Two-step sign-in

Each person can turn on two-step sign-in in Profile. It uses any authenticator app (time-based codes, SHA-1, 6 digits,
30 seconds). Turning it on needs the account password, a code from the app to prove it works, and gives eight
one-time recovery codes (stored only as hashes; shown once). A code works once, and wrong codes count toward the same
sign-in lockout as wrong passwords. Turning it off needs the password. There is no QR code yet: the key is shown to
type in (or open as an `otpauth://` link). If someone loses their app and their recovery codes, an admin can press "Reset two-step" next to their name on the Admin screen: it switches two-step off for that account and signs them out everywhere, and they can turn it on again after signing in with their password.
