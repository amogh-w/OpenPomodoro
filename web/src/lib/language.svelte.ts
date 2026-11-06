// The language choice for this device ("auto" follows the browser). Stored under the app's `op_` prefix.
import { type LanguageChoice, isChoice, resolveLanguage } from "./language";

const KEY = "op_lang";

function read(): LanguageChoice {
  try {
    const stored = localStorage.getItem(KEY);
    return isChoice(stored) ? stored : "auto";
  } catch {
    return "auto";
  }
}

export const language = $state<{ choice: LanguageChoice }>({ choice: read() });

export function setLanguage(choice: LanguageChoice): void {
  language.choice = choice;
  try {
    localStorage.setItem(KEY, choice);
  } catch {
    // The choice lasts until the page closes.
  }
}

/** The language in use now (reactive: reading it inside a template or effect follows later changes). */
export function activeLanguage(): ReturnType<typeof resolveLanguage> {
  return resolveLanguage(language.choice, typeof navigator === "undefined" ? [] : (navigator.languages ?? [navigator.language]));
}
