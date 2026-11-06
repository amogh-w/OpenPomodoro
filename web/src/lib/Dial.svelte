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
    /** Ring colour: a focus session is the accent, a break is green. */
    tone?: "accent" | "green";
    /** What `value` counts: minutes (a turn is 60 of them) or seconds (a turn is a minute). */
    unit?: "min" | "sec";
    /** Called when the hand is turned back past the start (counter-clockwise), to switch between focus and break. */
    onflip?: () => void;
  }
  let { value, min = 5, max = 180, step = 5, onchange, disabled = false, class: extra = "", tone = "accent", unit = "min", onflip }: Props = $props();

  const range = $derived({ min, max, step });
  // Geometry (viewBox 240): a ring of 60 tick marks, a pie that fills as the length grows, and a hand to turn.
  const WEDGE = 68;
  const HAND = 94;
  const ticks = Array.from({ length: 60 }, (_, i) => {
    const a = (i * 6 * Math.PI) / 180;
    const major = i % 5 === 0;
    const from = major ? 100 : 106;
    return { i, major, x1: 120 + from * Math.sin(a), y1: 120 - from * Math.cos(a), x2: 120 + 114 * Math.sin(a), y2: 120 - 114 * Math.cos(a) };
  });

  // Fraction of the circle that is filled: a full lap at 60 minutes, then it starts a second lap.
  const lap = $derived(((value - 1) % 60 + 1) / 60);
  const secondLap = $derived(value > 60);
  const angle = $derived((handleAngle(value) * Math.PI) / 180);
  const tipX = $derived(120 + HAND * Math.sin(angle));
  const tipY = $derived(120 - HAND * Math.cos(angle));
  const edgeX = $derived(120 + WEDGE * Math.sin(angle));
  const edgeY = $derived(120 - WEDGE * Math.cos(angle));
  const wedge = $derived(
    lap >= 0.9999
      ? ""
      : `M 120 120 L 120 ${120 - WEDGE} A ${WEDGE} ${WEDGE} 0 ${lap > 0.5 ? 1 : 0} 1 ${edgeX} ${edgeY} Z`,
  );
  const fillClass = $derived(tone === "green" ? "fill-green" : "fill-accent");
  const strokeClass = $derived(tone === "green" ? "stroke-green" : "stroke-accent");

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
    if (onflip && drag.start + (drag.total / 360) * 60 < min - step) {
      drag = null;
      onflip();
      return;
    }
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
  aria-valuetext={unit === "sec" ? t("timer.dial.seconds", { seconds: value }) : t("timer.dial.minutes", { minutes: value })}
  aria-disabled={disabled}
  onpointerdown={down}
  onpointermove={move}
  onpointerup={up}
  onpointercancel={up}
  onkeydown={key}
>
  {#each ticks as tickMark (tickMark.i)}
    <line
      x1={tickMark.x1}
      y1={tickMark.y1}
      x2={tickMark.x2}
      y2={tickMark.y2}
      class={tickMark.major ? "stroke-subtext0" : "stroke-overlay0"}
      stroke-width={tickMark.major ? 2.4 : 1.2}
      stroke-linecap="round"
    />
  {/each}
  <circle cx="120" cy="120" r={WEDGE} class="fill-surface0" />
  {#if secondLap}<circle cx="120" cy="120" r={WEDGE} class={fillClass} opacity="0.35" />{/if}
  {#if wedge === ""}
    <circle cx="120" cy="120" r={WEDGE} class={fillClass} opacity="0.85" />
  {:else}
    <path d={wedge} class={fillClass} opacity="0.85" />
  {/if}
  <line x1="120" y1="120" x2={tipX} y2={tipY} class={strokeClass} stroke-width="3.5" stroke-linecap="round" />
  <circle cx="120" cy="120" r="10" class="fill-text" opacity="0.9" />
</svg>
