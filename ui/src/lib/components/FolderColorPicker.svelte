<script lang="ts">
  import { FOLDER_COLORS, folderColorCss, type FolderColorId } from "../folderMeta.svelte";
  import Icon from "./Icon.svelte";

  /// Folder colour swatches for the New / Edit folder dialogs: a folder icon
  /// per colour (plus the muted default), one radio group, arrow keys move.
  let { value = $bindable(null) }: { value?: FolderColorId | null } = $props();

  const options = [
    { id: null, label: "Default", color: null as string | null },
    ...FOLDER_COLORS.map((c) => ({ id: c.id as FolderColorId | null, label: c.label, color: folderColorCss(c.hue) })),
  ];

  let buttons: HTMLButtonElement[] = [];

  function onKey(event: KeyboardEvent, index: number) {
    const step = event.key === "ArrowRight" || event.key === "ArrowDown" ? 1 : event.key === "ArrowLeft" || event.key === "ArrowUp" ? -1 : 0;
    if (step === 0) return;
    event.preventDefault();
    const next = (index + step + options.length) % options.length;
    value = options[next].id;
    buttons[next]?.focus();
  }
</script>

<div class="swatches" role="radiogroup" aria-label="Folder color">
  {#each options as option, i (option.label)}
    <button
      type="button"
      class="swatch"
      class:selected={value === option.id}
      role="radio"
      aria-checked={value === option.id}
      aria-label={option.label}
      title={option.label}
      tabindex={value === option.id ? 0 : -1}
      style:color={option.color ?? undefined}
      bind:this={buttons[i]}
      onclick={() => (value = option.id)}
      onkeydown={(event) => onKey(event, i)}
    ><Icon name="folder" size={20} /></button>
  {/each}
</div>

<style>
  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }
  .swatch {
    display: grid;
    place-items: center;
    width: 38px;
    height: 38px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    /* Default (no colour set): the muted folder. */
    color: var(--text-muted);
    background: var(--surface-2);
    transition:
      background-color var(--dur-micro) var(--ease-enter),
      border-color var(--dur-micro) var(--ease-enter);
  }
  .swatch:hover {
    background: var(--surface-3);
  }
  .swatch.selected {
    border-color: currentColor;
    background: color-mix(in srgb, currentColor 16%, var(--surface-2));
    box-shadow: 0 0 0 1px currentColor;
  }
  .swatch:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
</style>
