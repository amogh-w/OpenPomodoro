// What the timer screen is about to start (its planned length), so the day view beside it can show where the
// session would land. Set only while the timer is idle on a focus session; null otherwise.
export const planPreview = $state<{ plannedMs: number | null }>({ plannedMs: null });
