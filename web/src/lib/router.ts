export const routes = ["timer", "history", "profile", "settings", "admin"] as const;
export type Route = (typeof routes)[number];

const isRoute = (name: string): name is Route => (routes as readonly string[]).includes(name);

/** `#/history?x=1` -> `history`. Unknown or empty hashes fall back to the timer. */
export function parseHash(hash: string): Route {
  const name = hash.replace(/^#\/?/, "").split(/[/?]/)[0] ?? "";
  if (name === "analytics") return "history"; // the link-friendly name for the history screen
  return isRoute(name) ? name : "timer";
}

export function hashFor(route: Route): string {
  return `#/${route}`;
}
