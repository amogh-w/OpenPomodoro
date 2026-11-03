// Browser adapter: plays the alert sounds with Web Audio. Browsers keep audio locked until the person has touched
// the page, so the first tap or key press unlocks it; until then `playAlert` returns false instead of failing.
import { type AlertKind, scoreFor } from "../lib/sounds";

type AudioWindow = Window & { webkitAudioContext?: typeof AudioContext };

let context: AudioContext | null = null;

function ctx(): AudioContext | null {
  if (context) return context;
  const Ctor = window.AudioContext ?? (window as AudioWindow).webkitAudioContext;
  if (!Ctor) return null;
  try {
    context = new Ctor();
  } catch {
    context = null;
  }
  return context;
}

/** Called from a tap or key press: lets audio play from now on. */
export function unlockAudio(): void {
  const c = ctx();
  if (c && c.state === "suspended") void c.resume().catch(() => {});
}

/** Unlock audio on the first interaction anywhere on the page. */
export function installAudioUnlock(): void {
  const events = ["pointerdown", "keydown", "touchstart"] as const;
  const once = (): void => {
    unlockAudio();
    for (const e of events) removeEventListener(e, once);
  };
  for (const e of events) addEventListener(e, once, { passive: true });
}

export const audioSupported = (): boolean => ctx() !== null;

export function audioUnlocked(): boolean {
  return context !== null && context.state === "running";
}

/** Play an alert at `volume` (0 to 100). Returns false if nothing could be played (locked, unsupported, silent). */
export function playAlert(kind: AlertKind, volume: number): boolean {
  const c = ctx();
  if (!c || volume <= 0) return false;
  if (c.state !== "running") {
    void c.resume().catch(() => {});
    return false;
  }
  // Keep even full volume gentle: these tones are meant to be noticed, not to startle.
  const peak = (Math.min(100, volume) / 100) * 0.35;
  const start = c.currentTime + 0.02;
  for (const n of scoreFor(kind)) {
    const osc = c.createOscillator();
    const gain = c.createGain();
    osc.type = "sine";
    osc.frequency.value = n.freq;
    const from = start + n.at;
    gain.gain.setValueAtTime(0.0001, from);
    gain.gain.exponentialRampToValueAtTime(peak, from + 0.015);
    gain.gain.exponentialRampToValueAtTime(0.0001, from + n.length);
    osc.connect(gain).connect(c.destination);
    osc.start(from);
    osc.stop(from + n.length + 0.05);
  }
  return true;
}
