// A picture of the month overview to keep or share, drawn on a canvas in the browser from the theme's own colours.
// Nothing leaves the device unless the person shares it.

export interface Bar {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Where each day's bar goes in an area, tallest day filling the height; empty days keep a thin stub. */
export function barLayout(values: readonly number[], area: { x: number; y: number; width: number; height: number }, gap = 6): Bar[] {
  if (values.length === 0) return [];
  const peak = Math.max(1, ...values);
  const width = Math.max(1, (area.width - gap * (values.length - 1)) / values.length);
  return values.map((v, i) => {
    const height = v <= 0 ? 3 : Math.max(6, (v / peak) * area.height);
    return { x: area.x + i * (width + gap), y: area.y + area.height - height, width, height };
  });
}

export interface ShareData {
  title: string;
  subtitle: string;
  /** Big figures: label and value. */
  figures: { label: string; value: string }[];
  /** Focus per day, in order. */
  days: number[];
  footer: string;
}

export const SHARE_SIZE = { width: 1080, height: 1080 } as const;

function color(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement).getPropertyValue(name).trim();
  return value === "" ? fallback : value;
}

/** Draw the picture and return it as a PNG. */
export function renderShareImage(data: ShareData): Promise<Blob | null> {
  const canvas = document.createElement("canvas");
  canvas.width = SHARE_SIZE.width;
  canvas.height = SHARE_SIZE.height;
  const ctx = canvas.getContext("2d");
  if (!ctx) return Promise.resolve(null);
  const accent = color("--accent", "#8aadf4");
  const base = color("--ctp-base", "#1e1e2e");
  const text = color("--ctp-text", "#cdd6f4");
  const sub = color("--ctp-subtext0", "#a6adc8");
  const track = color("--ctp-surface0", "#313244");
  const font = 'system-ui, -apple-system, "Segoe UI", sans-serif';

  ctx.fillStyle = base;
  ctx.fillRect(0, 0, canvas.width, canvas.height);

  ctx.fillStyle = text;
  ctx.font = `700 64px ${font}`;
  ctx.fillText(data.title, 80, 140);
  ctx.fillStyle = sub;
  ctx.font = `400 34px ${font}`;
  ctx.fillText(data.subtitle, 80, 196);

  const columns = Math.max(1, data.figures.length);
  const colWidth = (canvas.width - 160) / columns;
  data.figures.forEach((f, i) => {
    const x = 80 + i * colWidth;
    ctx.fillStyle = text;
    ctx.font = `700 76px ${font}`;
    ctx.fillText(f.value, x, 360);
    ctx.fillStyle = sub;
    ctx.font = `400 30px ${font}`;
    ctx.fillText(f.label, x, 408);
  });

  const area = { x: 80, y: 500, width: canvas.width - 160, height: 380 };
  ctx.fillStyle = track;
  ctx.fillRect(area.x, area.y + area.height + 8, area.width, 4);
  ctx.fillStyle = accent;
  for (const bar of barLayout(data.days, area)) {
    ctx.beginPath();
    ctx.roundRect(bar.x, bar.y, bar.width, bar.height, Math.min(8, bar.width / 2));
    ctx.fill();
  }

  ctx.fillStyle = sub;
  ctx.font = `400 28px ${font}`;
  ctx.fillText(data.footer, 80, canvas.height - 70);

  return new Promise((resolve) => canvas.toBlob(resolve, "image/png"));
}
