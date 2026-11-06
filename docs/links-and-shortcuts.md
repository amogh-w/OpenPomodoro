# Links and keyboard shortcuts

## Links

These open the app and do something. They work as a hash on the app's address (`https://your-server/#/start?...`)
and as `web+openpomodoro:` links once the app is installed (the browser asks to register the handler).

| Link | What it does |
|---|---|
| `#/start?intent=write&duration=30&category=Deep%20work&notes=first%20draft` | Start a focus session. Every part is optional. `duration` is whole minutes (1 to 720); `category` is a category name (an exact name, or a prefix that matches only one). |
| `#/pause` / `#/resume` | Pause a running timer / resume a paused one. |
| `#/break` | Take a break (from the reflect screen, or when idle). |
| `#/finish` | Finish the focus session now. |
| `#/analytics` | Open History. Opening a screen needs no confirmation. |

Protocol form: `web+openpomodoro://start?intent=write&duration=30`.

**Links that change the timer ask first.** Any web page can open a link, so by default a dialog shows what the
link wants to do and you choose Allow or Cancel. Turn on "Let links control the timer" in Settings (or tick
"Always allow" in the dialog) to skip the question. The setting is per device. A link that is not possible in the
current state (pausing when nothing runs, starting while a session is running) does nothing and says so.

### Callbacks

An acting link (`#/start`, `#/pause`, `#/resume`, `#/break`, `#/finish`) may carry `x-success` and `x-error`, plain
`http://` or `https://` addresses (no user name or password; anything else is ignored). Once the action has been
tried, the app goes to `x-success` if it worked, or to `x-error` with `error=<code>` added if it did not. This is
for other tools that open a link and want to be sent back. Because it sends you to another site, a callback is only
followed after you have seen the confirmation, which names the host it will take you to. With "Let links control
the timer" on, links run without asking and their callbacks are ignored.

Unknown links, and values out of range, are ignored rather than guessed at.

## Keyboard shortcuts (timer screen)

| Key | Action |
|---|---|
| Space or P | Pause or resume |
| S | Start a focus session (idle) |
| + / − | Add or take off 5 minutes |
| F | Finish the focus session |
| B | Take a break |
| / | Go to the intention field |
| Esc | Leave a field |

Shortcuts are off while you are typing in a field, while a dialog is open, and when Ctrl, Cmd or Alt is held.

The keys can be changed in Settings, under Keyboard shortcuts (per device): tap Change next to an action, then press a single letter, number, symbol or the space bar. A key that another action already uses is refused, and "Restore the default keys" undoes every change. Esc always leaves a field and cannot be changed. The table above lists the defaults.
