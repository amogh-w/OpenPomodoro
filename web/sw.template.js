// Service worker for the app shell only. The build (vite.config.ts) replaces the two
// placeholders below with the real version and file list. Alerts are NOT scheduled here:
// browsers give service workers no way to fire a notification later (PLAN.md, section 2). A closed app is reached
// by the server through Web Push instead, handled by the "push" listener below.
const VERSION = "__VERSION__";
const PRECACHE = __PRECACHE__;
const CACHE = `op-shell-${VERSION}`;

self.addEventListener("install", (event) => {
  event.waitUntil(caches.open(CACHE).then((cache) => cache.addAll(PRECACHE)));
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches
      .keys()
      .then((keys) => Promise.all(keys.filter((k) => k.startsWith("op-shell-") && k !== CACHE).map((k) => caches.delete(k)))),
  );
});

// The page asks to take over when the user taps "Update now".
self.addEventListener("message", (event) => {
  if (event.data && event.data.type === "SKIP_WAITING") self.skipWaiting();
});

// A message from the server's push service (the app may be closed). It always shows a notification, which is what
// the browser requires of a push subscription.
self.addEventListener("push", (event) => {
  let data = {};
  try {
    data = event.data ? event.data.json() : {};
  } catch {
    data = {};
  }
  const title = typeof data.title === "string" && data.title !== "" ? data.title : "OpenPomodoro";
  event.waitUntil(
    self.registration.showNotification(title, {
      body: typeof data.body === "string" ? data.body : "",
      tag: typeof data.tag === "string" ? data.tag : "op-push",
      icon: "/icon-192.png",
    }),
  );
});

// Clicking a notification brings the open app forward, or opens it.
self.addEventListener("notificationclick", (event) => {
  event.notification.close();
  event.waitUntil(
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((windows) => {
      const open = windows.find((w) => new URL(w.url).origin === self.location.origin);
      return open ? open.focus() : self.clients.openWindow("/");
    }),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  const url = new URL(req.url);
  if (req.method !== "GET" || url.origin !== self.location.origin) return;
  if (url.pathname.startsWith("/api/") || url.pathname === "/ws") return; // live data is never cached

  if (req.mode === "navigate") {
    event.respondWith(caches.match("/index.html").then((hit) => hit || fetch(req)));
    return;
  }
  event.respondWith(caches.match(req).then((hit) => hit || fetch(req)));
});
