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
  // The tiles sit on a ring well inside the face, so there is clear space between them, the rim and the middle.
  const radius = $derived(count <= 4 ? 16.4 : count <= 8 ? 17.2 : 18);
  const size = $derived(count <= 4 ? 0.3 : count <= 8 ? 0.23 : 0.17);
  const sockets = $derived(
    Array.from({ length: count }, (_, i) => {
      const angle = (i / count) * 2 * Math.PI - Math.PI / 2;
      return { i, angle, x: 32 + radius * Math.cos(angle), y: 32 + radius * Math.sin(angle), filled: slots[i] };
    }),
  );
  // Brass studs between the tiles round the stone ring.
  const studs = $derived(
    Array.from({ length: count }, (_, i) => {
      const angle = ((i + 0.5) / count) * 2 * Math.PI - Math.PI / 2;
      return { i, x: 32 + 27.2 * Math.cos(angle), y: 32 + 27.2 * Math.sin(angle) };
    }),
  );
</script>

<!-- The picture is drawn a little smaller than its box, so shadows and edges are never cut off and it has room round it. -->
<svg class="block {extra}" viewBox="-3 -3 70 70" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <defs>
    <linearGradient id="{uid}-metal" x1="0" y1="0" x2="1" y2="1">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-yellow) 70%, white)" />
      <stop offset="0.5" style="stop-color: var(--ctp-yellow)" />
      <stop offset="1" style="stop-color: color-mix(in oklab, var(--ctp-peach) 70%, var(--ctp-crust))" />
    </linearGradient>
    <radialGradient id="{uid}-face" cx="0.38" cy="0.32" r="0.9">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-surface1) 85%, var(--ctp-yellow) {sealed ? 20 : 0}%)" />
      <stop offset="1" style="stop-color: color-mix(in oklab, var(--ctp-surface0) 80%, var(--ctp-crust))" />
    </radialGradient>
  </defs>

  <!-- the stone ring, with a shadow under it and a rim that turns gold once sealed -->
  <circle cx="32" cy="33" r="30.6" fill="black" opacity="0.28" />
  <circle cx="32" cy="32" r="30.6" class="fill-surface1" />
  <circle cx="32" cy="32" r="30" fill="none" stroke={sealed ? `url(#${uid}-metal)` : "var(--ctp-overlay0)"} stroke-width="1.8" />
  <circle cx="32" cy="32" r="28.6" fill="none" stroke="white" stroke-opacity="0.12" stroke-width="0.7" />
  {#each studs as stud (stud.i)}
    <circle cx={stud.x} cy={stud.y} r="1.25" fill={sealed ? `url(#${uid}-metal)` : "var(--ctp-overlay1)"} />
  {/each}

  <!-- the face, with a ring round the middle -->
  <circle cx="32" cy="32" r="24.6" fill="url(#{uid}-face)" />
  <circle cx="32" cy="32" r="24.6" fill="none" stroke={sealed ? "var(--ctp-yellow)" : "var(--ctp-surface2)"} stroke-opacity={sealed ? 0.6 : 0.9} stroke-width="0.8" />
  <circle cx="32" cy="32" r="7.6" fill="none" stroke={sealed ? "var(--ctp-yellow)" : "var(--ctp-surface2)"} stroke-opacity={sealed ? 0.7 : 0.9} stroke-width="0.8" />
  {#if sealed}
    <path d="M32 25.2 L33.5 30.5 L38.8 32 L33.5 33.5 L32 38.8 L30.5 33.5 L25.2 32 L30.5 30.5 Z" fill="url(#{uid}-metal)" />
  {/if}

  {#each sockets as s (s.i)}
    {#if s.filled}
      <!-- a stone tablet turned to face outward, centred on its place -->
      <g transform="translate({s.x} {s.y}) rotate({(s.angle * 180) / Math.PI + 90}) translate({-16 * size} {-20 * size}) scale({size})">
        <rect x="1.4" y="2.6" width="32" height="40" rx="6" fill="black" opacity="0.32" />
        <rect x="0" y="0" width="32" height="40" rx="6" class="fill-surface1" style={sealed ? "stroke: var(--ctp-yellow); stroke-opacity: 0.75" : "stroke: var(--ctp-overlay0)"} stroke-width="1.8" />
        <rect x="2" y="2" width="28" height="36" rx="4.4" fill="none" stroke="white" stroke-opacity="0.12" stroke-width="1" />
        <path d={glyphPath(s.filled.glyph)} transform="translate(0.9 1.1)" fill="none" stroke="black" stroke-opacity="0.4" stroke-width="3.8" stroke-linecap="round" stroke-linejoin="round" />
        <path d={glyphPath(s.filled.glyph)} fill="none" class="stroke-overlay0" stroke-width="3.4" stroke-linecap="round" stroke-linejoin="round" />
        <path
          d={glyphPath(s.filled.glyph)}
          fill="none"
          style="stroke: {s.filled.glow ?? 'var(--accent)'}"
          stroke-opacity={0.3 + 0.7 * Math.min(1, Math.max(0, s.filled.lit))}
          stroke-width="3.4"
          stroke-linecap="round"
          stroke-linejoin="round"
        />
      </g>
    {:else}
      <!-- an empty place: a shallow dish waiting for its tablet -->
      <circle cx={s.x} cy={s.y} r={count <= 4 ? 3.6 : 3} fill="black" opacity="0.26" />
      <circle cx={s.x} cy={s.y} r={count <= 4 ? 3.2 : 2.6} class="fill-surface1" stroke="var(--ctp-overlay0)" stroke-opacity="0.6" stroke-width="0.7" />
    {/if}
  {/each}
</svg>
