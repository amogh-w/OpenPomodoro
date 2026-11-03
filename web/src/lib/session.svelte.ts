// Who is signed in, and the first-run / sign-in flow around it.
import { api, ApiError } from "./api";
import {
  discardLocal,
  flushOutbox,
  pendingCount,
  setUnauthorizedHandler,
  start as startLive,
  stop as stopLive,
} from "./appState.svelte";
import { kvDelete, kvGet, kvSet } from "./idb";
import type { User } from "./types";

type Status = "loading" | "setup" | "signed_out" | "signed_in" | "error";

export const session = $state<{ status: Status; user: User | null; registration: "open" | "closed" }>({
  status: "loading",
  user: null,
  registration: "closed",
});

/** `offline` means the server could not be reached and the user comes from what this device remembered. */
function enter(user: User, offline = false): void {
  session.user = user;
  session.status = "signed_in";
  void kvSet("user", user).catch(() => {});
  startLive(user.id, { offline });
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

/** The server no longer accepts our cookie. What was queued stays, in case the same user signs in again. */
async function sessionLost(): Promise<void> {
  stopLive();
  await kvDelete("user").catch(() => {});
  await showSignedOut();
}

setUnauthorizedHandler(() => void sessionLost());

/** Run once at startup. With no connection, a user this device remembers is signed in from what it has saved. */
export async function bootstrap(): Promise<void> {
  session.status = "loading";
  try {
    enter(await api.me());
  } catch (error) {
    if (error instanceof ApiError && error.status === 401) {
      await kvDelete("user").catch(() => {});
      await showSignedOut();
    } else if (error instanceof ApiError && (error.isNetwork || error.status >= 500)) {
      const remembered = await kvGet<User>("user").catch(() => undefined);
      if (remembered) enter(remembered, true);
      else session.status = "error";
    } else {
      session.status = "error";
    }
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

/**
 * Sign out. If things done offline have not reached the server, nothing happens and `pending` comes back, so
 * the screen can ask; `force` signs out anyway and throws those changes away.
 */
export async function signOut(force = false): Promise<"done" | "pending"> {
  const user = session.user;
  if (!force && user) {
    await flushOutbox();
    if (pendingCount() > 0) return "pending";
  }
  await api.signOut().catch(() => {});
  if (user) await discardLocal(user.id);
  stopLive();
  await showSignedOut();
  return "done";
}

/** After the account was deleted on the server. */
export async function accountGone(): Promise<void> {
  const user = session.user;
  if (user) await discardLocal(user.id);
  stopLive();
  await showSignedOut();
}
