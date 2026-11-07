<script lang="ts">
  import { onDestroy, untrack } from "svelte";
  import { angleOf, handleAngle, scrubRaw, snap, turnBetween } from "./dial";
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
  }
  let { value, min = 5, max = 180, step = 5, onchange, disabled = false, class: extra = "", tone = "accent", unit = "min" }: Props = $props();

  const range = $derived({ min, max, step });

  // What is drawn. While dragging it follows the pointer exactly (smooth); the number you see and the value that is
  // kept still snap to the step. Letting go, or any other change (keys, a typed "30m"), glides to the snapped value.
  let display = $state(untrack(() => value));
  let dragging = false;
  let frame = 0;

  const reduced = (): boolean =>
    document.documentElement.classList.contains("reduce-motion") || matchMedia("(prefers-reduced-motion: reduce)").matches;

  function glideTo(target: number): void {
    cancelAnimationFrame(frame);
    const from = display;
    if (reduced() || Math.abs(target - from) < 0.01) {
      display = target;
      return;
    }
    const began = performance.now();
    const length = Math.min(260, 90 + Math.abs(target - from) * 5);
    const tickFrame = (now: number): void => {
      const t = Math.min(1, (now - began) / length);
      display = from + (target - from) * (1 - (1 - t) ** 3);
      if (t < 1) frame = requestAnimationFrame(tickFrame);
    };
    frame = requestAnimationFrame(tickFrame);
  }

  $effect(() => {
    const target = value;
    if (!dragging) untrack(() => glideTo(target));
  });
  onDestroy(() => cancelAnimationFrame(frame));
  // Geometry (viewBox 240): a ring of 60 tick marks, a pie that fills as the length grows, and a hand to turn.
  const WEDGE = 68;
  const HAND = 80;
  const ticks = Array.from({ length: 60 }, (_, i) => {
    const a = (i * 6 * Math.PI) / 180;
    return { i, major: i % 5 === 0, sin: Math.sin(a), cos: Math.cos(a) };
  });
  const at = (tick: { sin: number; cos: number }, r: number) => ({ x: 120 + r * tick.sin, y: 120 - r * tick.cos });
  // Gradients need an id that is unique on the page.
  const uid = `dial-${Math.random().toString(36).slice(2, 9)}`;

  // Fraction of the circle that is filled: a full lap at 60 minutes, then it starts a second lap.
  const lap = $derived(((display - 1) % 60 + 1) / 60);
  const secondLap = $derived(display > 60);
  const angle = $derived((handleAngle(display) * Math.PI) / 180);
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
    cancelAnimationFrame(frame);
    dragging = true;
    drag = { prev: pointerAngle(event), total: 0, start: display };
  }

  function move(event: PointerEvent): void {
    if (!drag) return;
    const now = pointerAngle(event);
    drag.total += turnBetween(drag.prev, now);
    drag.prev = now;
    const raw = scrubRaw(drag.start, drag.total, range);
    display = raw;
    const next = snap(raw, range);
    if (next !== value) onchange(next);
  }

  function up(): void {
    if (!drag) return;
    drag = null;
    dragging = false;
    glideTo(value);
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
  class="mx-auto block size-[min(16rem,max(11rem,40vh))] max-w-full touch-none select-none rounded-full outline-none focus-visible:ring-2 focus-visible:ring-accent {disabled ? 'opacity-60' : ''} {extra}"
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
  <defs>
    <linearGradient id="{uid}-metal" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-yellow) 70%, white)" />
      <stop offset="0.5" style="stop-color: var(--ctp-yellow)" />
      <stop offset="1" style="stop-color: color-mix(in oklab, var(--ctp-peach) 70%, var(--ctp-crust))" />
    </linearGradient>
    <radialGradient id="{uid}-face" cx="0.38" cy="0.32" r="0.9">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-surface1) 85%, transparent)" />
      <stop offset="1" style="stop-color: color-mix(in oklab, var(--ctp-surface0) 80%, var(--ctp-crust))" />
    </radialGradient>
  </defs>
  <!-- the stone bezel: a shadow under it, a rim, and a pale edge inside -->
  <circle cx="120" cy="123" r="117" fill="black" opacity="0.28" />
  <circle cx="120" cy="120" r="117" class="fill-surface1" />
  <circle cx="120" cy="120" r="116" fill="none" class="stroke-overlay0" stroke-width="2" />
  <circle cx="120" cy="120" r="111" fill="none" stroke="white" stroke-opacity="0.12" stroke-width="1" />
  <!-- carved minute marks, and a brass stud at every fifth -->
  {#each ticks as tick (tick.i)}
    {#if tick.major}
      <circle cx={at(tick, 103).x} cy={at(tick, 103).y + 0.8} r={tick.i === 0 ? 4.4 : 3.2} fill="black" opacity="0.35" />
      <circle cx={at(tick, 103).x} cy={at(tick, 103).y} r={tick.i === 0 ? 4.4 : 3.2} fill="url(#{uid}-metal)" />
    {:else}
      <line x1={at(tick, 101).x} y1={at(tick, 101).y + 0.8} x2={at(tick, 106).x} y2={at(tick, 106).y + 0.8} stroke="black" stroke-opacity="0.4" stroke-width="1.6" stroke-linecap="round" />
      <line x1={at(tick, 101).x} y1={at(tick, 101).y} x2={at(tick, 106).x} y2={at(tick, 106).y} class="stroke-overlay1" stroke-width="1.4" stroke-linecap="round" />
    {/if}
  {/each}
  <circle cx="120" cy="120" r="90" fill="url(#{uid}-face)" />
  <circle cx="120" cy="120" r="90" fill="none" class="stroke-surface2" stroke-width="1.4" />
  <circle cx="120" cy="120" r={WEDGE + 4} fill="none" class="stroke-surface2" stroke-width="1" />
  <circle cx="120" cy="120" r={WEDGE} class="fill-base" opacity="0.55" />
  {#if secondLap}<circle cx="120" cy="120" r={WEDGE} class={fillClass} opacity="0.35" />{/if}
  {#if wedge === ""}
    <circle cx="120" cy="120" r={WEDGE} class={fillClass} opacity="0.85" />
  {:else}
    <path d={wedge} class={fillClass} opacity="0.85" />
  {/if}
  <line x1="120" y1="120" x2={tipX} y2={tipY} class={strokeClass} stroke-width="3.5" stroke-linecap="round" />
  <circle cx={tipX} cy={tipY + 0.8} r="6" fill="black" opacity="0.35" />
  <circle cx={tipX} cy={tipY} r="6" fill="url(#{uid}-metal)" />
  <circle cx="120" cy="120" r="10" fill="url(#{uid}-metal)" />
  <circle cx="120" cy="120" r="3" class="fill-crust" opacity="0.6" />
</svg>
