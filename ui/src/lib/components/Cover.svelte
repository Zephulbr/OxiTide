<script lang="ts">
  import { coverUrl, type CoverSize } from "../artwork";
  import Icon from "./Icon.svelte";

  let {
    uuid,
    size = 320,
    radius = 6,
    alt = "",
  }: { uuid: string | null | undefined; size?: CoverSize; radius?: number; alt?: string } = $props();

  let failed = $state(false);
  $effect(() => {
    uuid;
    failed = false;
  });
  const src = $derived(uuid && !failed ? coverUrl(uuid, size) : "");
</script>

{#if src}
  <img
    class="cover"
    style:border-radius="{radius}px"
    {src}
    {alt}
    loading="lazy"
    draggable="false"
    onerror={() => (failed = true)}
  />
{:else}
  <div class="cover fallback" style:border-radius="{radius}px" role="img" aria-label={alt || "No artwork"}>
    <Icon name="music" size={20} />
  </div>
{/if}

<style>
  .cover {
    display: block;
    width: 100%;
    aspect-ratio: 1;
    object-fit: cover;
    background: var(--surface-2);
  }
  .fallback {
    display: grid;
    place-items: center;
    color: var(--text-muted);
    background: linear-gradient(135deg, var(--surface-2), var(--surface-3));
  }
</style>
