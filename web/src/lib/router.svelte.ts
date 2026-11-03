import { type Route, hashFor, parseHash } from "./router";

/** The current route, kept in sync with the URL hash. */
export const router = $state<{ route: Route }>({ route: parseHash(location.hash) });

addEventListener("hashchange", () => {
  router.route = parseHash(location.hash);
});

export function navigate(route: Route): void {
  location.hash = hashFor(route);
}
