// Browser adapter: a small always-on-top window for the timer (Document Picture-in-Picture). Only some browsers have
// it (Chromium based ones); everywhere else `pipSupported()` is false and the button is not shown.
interface DocumentPip {
  requestWindow(options?: { width?: number; height?: number }): Promise<Window>;
}

const api = (): DocumentPip | undefined => (window as unknown as { documentPictureInPicture?: DocumentPip }).documentPictureInPicture;

export const pip = $state({ open: false });

export const pipSupported = (): boolean => api() !== undefined;

let card: HTMLElement | null = null;
let home: HTMLElement | null = null;

/** The component that draws the floating timer hands over its element, which is moved into the window and back. */
export function registerPipCard(node: HTMLElement | null): void {
  card = node;
  home = node?.parentElement ?? null;
}

export async function openPip(): Promise<void> {
  const documentPip = api();
  if (!documentPip || !card || pip.open) return;
  const win = await documentPip.requestWindow({ width: 340, height: 190 });
  // The floating window starts empty: give it the app's styles and look (theme class, accent and radius variables).
  for (const sheet of document.querySelectorAll('link[rel="stylesheet"], style')) win.document.head.appendChild(sheet.cloneNode(true));
  win.document.documentElement.className = document.documentElement.className;
  win.document.documentElement.setAttribute("style", document.documentElement.getAttribute("style") ?? "");
  win.document.documentElement.lang = document.documentElement.lang;
  win.document.body.className = document.body.className;
  const moving = card;
  win.document.body.appendChild(moving);
  pip.open = true;
  win.addEventListener("pagehide", () => {
    home?.appendChild(moving);
    pip.open = false;
  });
}
