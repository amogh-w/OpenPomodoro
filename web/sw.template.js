// Service worker for the app shell only. The build (vite.config.ts) replaces the two
// placeholders below with the real version and file list. Alerts are NOT scheduled here:
// browsers give service workers no way to fire a notification later (PLAN.md, section 2).
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
