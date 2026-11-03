<script lang="ts">
  import { angleOf, handleAngle, scrub, snap, turnBetween } from "./dial";
  import { t } from "./i18n";

  interface Props {
    /** Minutes. */
    value: number;
    min?: number;
    max?: number;
    step?: number;
    onchange: (minutes: number) => void;
    disabled?: boolean;
    class?: string;
  }
  let { value, min = 5, max = 180, step = 5, onchange, disabled = false, class: extra = "" }: Props = $props();

  const range = $derived({ min, max, step });
  const R = 100;
  const C = 2 * Math.PI * R;

  // Fraction of the ring that is filled: a full lap at 60 minutes, then it starts a second lap.
  const lap = $derived(((value - 1) % 60 + 1) / 60);
  const secondLap = $derived(value > 60);
  const angle = $derived((handleAngle(value) * Math.PI) / 180);
  const hx = $derived(120 + R * Math.sin(angle));
  const hy = $derived(120 - R * Math.cos(angle));

  let svg: SVGSVGElement | undefined = $state();
  let drag: { prev: number; total: number; start: number } | null = null;

  function pointerAngle(event: PointerEvent): number {
    const box = svg?.getBoundingClientRect();
    if (!box) return 0;
    return angleOf(box.left + box.width / 2, box.top + box.height / 2, event.clientX, event.clientY);
  }

  function down(event: PointerEvent): void {
    if (disabled) return;
    svg?.setPointerCapture(event.pointerId);
    drag = { prev: pointerAngle(event), total: 0, start: value };
  }

  function move(event: PointerEvent): void {
    if (!drag) return;
    const now = pointerAngle(event);
    drag.total += turnBetween(drag.prev, now);
    drag.prev = now;
    const next = scrub(drag.start, drag.total, range);
    if (next !== value) onchange(next);
  }

  function up(): void {
    drag = null;
  }

  function key(event: KeyboardEvent): void {
    if (disabled) return;
    const moves: Record<string, number> = {
      ArrowUp: step,
      ArrowRight: step,
      ArrowDown: -step,
      ArrowLeft: -step,
      PageUp: step * 3,
      PageDown: -step * 3,
    };
    let next: number | undefined;
    if (event.key === "Home") next = min;
    else if (event.key === "End") next = max;
    else if (event.key in moves) next = snap(value + (moves[event.key] as number), range);
    if (next === undefined) return;
    event.preventDefault();
    if (next !== value) onchange(next);
  }
</script>

<!-- A slider for assistive tech and keyboards; pointer drag turns it like a wheel. -->
<svg
  bind:this={svg}
  class="mx-auto block size-64 max-w-full touch-none select-none rounded-full outline-none focus-visible:ring-2 focus-visible:ring-accent {disabled ? 'opacity-60' : ''} {extra}"
  viewBox="0 0 240 240"
  role="slider"
  tabindex="0"
  aria-label={t("timer.dial.label")}
  aria-orientation="horizontal"
  aria-valuemin={min}
  aria-valuemax={max}
  aria-valuenow={value}
  aria-valuetext={t("timer.dial.minutes", { minutes: value })}
  aria-disabled={disabled}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
>
  <circle cx="120" cy="120" r={R} fill="none" class="stroke-surface1" stroke-width="16" />
  {#if secondLap}
    <circle cx="120" cy="120" r={R} fill="none" class="stroke-accent" stroke-width="16" opacity="0.3" />
  {/if}
  <circle
    cx="120"
    cy="120"
    r={R}
    fill="none"
    class="stroke-accent"
    stroke-width="16"
    stroke-linecap="round"
    stroke-dasharray={C}
    stroke-dashoffset={C * (1 - lap)}
    transform="rotate(-90 120 120)"
  />
  <circle cx={hx} cy={hy} r="14" class="fill-accent stroke-base" stroke-width="4" />
  <text x="120" y="128" text-anchor="middle" class="fill-text" font-size="56" font-weight="700">{value}</text>
  <text x="120" y="156" text-anchor="middle" class="fill-subtext0" font-size="16">{t("timer.dial.unit")}</text>
</svg>
