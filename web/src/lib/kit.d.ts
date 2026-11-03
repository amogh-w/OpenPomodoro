// Globals defined by the cuppa-ui kit's theme.js (a classic script loaded in <head>).
declare function applyTheme(): void;
declare const ACCENTS: readonly string[];
declare const store: {
  get(key: string): string | null;
  set(key: string, value: string): void;
};
