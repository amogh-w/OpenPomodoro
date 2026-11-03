// Browser adapter: the app icon badge (installed apps on supported platforms).
type BadgeNavigator = Navigator & {
  setAppBadge?: (count?: number) => Promise<void>;
  clearAppBadge?: () => Promise<void>;
};

/** Show `count` on the app icon, or clear it with `null`. Silently does nothing where unsupported. */
export function setBadge(count: number | null): void {
  const nav = navigator as BadgeNavigator;
  try {
    if (count === null || count <= 0) void nav.clearAppBadge?.().catch(() => {});
    else void nav.setAppBadge?.(count).catch(() => {});
  } catch {
    // Unsupported or blocked.
  }
}
