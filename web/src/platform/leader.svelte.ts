// Browser adapter: with several tabs open, exactly one is the leader. The leader does the jobs that must happen
// once (sending the offline outbox, and later playing sounds and showing notifications); the others only draw.
//
// The Web Locks API does the election: one tab holds an exclusive lock for as long as it lives, and when it
// closes or crashes the browser hands the lock to another tab. Browsers without Web Locks have no way to see
// other tabs, so each tab acts as its own leader (duplicate sends are harmless; duplicate sounds are possible).
export const leader = $state({ isLeader: false });

let started = false;

export function startLeaderElection(): void {
  if (started) return;
  started = true;
  if (typeof navigator === "undefined" || !("locks" in navigator)) {
    leader.isLeader = true;
    return;
  }
  void navigator.locks.request("op-leader", () => {
    leader.isLeader = true;
    return new Promise<never>(() => {}); // hold the lock until this tab goes away
  });
}
