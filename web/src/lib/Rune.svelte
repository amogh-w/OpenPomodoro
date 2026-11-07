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
  </defs>
  <!-- the stone -->
  <rect x="1.5" y="1.5" width="29" height="37" rx="7" class="fill-surface0 stroke-surface2" stroke-width="1.2" />
  <!-- the carving, dim -->
  <path d={path} fill="none" class="stroke-overlay0" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
  {#if level > 0}
    <!-- the same carving, lit as far as the session went -->
    <g clip-path="url(#{uid})">
      <path d={path} fill="none" style="stroke: {glow}" stroke-opacity="0.28" stroke-width="6" stroke-linecap="round" stroke-linejoin="round" />
      <path d={path} fill="none" style="stroke: {glow}" stroke-width="2.6" stroke-linecap="round" stroke-linejoin="round" />
    </g>
  {/if}
</svg>
