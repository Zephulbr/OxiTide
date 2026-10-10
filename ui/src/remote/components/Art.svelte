<script lang="ts">
  import Icon from "../../lib/components/Icon.svelte";

  let {
    src,
    size = 48,
    round = false,
    eager = false,
  }: { src: string; size?: number | string; round?: boolean; eager?: boolean } = $props();

  let failed = $state(false);
  $effect(() => {
    void src;
    failed = false;
  });

  const box = $derived(typeof size === "number" ? `${size}px` : size);
</script>

<span class="art" class:round style:width={box} style:height={box}>
  {#if src && !failed}
    <img {src} alt="" loading={eager ? "eager" : "lazy"} decoding="async" onerror={() => (failed = true)} />
  {:else}
    <Icon name="music" size={20} />
  {/if}
</span>

<style>
  .art {
    display: inline-grid;
    place-items: center;
    flex: none;
    overflow: hidden;
    border-radius: var(--radius-card-sharp);
    background: var(--surface-2);
    color: var(--text-muted);
  }
  .round {
    border-radius: 50%;
  }
  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    display: block;
  }
</style>
