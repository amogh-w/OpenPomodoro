// The person's own alert sounds: kept on the server (so they follow the account and its backups), cached on this
// device for offline use, and decoded for the audio adapter.
import { ApiError, api, type SoundInfo } from "../lib/api";
import { kvGet, kvSet } from "../lib/idb";
import { ALERT_KINDS, type AlertKind } from "../lib/sounds";
import { decodeSound, setCustomSound } from "./audio";

interface Cached {
  updated_at: number;
  data: ArrayBuffer;
}

const cacheKey = (userId: string): string => `sounds:${userId}`;

/** Which alert slots have an uploaded sound (for the settings screen). */
export const customSounds = $state<{ slots: Partial<Record<AlertKind, SoundInfo>> }>({ slots: {} });

const isKind = (slot: string): slot is AlertKind => (ALERT_KINDS as string[]).includes(slot);

async function readCache(userId: string): Promise<Record<string, Cached>> {
  return (await kvGet<Record<string, Cached>>(cacheKey(userId)).catch(() => undefined)) ?? {};
}

async function apply(slot: AlertKind, data: ArrayBuffer | null): Promise<boolean> {
  const buffer = data ? await decodeSound(data) : null;
  setCustomSound(slot, buffer);
  return data === null || buffer !== null;
}

/** Forget every uploaded sound (signing out). */
export function clearCustomSounds(): void {
  for (const kind of ALERT_KINDS) setCustomSound(kind, null);
  customSounds.slots = {};
}

/**
 * Bring this device up to date with the server's list: download what is new or changed, drop what was removed.
 * Without a connection it uses what was cached on this device.
 */
export async function loadCustomSounds(userId: string): Promise<void> {
  const cache = await readCache(userId);
  let listed: SoundInfo[];
  try {
    listed = (await api.listSounds()).sounds;
  } catch (error) {
    if (!(error instanceof ApiError && error.isNetwork)) return;
    const slots: typeof customSounds.slots = {};
    for (const [slot, entry] of Object.entries(cache)) {
      if (isKind(slot) && (await apply(slot, entry.data))) slots[slot] = { slot, mime: "", size: entry.data.byteLength, updated_at: entry.updated_at };
    }
    customSounds.slots = slots;
    return;
  }

  const next: Record<string, Cached> = {};
  const slots: typeof customSounds.slots = {};
  for (const info of listed) {
    if (!isKind(info.slot)) continue;
    const cached = cache[info.slot];
    let data = cached && cached.updated_at === info.updated_at ? cached.data : null;
    if (!data) data = await api.downloadSound(info.slot).catch(() => null);
    if (!data) continue;
    if (await apply(info.slot, data)) {
      next[info.slot] = { updated_at: info.updated_at, data };
      slots[info.slot] = info;
    }
  }
  for (const kind of ALERT_KINDS) if (!slots[kind]) setCustomSound(kind, null);
  customSounds.slots = slots;
  await kvSet(cacheKey(userId), next).catch(() => {});
}

/** Upload a file for a slot. Throws an `ApiError` (`not_audio`, `sound_too_large`) or `Error("undecodable")`. */
export async function uploadCustomSound(userId: string, slot: AlertKind, file: File): Promise<void> {
  const data = await file.arrayBuffer();
  if ((await decodeSound(data)) === null) throw new Error("undecodable");
  await api.uploadSound(slot, new Blob([data]));
  await loadCustomSounds(userId);
}

export async function removeCustomSound(userId: string, slot: AlertKind): Promise<void> {
  await api.removeSound(slot);
  await loadCustomSounds(userId);
}
