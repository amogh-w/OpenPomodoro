// A browser's long identification string, boiled down to something a person recognises: "Chrome on macOS".

export interface Agent {
  browser: string;
  os: string;
}

/** Null when nothing recognisable is in the string (an empty label, a script, an old session). */
export function describeAgent(ua: string): Agent | null {
  if (ua.trim() === "") return null;
  // Order matters: Edge and Opera also say Chrome, and Chrome also says Safari.
  const browser = /Edg(e|A|iOS)?\//.test(ua)
    ? "Edge"
    : /OPR\/|Opera/.test(ua)
      ? "Opera"
      : /Firefox\/|FxiOS\//.test(ua)
        ? "Firefox"
        : /Chrome\/|CriOS\//.test(ua)
          ? "Chrome"
          : /Safari\//.test(ua)
            ? "Safari"
            : null;
  const os = /Windows/.test(ua)
    ? "Windows"
    : /iPhone|iPad|iPod/.test(ua)
      ? "iOS"
      : /Android/.test(ua)
        ? "Android"
        : /Mac OS X|Macintosh/.test(ua)
          ? "macOS"
          : /CrOS/.test(ua)
            ? "ChromeOS"
            : /Linux|X11/.test(ua)
              ? "Linux"
              : null;
  return browser && os ? { browser, os } : null;
}
