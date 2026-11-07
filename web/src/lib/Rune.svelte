<script lang="ts">
  import { glyphPath } from "./glyphs";

  interface Props {
    /** Which of the eight marks. */
    glyph: number;
    /** How much of it is lit, 0 to 1 (it fills from the bottom). */
    lit: number;
    /** A theme colour for the glow, such as `var(--ctp-peach)`; the accent when not given. */
    glow?: string;
    class?: string;
    style?: string;
    /** Spoken description; leave out for a purely decorative rune. */
    label?: string;
  }
  let { glyph, lit, glow = "var(--accent)", class: extra = "", style, label }: Props = $props();

  // Clips need an id that is unique on the page, however many runes are drawn.
  const uid = `rune-${Math.random().toString(36).slice(2, 9)}`;
  const level = $derived(Math.min(1, Math.max(0, lit)));
  const path = $derived(glyphPath(glyph));
</script>

<svg class="block {extra}" {style} viewBox="0 0 32 40" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <defs>
    <clipPath id={uid}><rect x="0" y={40 * (1 - level)} width="32" height={40 * level + 0.01} /></clipPath>
    <linearGradient id="{uid}-stone" x1="0" y1="0" x2="0" y2="1">
      <stop offset="0" style="stop-color: color-mix(in oklab, var(--ctp-surface1) 88%, white)" />
      <stop offset="1" style="stop-color: var(--ctp-surface0)" />
    </linearGradient>
  </defs>
  <!-- the stone: a soft shadow, a bevelled face, a notch at each end -->
  <rect x="2" y="3" width="29" height="37" rx="7" fill="black" opacity="0.3" />
  <rect x="1.2" y="1.2" width="29.6" height="37.6" rx="7" fill="url(#{uid}-stone)" class="stroke-surface2" stroke-width="1.2" />
  <rect x="3" y="3" width="26" height="34" rx="5.4" fill="none" stroke="white" stroke-opacity="0.12" stroke-width="0.9" />
  <circle cx="16" cy="5.2" r="0.9" class="fill-overlay1" />
  <circle cx="16" cy="34.8" r="0.9" class="fill-overlay1" />
  <!-- the carving, cut dark into the stone -->
  <path d={path} transform="translate(0.8 1)" fill="none" stroke="black" stroke-opacity="0.4" stroke-width="3" stroke-linecap="round" stroke-linejoin="round" />
  <path d={path} fill="none" class="stroke-overlay0" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
  {#if level > 0}
    <!-- the same carving, lit as far as the session went, with a halo and a bright core -->
    <g clip-path="url(#{uid})">
      <path d={path} fill="none" style="stroke: {glow}" stroke-opacity="0.3" stroke-width="6.4" stroke-linecap="round" stroke-linejoin="round" />
      <path d={path} fill="none" style="stroke: {glow}" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
      <path d={path} fill="none" stroke="white" stroke-opacity="0.45" stroke-width="0.9" stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {/if}
</svg>
