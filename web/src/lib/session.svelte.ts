// Who is signed in, and the first-run / sign-in flow around it.
import { api, ApiError } from "./api";
import { setUnauthorizedHandler, start as startLive, stop as stopLive } from "./appState.svelte";
import type { User } from "./types";

type Status = "loading" | "setup" | "signed_out" | "signed_in" | "error";

export const session = $state<{ status: Status; user: User | null; registration: "open" | "closed" }>({
  status: "loading",
  user: null,
  registration: "closed",
});

function enter(user: User): void {
  session.user = user;
  session.status = "signed_in";
  startLive();
}

/** Work out which screen to show when nobody is signed in: first-run setup or the sign-in form. */
async function showSignedOut(): Promise<void> {
  session.user = null;
  try {
    const setup = await api.setupStatus();
    session.registration = setup.registration;
    session.status = setup.needs_setup ? "setup" : "signed_out";
  } catch {
    session.status = "error";
  }
}

/** The server no longer accepts our cookie. */
async function sessionLost(): Promise<void> {
  stopLive();
  await showSignedOut();
}

setUnauthorizedHandler(() => void sessionLost());

/** Run once at startup. */
export async function bootstrap(): Promise<void> {
  session.status = "loading";
  try {
    enter(await api.me());
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) await showSignedOut();
    else session.status = "error";
  }
}

export async function signIn(username: string, password: string): Promise<void> {
  enter(await api.signIn(username, password));
}

export async function createFirstAccount(username: string, password: string): Promise<void> {
  enter(await api.setup(username, password));
}

export async function register(username: string, password: string): Promise<void> {
  enter(await api.register(username, password));
}

export async function signOut(): Promise<void> {
  await api.signOut().catch(() => {});
  stopLive();
  await showSignedOut();
}

/** After the account was deleted on the server. */
export async function accountGone(): Promise<void> {
  stopLive();
  await showSignedOut();
}
