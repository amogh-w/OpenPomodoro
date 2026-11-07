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

  const count = $derived(Math.max(1, Math.min(total, 12)));
  const sealed = $derived(slots.length >= count);
  // One socket per session, evenly round the inside of the ring starting at the top.
  const RADIUS = 20;
  const sockets = $derived(
    Array.from({ length: count }, (_, i) => {
      const angle = (i / count) * 2 * Math.PI - Math.PI / 2;
      return { i, x: 32 + RADIUS * Math.cos(angle), y: 32 + RADIUS * Math.sin(angle), filled: slots[i] };
    }),
  );
  const size = $derived(count <= 4 ? 0.34 : count <= 8 ? 0.25 : 0.18);
</script>

<svg class="block {extra}" viewBox="0 0 64 64" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <!-- the medal: a dim ring while it is being made, bright metal once sealed -->
  <circle cx="32" cy="32" r="30" class={sealed ? "stroke-yellow" : "stroke-surface2"} fill="none" stroke-width="2.4" />
  <circle
    cx="32"
    cy="32"
    r="28.6"
    style={sealed ? "fill: color-mix(in oklab, var(--ctp-yellow) 18%, var(--ctp-surface0))" : ""}
    class={sealed ? "" : "fill-surface0"}
  />
  {#if sealed}
    <!-- the emblem struck into the centre: a small eight-pointed star -->
    <path d="M32 24 L33.9 30.1 L40 32 L33.9 33.9 L32 40 L30.1 33.9 L24 32 L30.1 30.1 Z" class="fill-yellow" />
  {:else}
    <circle cx="32" cy="32" r="2.2" class="fill-surface1" />
  {/if}
  {#each sockets as s (s.i)}
    {#if s.filled}
      <!-- a carved rune on its own little tile, centred on its socket -->
      <g transform="translate({s.x - 16 * size} {s.y - 20 * size}) scale({size})">
        <rect
          x="0"
          y="0"
          width="32"
          height="40"
          rx="8"
          class="fill-surface1"
          style={sealed ? "stroke: var(--ctp-yellow); stroke-opacity: 0.55" : "stroke: var(--ctp-surface2)"}
          stroke-width="1.6"
        />
        <path d={glyphPath(s.filled.glyph)} fill="none" class="stroke-overlay0" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round" />
        <path
          d={glyphPath(s.filled.glyph)}
          fill="none"
          style="stroke: {s.filled.glow ?? 'var(--accent)'}"
          stroke-opacity={0.35 + 0.65 * Math.min(1, Math.max(0, s.filled.lit))}
          stroke-width="3.4"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </g>
    {:else}
      <circle cx={s.x} cy={s.y} r="3.4" class="fill-surface1" />
    {/if}
  {/each}
</svg>
