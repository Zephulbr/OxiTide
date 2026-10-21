<script lang="ts">
  import type { CatjamCoat } from "../../api";
  import { COAT_IDS, COAT_LABELS, renderIdle } from "../../catjamSprite";

  /// The jamming cat's coat (CJ-D12): one button per coat showing the cat in
  /// it. Same semantics as `Segmented` (a labelled group of pressed-state
  /// buttons; arrow keys move focus and never reach the transport shortcuts).
  let { value, onchange }: { value: CatjamCoat; onchange: (coat: CatjamCoat) => void } = $props();

  const swatches = COAT_IDS.map((id) => ({ id, label: COAT_LABELS[id], src: renderIdle(id) }));

  function onKeydown(event: KeyboardEvent) {
    const keys = ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const group = event.currentTarget as HTMLElement;
    const buttons = [...group.querySelectorAll<HTMLButtonElement>("button")];
    const at = buttons.indexOf(document.activeElement as HTMLButtonElement);
    if (at < 0) return;
    event.preventDefault();
    event.stopPropagation();
    let next = at;
    if (event.key === "Home") next = 0;
    else if (event.key === "End") next = buttons.length - 1;
    else next = (at + (event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div class="coats" role="group" aria-label="Coat" onkeydown={onKeydown}>
  {#each swatches as swatch (swatch.id)}
    <button
      type="button"
      class:active={swatch.id === value}
      aria-pressed={swatch.id === value}
      aria-label={swatch.label}
      title={swatch.label}
      onclick={() => onchange(swatch.id)}
    >
      {#if swatch.src}<img src={swatch.src} alt="" width="32" height="32" />{/if}
    </button>
  {/each}
</div>

<style>
  .coats {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 3px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-1);
  }
  button {
    display: grid;
    place-items: center;
    width: 44px;
    height: 44px;
    border-radius: calc(var(--radius-control) - 3px);
  }
  button:hover:not(.active) {
    background: var(--surface-2);
  }
  button.active {
    background: color-mix(in srgb, var(--accent) 18%, var(--surface-3));
    box-shadow: inset 0 0 0 1px var(--accent);
  }
  img {
    image-rendering: pixelated;
  }
</style>
