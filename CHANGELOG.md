# Changelog

## Unreleased

- **Slimmer side panel and runic dial:** the panel beside the timer now shows just today (total, medallion, sessions);
  the week strip and the day timeline are in History, which also draws calendar-file events. The dial and the running
  ring share the medallion's stone-and-brass look. Mood after a session is picked from three faces.
- **Breaks follow focus:** the manual "set up a break" button is gone from the idle screen; a break is offered when a
  session ends. When the time is up, the choices are labelled (Take a break, Another session, End session). "Paused"
  no longer pushes the clock up, task checkboxes use the kit's checkbox, and every rune glyph is centred on its tablet
  (the corner dots are gone).
- **Simpler tasks and confirmations:** tasks are one list with a checkbox; a ticked task stays in place, crossed out,
  instead of moving to a separate Done section (rename is gone; delete stays). Confirmation dialogs ask a plain question
  with Yes and No.
- **Motion:** the kit's entrance helpers are used: screens fade in, lists rise in one after another, the medallion pops
  when it seals, the mood faces pop when chosen, the time-up buttons rise in, and errors shake once. Reduced motion is
  honoured by the kit.
- **Mood is required:** with "ask how it went" on, choose Focused (green), Neutral (yellow) or Distracted (red) before
  taking a break or finishing; the buttons wait until you do.
- **Categories reworked:** each category has a colour and an icon. On the timer they are a row of chips (most recently
  used first) with one-tap choosing, a quick "New" form and a Manage dialog; a new account is offered the usual set
  (Study, Work, Reading, Exercise, Creative, Admin). History filters by category chips with the month's time on each,
  rows show the category icon and name, and an opened day shows how it split across categories. The session editors
  use the same picker. Categories gained an `icon` field (also in backups).
- **One card for the session:** the idle timer screen now has a single card with the intention field, the category
  chips and the task list, instead of three separate pieces. Choosing a task fills the field and highlights it (tap
  it again to let go); the "Working on" pill is gone. A task stays linked only while the field still shows its title.
  A new task is filed under the category chosen above it, each task shows its category, picking a task takes on its
  category, and letting it go puts back what was chosen before, so the category no longer drifts between tasks.
  The category row above the field is gone: the category is chosen with one selector in the tasks header, which also
  makes a new category, offers the usual set and opens the editor.
  The selector now sits on the add row, between the field and Add, and every task has its own selector to change its
  category after it is made.
  Layout settled: the field, then its category selector, then a plain checklist where each row shows its tag on the
  right (tap it to change), then the add-a-task line at the bottom.
- **Runes and medallions:** a rune lights up during each focus session, and every finished cycle seals a medallion with
  one socket per session; the Profile screen shows the hall of them. An early end lights its rune only as far as you
  got, abandoning costs nothing, and it can be switched off in Settings.

## 0.2.0

The browser app is now feature-complete for what a browser can do. Nothing here needs a desktop or phone wrapper.

- **Interface:** a redesigned timer screen (prompt, category picker, tick-ring dial that drags smoothly and snaps on
  release, one start button), a day panel with an hourly timeline and the session about to start, a Profile screen,
  a small timer bar on the other screens, an icon rail and two panes on wide screens, a compact view, and a
  floating picture-in-picture timer where the browser has one.
- **Timer:** End session (saves it, counts as a round) and Abandon session (saves nothing), each asking first;
  timers as short as 5 seconds for trying the flow; strict mode; presence nudge; special breaks at fixed times;
  optional tick sound; spoken alerts; your own alert sounds.
- **Capture and reports:** tasks linked to sessions (rename, time spent, in exports), category groups and a usual
  length per category, month overview with time by category, group and intention, a 52-week heatmap, a share
  image, a calendar-file overlay on the day view.
- **Accounts and access:** two-step sign-in with recovery codes (and an admin reset), personal API tokens, a list of
  signed-in browsers, a documented token API with a live WebSocket stream ([docs/api.md](docs/api.md)).
- **Integration:** lifecycle hooks (programs) and signed webhooks set in a file by the admin, with Test buttons;
  callback addresses on acting links; closed-app alerts through Web Push.
- **Look and language:** high-contrast, Nord, Gruvbox and Solarized themes, your own JSON theme, a language picker
  (English and Spanish) and a translators' guide, custom keyboard shortcuts.
- **Diagnostics:** a copyable log on each device and a server log for the admin.
- **Changed:** the 30-second automatic discard and the "early endings do not count" rule are gone; a database
  upgrade runs on first start (new tables for sounds, tokens, two-step and push).
- **New dependencies:** `ureq` (webhooks and push, with rustls), `p256`, `aes-gcm`, `sha1`.

Known gaps: no OpenID Connect sign-in, no QR code for two-step setup, no website blocker or desktop shell (parked, see
[PLAN.md](PLAN.md)), the Docker image has never been built, and the newest screens were checked by tests and the
build, not yet by hand in a browser.

## 0.1.0

First release: a self-hosted, multi-user Pomodoro and focus tracker.

- **Server:** one Rust program with the web app built in and one SQLite file. Accounts (the first is the admin,
  sign-up closed by default, recovery by the admin), per-user data, sign-in rate limits, host/origin and CSRF
  guards, security headers, live updates over WebSocket.
- **Timer:** focus, short and long breaks, pause, overflow, strict and auto-start modes, notes and mood on
  reflection. Time is stored as timestamps, so reloads, sleep and a second device stay correct.
- **History:** timeline, daily totals and stats, editing, categories, export (JSON, CSV, ICS), backup and restore.
- **App:** installable and offline-capable; commands made offline are replayed with their true times. Sound and
  notification alerts with quiet hours, links and keyboard shortcuts, four themes, reduced motion, screen reader
  announcements.
- **Hosting:** `scripts/build-release.sh`, a `Dockerfile` and `docker-compose.yml` (written but not yet built in CI),
  and [docs/self-hosting.md](docs/self-hosting.md).

Known gaps: no push notifications while the app is closed, no site blocking, no sync between servers, no desktop
or phone wrappers yet. See [PLAN.md](PLAN.md).
