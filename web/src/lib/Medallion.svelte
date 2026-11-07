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
  // One socket per session, evenly round the ring starting at the top.
  const sockets = $derived(
    Array.from({ length: count }, (_, i) => {
      const angle = (i / count) * 2 * Math.PI - Math.PI / 2;
      return { i, x: 32 + 23 * Math.cos(angle), y: 32 + 23 * Math.sin(angle), filled: slots[i] };
    }),
  );
  const size = $derived(count <= 4 ? 0.42 : count <= 8 ? 0.3 : 0.22);
</script>

<svg class="block {extra}" viewBox="0 0 64 64" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <!-- the medal: a dim ring while it is being made, bright metal once sealed -->
  <circle cx="32" cy="32" r="29" class={sealed ? "stroke-yellow" : "stroke-surface2"} fill="none" stroke-width="2.4" />
  <circle
    cx="32"
    cy="32"
    r="27.5"
    style={sealed ? "fill: color-mix(in oklab, var(--ctp-yellow) 22%, var(--ctp-surface0))" : ""}
    class={sealed ? "" : "fill-surface0"}
  />
  <circle cx="32" cy="32" r="16" fill="none" class={sealed ? "stroke-yellow" : "stroke-surface1"} stroke-width="1" stroke-opacity={sealed ? 0.7 : 1} />
  {#if sealed}
    <!-- the emblem: an eight-pointed star, struck into the centre -->
    <path d="M32 21 L34.6 29.4 L43 32 L34.6 34.6 L32 43 L29.4 34.6 L21 32 L29.4 29.4 Z" class="fill-yellow" />
    <circle cx="32" cy="32" r="2.2" class="fill-surface0" />
  {/if}
  {#each sockets as s (s.i)}
    {#if s.filled}
      <g transform="translate({s.x - 16 * size} {s.y - 20 * size}) scale({size})">
        <rect x="-2" y="-2" width="36" height="44" rx="9" class="fill-surface0" />
        <path d={glyphPath(s.filled.glyph)} fill="none" class="stroke-overlay0" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />
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
      <circle cx={s.x} cy={s.y} r="3.2" class="fill-surface1" />
    {/if}
  {/each}
</svg>
