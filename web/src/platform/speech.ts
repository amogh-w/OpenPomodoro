// Browser adapter: spoken announcements with the browser's own speech voices. Off unless the person turns it on.
export const speechSupported = (): boolean => typeof speechSynthesis !== "undefined" && typeof SpeechSynthesisUtterance !== "undefined";

/** Say `text` in `lang` at `volume` (0 to 100). A new announcement replaces one still being spoken. */
export function speak(text: string, lang: string, volume: number): void {
  if (!speechSupported() || text.trim() === "" || volume <= 0) return;
  try {
    speechSynthesis.cancel();
    const utterance = new SpeechSynthesisUtterance(text);
    utterance.lang = lang;
    utterance.volume = Math.min(1, volume / 100);
    speechSynthesis.speak(utterance);
  } catch {
    // Some browsers refuse until the page has been touched; the sound alerts still work.
  }
}
