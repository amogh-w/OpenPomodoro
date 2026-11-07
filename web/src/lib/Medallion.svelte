<script lang="ts">
  import { glyphPath } from "./glyphs";
  import type { Slot } from "./runes";

  interface Props {
    /** The runes carved so far, in order. */
    slots: Slot[];
    /** How many it takes to complete the ring. */
    total: number;
    class?: string;
    label?: string;
  }
  let { slots, total, class: extra = "", label }: Props = $props();

  // Gradients need an id that is unique on the page, however many medallions are drawn.
  const uid = `med-${Math.random().toString(36).slice(2, 9)}`;
  const count = $derived(Math.max(1, Math.min(total, 12)));
  const sealed = $derived(slots.length >= count);
  const RADIUS = 18.2;
  // One socket per session, evenly round the face starting at the top.
  const sockets = $derived(
    Array.from({ length: count }, (_, i) => {
      const angle = (i / count) * 2 * Math.PI - Math.PI / 2;
      return { i, angle, x: 32 + RADIUS * Math.cos(angle), y: 32 + RADIUS * Math.sin(angle), filled: slots[i] };
    }),
  );
  const size = $derived(count <= 4 ? 0.36 : count <= 8 ? 0.27 : 0.2);
  // Fine marks round the rim, like the milling on a coin.
  const ticks = Array.from({ length: 48 }, (_, i) => {
    const a = (i / 48) * 2 * Math.PI;
    const long = i % 4 === 0;
    const r1 = long ? 26.1 : 26.9;
    return { i, x1: 32 + r1 * Math.cos(a), y1: 32 + r1 * Math.sin(a), x2: 32 + 28.1 * Math.cos(a), y2: 32 + 28.1 * Math.sin(a), long };
  });
</script>

<svg class="block {extra}" viewBox="0 0 64 64" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <defs>
    <!-- bright metal for a sealed medallion, from the theme's own yellow and peach -->
    <linearGradient id="{uid}-metal" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-yellow) 70%, white)" />
      <stop offset="0.5" style="stop-color: var(--ctp-yellow)" />
      <stop offset="1" style="stop-color: color-mix(in oklab, var(--ctp-peach) 70%, var(--ctp-crust))" />
    </linearGradient>
    <radialGradient id="{uid}-face" cx="0.38" cy="0.32" r="0.9">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-surface1) 85%, var(--ctp-yellow) {sealed ? 22 : 0}%)" />
      <stop offset="1" style="stop-color: color-mix(in oklab, var(--ctp-surface0) 80%, var(--ctp-crust))" />
    </radialGradient>
  </defs>

  <!-- the rim: a raised band with a bevel, dim until the medallion is sealed -->
  <circle cx="32" cy="32" r="31" fill="none" class="stroke-crust" stroke-opacity="0.55" stroke-width="1.4" />
  <circle cx="32" cy="32" r="29.2" fill="none" stroke={sealed ? `url(#${uid}-metal)` : "var(--ctp-surface2)"} stroke-width="3.4" />
  <circle cx="32" cy="32" r="30.7" fill="none" stroke={sealed ? "white" : "var(--ctp-overlay0)"} stroke-opacity="0.28" stroke-width="0.6" />
  {#each ticks as tick (tick.i)}
    <line x1={tick.x1} y1={tick.y1} x2={tick.x2} y2={tick.y2} stroke={sealed ? "var(--ctp-yellow)" : "var(--ctp-overlay0)"} stroke-opacity={tick.long ? 0.7 : 0.4} stroke-width={tick.long ? 0.9 : 0.6} stroke-linecap="round" />
  {/each}

  <!-- the face, with a ring inside it and a dotted ring round the middle -->
  <circle cx="32" cy="32" r="25" fill="url(#{uid}-face)" />
  <circle cx="32" cy="32" r="24.2" fill="none" stroke={sealed ? "var(--ctp-yellow)" : "var(--ctp-surface2)"} stroke-opacity={sealed ? 0.55 : 0.8} stroke-width="0.8" />
  <circle cx="32" cy="32" r="9.6" fill="none" stroke={sealed ? "var(--ctp-yellow)" : "var(--ctp-surface2)"} stroke-opacity={sealed ? 0.6 : 0.8} stroke-width="0.7" stroke-dasharray="1.1 1.9" />

  {#if sealed}
    <!-- the emblem struck into the centre: a star in a ring -->
    <circle cx="32" cy="32" r="6.4" fill="none" stroke="var(--ctp-yellow)" stroke-width="0.9" />
    <path d="M32 24.6 L33.7 30.3 L39.4 32 L33.7 33.7 L32 39.4 L30.3 33.7 L24.6 32 L30.3 30.3 Z" fill="url(#{uid}-metal)" />
    <circle cx="32" cy="32" r="1.5" style="fill: var(--ctp-surface0)" />
  {:else}
    <circle cx="32" cy="32" r="1.8" class="fill-surface2" />
  {/if}

  {#each sockets as s (s.i)}
    {#if s.filled}
      <g transform="translate({s.x - 16 * size} {s.y - 20 * size}) scale({size})">
        <!-- a stone tile, with a soft shadow, a bevelled edge and a small notch at each end -->
        <rect x="1.2" y="2.6" width="32" height="40" rx="8" fill="black" opacity="0.32" />
        <rect x="0" y="0" width="32" height="40" rx="8" class="fill-surface1" />
        <rect x="1.6" y="1.6" width="28.8" height="36.8" rx="6.6" fill="none" stroke="white" stroke-opacity="0.14" stroke-width="1" />
        <rect x="0" y="0" width="32" height="40" rx="8" fill="none" style={sealed ? "stroke: var(--ctp-yellow); stroke-opacity: 0.7" : "stroke: var(--ctp-overlay0); stroke-opacity: 0.8"} stroke-width="1.6" />
        <circle cx="16" cy="3.8" r="1" class="fill-overlay1" />
        <circle cx="16" cy="36.2" r="1" class="fill-overlay1" />
        <!-- the carving: a dark cut, then the lit line along it -->
        <path d={glyphPath(s.filled.glyph)} transform="translate(0.9 1.1)" fill="none" stroke="black" stroke-opacity="0.45" stroke-width="3.6" stroke-linecap="round" stroke-linejoin="round" />
        <path d={glyphPath(s.filled.glyph)} fill="none" class="stroke-overlay0" stroke-width="3.2" stroke-linecap="round" stroke-linejoin="round" />
        <path
          d={glyphPath(s.filled.glyph)}
          fill="none"
          style="stroke: {s.filled.glow ?? 'var(--accent)'}"
          stroke-opacity={0.3 + 0.7 * Math.min(1, Math.max(0, s.filled.lit))}
          stroke-width="3.2"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </g>
    {:else}
      <!-- an empty socket: a shallow dish waiting for its rune -->
      <circle cx={s.x} cy={s.y} r="3.6" fill="black" opacity="0.28" />
      <circle cx={s.x} cy={s.y} r="3.2" class="fill-surface1" />
      <circle cx={s.x} cy={s.y} r="3.2" fill="none" stroke="var(--ctp-overlay0)" stroke-opacity="0.55" stroke-width="0.7" />
    {/if}
  {/each}
</svg>
