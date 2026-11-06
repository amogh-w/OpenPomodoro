# API for scripts (version 1)

Your own scripts can read your data and control your timer through the API. Everything is scoped to the account
that owns the token. The routes under `/api/v1` will not change in a breaking way; a breaking change would arrive
as `/api/v2`. (The routes the web app itself uses, such as `/api/state`, are internal and may change.)

## Tokens

In the app: **Settings, Devices and scripts, Make a token**. The token (it starts with `opt_`) is shown once; the
server keeps only a hash. Remove a token there at any time and it stops working at once. Tokens are tied to the
account: a disabled account's tokens stop working, and anyone holding a token can do what you can, so keep them
private and never put one in a URL.

Send it in a header:

```
Authorization: Bearer opt_...
```

Use the server's own address (the `Host` must be the configured public host), for example
`https://focus.example.com`. These routes do not use cookies and need no `X-Requested-With` header.

## Routes

### `GET /api/v1/state`

The clock, the timer, the settings, the categories and the tasks:

```json
{
  "server_time_ms": 1793958000000,
  "timer": { "state": "session", "kind": "focus", "start": 1793957000000, "total": 1500000,
             "pause_buffer": 0, "pause_ts": null, "planned_ms": 1500000, "intention": "write",
             "category": null, "task": null, "rounds": 0, "...": "..." },
  "settings": { "focus_ms": 1500000, "short_break_ms": 300000, "long_break_ms": 900000,
                "rounds_before_long": 4, "breathe": false, "auto_start_break": false,
                "auto_start_focus": false, "strict": false },
  "categories": [ { "id": "...", "name": "Writing", "color": "peach" } ],
  "tasks": [ { "id": "...", "title": "Draft the report", "done": false } ]
}
```

`timer.state` is one of `idle`, `breathe`, `session`, `session_end` (focus overtime), `paused`, `reflect`, `rest`,
`rest_end` (break overtime). Time left is computed from the timestamps: while running, elapsed is
`server_time_ms - start - pause_buffer`; the phase ends when elapsed reaches `total`.

### `POST /api/v1/commands`

Run a timer command. The body is JSON:

| Field | Meaning |
|---|---|
| `id` | A unique id you make up (required). Sending the same id again does nothing the second time, so retries are safe. Ids look like `<UTC yyyyMMddHHmmss>.<5 digits>@<your name>`. |
| `name` | `init_focus`, `start_focus`, `skip_breathe`, `timer_ends`, `toggle_pause`, `extend`, `stop`, `abandon`, `take_rest`, `done`, `skip_rest`, `edit_session` |
| `intention`, `category`, `task` | For `init_focus` and `edit_session`. |
| `duration_ms` | Focus length for `init_focus`, or break length for `take_rest` (5 seconds to 12 hours). |
| `delta_ms` | For `extend`: plus or minus whole 5-minute steps. |
| `notes`, `mood` | Saved with the session by `take_rest` or `done`. `mood` is `focused`, `neutral` or `distracted`. |

The answer is `{ "status": "applied" | "duplicate", "state": { ... } }`. A command that is not allowed right now
(for example pausing in strict mode) answers `409` with `rejected_strict`, `rejected_not_ended` or
`rejected_unknown`; a malformed one answers `400`.

```
curl -H "Authorization: Bearer $TOKEN" -H "Content-Type: application/json" \
  -d '{"id":"20261106100000.00001@desk","name":"init_focus","intention":"write","duration_ms":1500000}' \
  https://focus.example.com/api/v1/commands
```

### `GET /api/v1/sessions?from=<ms>&to=<ms>`

Sessions that started in `[from, to)` (Unix milliseconds), at most a year at a time and 5000 rows
(`"truncated": true` if there were more). Each has `kind` (`focus`, `short_break`, `long_break`), `started_at`,
`ended_at`, `duration_ms`, `planned_ms`, and for focus sessions `intention`, `category`, `task`, `notes`, `mood`.

## Live stream: `GET /api/v1/stream` (WebSocket)

Connect with the same `Authorization` header. The server sends

```json
{ "type": "state", "state": { "...": "same as GET /api/v1/state" } }
```

when you connect and again after every change to your data, so an overlay or a light never needs to poll. Your
client has nothing to send. The server pings every 30 seconds and closes the socket if the token is removed or
the account is disabled. One account may keep at most ten sockets open at once; an eleventh is closed with code
1013 (try again later).

## Errors

Errors are JSON, `{ "error": "<code>" }`, with the usual status: `401` (missing or unknown token), `400`
(malformed), `404`, `409` (not allowed in the current state), `413` (too large), `429` (rate limited).
