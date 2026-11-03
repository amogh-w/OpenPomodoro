import { type LinkAction, changesTimer, hashFromProtocolLink, parseLink } from "./links";
import { type Route, hashFor, parseHash } from "./router";

/** A visit from the browser's `web+openpomodoro:` handler arrives as `/?link=...`; turn it into the matching hash. */
function adoptProtocolLink(): void {
  const link = new URLSearchParams(location.search).get("link");
  if (link === null) return;
  const hash = hashFromProtocolLink(link);
  history.replaceState(null, "", `${location.pathname}${hash ?? ""}`);
}

adoptProtocolLink();

/** The current route, kept in sync with the URL hash. */
export const router = $state<{ route: Route }>({ route: parseHash(location.hash) });

/** A link that wants to change the timer, waiting to be confirmed or carried out (see LinkHandler). */
export const links = $state<{ pending: LinkAction | null }>({ pending: null });

/**
 * A link that changes the timer is taken out of the address bar at once (so a reload cannot repeat it) and held
 * as pending; it is never carried out here.
 */
function takeLink(): void {
  const action = parseLink(location.hash);
  if (action === null || !changesTimer(action)) return;
  links.pending = action;
  history.replaceState(null, "", `${location.pathname}${hashFor("timer")}`);
}

takeLink();
router.route = parseHash(location.hash);

addEventListener("hashchange", () => {
  takeLink();
  router.route = parseHash(location.hash);
});

export function navigate(route: Route): void {
  location.hash = hashFor(route);
}
