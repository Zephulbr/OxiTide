<script lang="ts">
  import type { Snippet } from "svelte";
  import Icon from "../../lib/components/Icon.svelte";
  import Art from "./Art.svelte";

  let {
    image,
    title,
    subtitle = "",
    kind,
    round = false,
    favorite = null,
    ontogglefavorite,
    children,
  }: {
    image: string;
    title: string;
    subtitle?: string;
    kind: string;
    round?: boolean;
    /// `null` hides the heart (no favorites for this kind).
    favorite?: boolean | null;
    ontogglefavorite?: () => void;
    children?: Snippet;
  } = $props();
</script>

<header class="head">
  <button class="icon-btn back" aria-label="Back" onclick={() => history.back()}>
    <Icon name="chevron-left" size={22} />
  </button>
  <div class="art"><Art src={image} size="100%" {round} eager /></div>
  <p class="kind muted">{kind}</p>
  <h1>{title}</h1>
  {#if subtitle}<p class="muted sub">{subtitle}</p>{/if}
  <div class="row">
    {@render children?.()}
    {#if favorite !== null}
      <button
        class="icon-btn fill"
        class:on={favorite}
        aria-pressed={favorite}
        aria-label={favorite ? "Remove from favorites" : "Add to favorites"}
        onclick={ontogglefavorite}
      >
        <Icon name="heart" size={22} />
      </button>
    {/if}
  </div>
</header>

<style>
  .head {
    position: relative;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    padding-top: 8px;
    margin-bottom: 12px;
  }
  .back {
    position: absolute;
    left: -8px;
    top: 0;
  }
  .art {
    width: min(64vw, 280px);
    aspect-ratio: 1;
    margin-bottom: 12px;
  }
  .art :global(.art) {
    width: 100% !important;
    height: 100% !important;
    border-radius: var(--radius-card);
    box-shadow: 0 14px 32px rgba(0, 0, 0, 0.3);
  }
  .art :global(.art.round) {
    border-radius: 50%;
  }
  .kind {
    margin: 0;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }
  h1 {
    font-size: 24px;
    margin: 2px 0;
  }
  .sub {
    margin: 0;
  }
  .row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 8px;
  }
</style>
