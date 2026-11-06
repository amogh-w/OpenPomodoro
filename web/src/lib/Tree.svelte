<script lang="ts">
  interface Props {
    /** Sessions done towards this tree, and how many it takes. */
    done: number;
    total: number;
    class?: string;
    label?: string;
  }
  let { done, total, class: extra = "", label }: Props = $props();

  const count = $derived(Math.max(1, Math.min(total, 12)));
  const grown = $derived(Math.max(0, Math.min(done, count)));
  // The canopy is a ring of leafy clumps around a centre; one fills in for every session done, the middle last.
  const clumps = $derived(
    Array.from({ length: count }, (_, i) => {
      const angle = (i / count) * 2 * Math.PI - Math.PI / 2;
      const radius = count === 1 ? 0 : 11;
      return { i, x: 24 + radius * Math.cos(angle), y: 20 + radius * 0.85 * Math.sin(angle) };
    }),
  );
  const complete = $derived(grown >= count);
</script>

<svg class="block {extra}" viewBox="0 0 48 56" role={label ? "img" : undefined} aria-label={label} aria-hidden={label ? undefined : true}>
  <ellipse cx="24" cy="52" rx="15" ry="3.6" class="fill-surface1" />
  <!-- the trunk -->
  <path d="M21 52 L22 32 L26 32 L27 52 Z" style="fill: color-mix(in oklab, var(--ctp-peach) 45%, var(--ctp-crust))" />
  {#each clumps as c (c.i)}
    <circle
      cx={c.x}
      cy={c.y}
      r={count === 1 ? 14 : 8.5}
      class="transition-[opacity] duration-500 {c.i < grown ? 'fill-green' : 'fill-surface1'}"
      opacity={c.i < grown ? 1 : 0.45}
    />
  {/each}
  {#if count > 1 && complete}
    <circle cx="24" cy="20" r="9.5" class="fill-green" />
  {/if}
  {#if complete}
    <circle cx="19" cy="17" r="2" class="fill-red" />
    <circle cx="29" cy="22" r="2" class="fill-red" />
    <circle cx="23" cy="26" r="2" class="fill-red" />
  {/if}
</svg>
