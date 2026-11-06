<script lang="ts">
  import type { Stage } from "./garden";

  interface Props {
    stage: Stage;
    /** Which of three shapes (the same session always gets the same one). */
    variant?: 0 | 1 | 2;
    /** A theme colour for the bloom, such as `var(--ctp-peach)`; the accent when not given. */
    bloom?: string;
    class?: string;
    style?: string;
    /** Spoken description; leave out for a purely decorative plant. */
    label?: string;
  }
  let { stage, variant = 0, bloom = "var(--accent)", class: extra = "", style, label }: Props = $props();

  // Leaves grow in pairs up the stem as the stage rises; a fully grown plant flowers.
  const leaves = $derived([
    { y: 38, dx: 7, tilt: 28 },
    { y: 32, dx: 8, tilt: 34 },
    { y: 26, dx: 7, tilt: 40 },
  ].slice(0, stage === 0 ? 0 : stage));
  const top = $derived([40, 33, 25, 15][stage] ?? 40);
</script>

<svg class="block {extra}" {style} viewBox="0 0 48 48" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <ellipse cx="24" cy="43" rx="14" ry="4" class="fill-surface1" />
  <!-- the stem -->
  <path d="M24 42 L24 {top}" class="stroke-green" stroke-width="2.4" stroke-linecap="round" fill="none" />
  {#if stage === 0}
    <!-- a sprout: two tiny leaves -->
    <ellipse cx="20.5" cy="38" rx="4.5" ry="2.4" transform="rotate(-25 20.5 38)" class="fill-green" />
    <ellipse cx="27.5" cy="38" rx="4.5" ry="2.4" transform="rotate(25 27.5 38)" class="fill-green" />
  {/if}
  {#each leaves as leaf, i (i)}
    <ellipse cx={24 - leaf.dx} cy={leaf.y} rx={variant === 1 ? 4 : 5.5} ry={variant === 2 ? 3.2 : 2.6} transform="rotate({-leaf.tilt} {24 - leaf.dx} {leaf.y})" class="fill-green" />
    <ellipse cx={24 + leaf.dx} cy={leaf.y - 2} rx={variant === 1 ? 4 : 5.5} ry={variant === 2 ? 3.2 : 2.6} transform="rotate({leaf.tilt} {24 + leaf.dx} {leaf.y - 2})" class="fill-green" />
  {/each}
  {#if stage === 1}
    <!-- a bud -->
    <circle cx="24" cy="31" r="3" class="fill-green" />
  {/if}
  {#if stage === 2}
    <circle cx="24" cy="22" r="3.4" class="fill-green" />
  {/if}
  {#if stage === 3}
    <g transform="translate(0 -3)">
    {#if variant === 0}
      <!-- a round flower -->
      {#each [0, 60, 120, 180, 240, 300] as angle (angle)}
        <ellipse cx="24" cy="10.5" rx="3.6" ry="5" transform="rotate({angle} 24 17)" style="fill: {bloom}" />
      {/each}
      <circle cx="24" cy="17" r="3.4" class="fill-yellow" />
    {:else if variant === 1}
      <!-- a tulip -->
      <path d="M17 13 Q17 24 24 24 Q31 24 31 13 L27.5 16.5 L24 11 L20.5 16.5 Z" style="fill: {bloom}" />
    {:else}
      <!-- a leafy bud with berries -->
      <circle cx="24" cy="18" r="6.5" class="fill-green" />
      <circle cx="21" cy="16" r="2" style="fill: {bloom}" />
      <circle cx="27.5" cy="17.5" r="2" style="fill: {bloom}" />
      <circle cx="24.5" cy="21.5" r="2" style="fill: {bloom}" />
    {/if}
    </g>
  {/if}
</svg>
