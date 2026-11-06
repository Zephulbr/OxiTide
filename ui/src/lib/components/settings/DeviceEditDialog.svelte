<script lang="ts">
  import type { DevicePrefsPatch, OutputDevice } from "../../api";
  import { DEVICE_ICON_CHOICES, kindIcon } from "../../outputDevices";
  import Icon from "../Icon.svelte";
  import Modal from "../Modal.svelte";

  /// Settings → Output: a device's nickname and icon, edited in a dialog like
  /// the playlist and folder edits, so the row's menu stays a short list of
  /// actions (pin, hide) instead of carrying the whole icon list.
  let {
    device,
    icons,
    onsave,
    onclose,
  }: {
    device: OutputDevice;
    /// `ui.device_icons`: with icons off the icon choice has nothing to show.
    icons: boolean;
    onsave: (change: DevicePrefsPatch) => void;
    onclose: () => void;
  } = $props();

  /// "auto" and "" both mean Automatic (the patch writes "auto").
  const normalized = (icon: string) => (icon === "auto" ? "" : icon);

  // svelte-ignore state_referenced_locally
  let name = $state(device.prefs.nickname);
  // svelte-ignore state_referenced_locally
  let icon = $state(normalized(device.prefs.icon));
  let tiles: HTMLButtonElement[] = [];

  function save(event: SubmitEvent) {
    event.preventDefault();
    const change: DevicePrefsPatch = {};
    const nickname = name.trim();
    if (nickname !== device.prefs.nickname) change.nickname = nickname;
    if (icon !== normalized(device.prefs.icon)) change.icon = icon === "" ? "auto" : icon;
    if (Object.keys(change).length > 0) onsave(change);
    onclose();
  }

  /// Arrow keys move the choice (radio group), wrapping like the folder colours.
  function onTileKey(event: KeyboardEvent, index: number) {
    const step =
      event.key === "ArrowRight" || event.key === "ArrowDown"
        ? 1
        : event.key === "ArrowLeft" || event.key === "ArrowUp"
          ? -1
          : 0;
    if (step === 0) return;
    event.preventDefault();
    event.stopPropagation();
    const next = (index + step + DEVICE_ICON_CHOICES.length) % DEVICE_ICON_CHOICES.length;
    icon = DEVICE_ICON_CHOICES[next].value;
    tiles[next]?.focus();
  }
</script>

<Modal title="Edit output" {onclose}>
  <form onsubmit={save}>
    <label>
      Nickname
      <input bind:value={name} aria-label="Nickname" placeholder={device.name} maxlength="64" />
      <span class="muted hint">Leave empty to show the device's own name.</span>
    </label>
    {#if icons}
      <div class="field">
        <span id="device-icon-label">Icon</span>
        <div class="tiles" role="radiogroup" aria-labelledby="device-icon-label">
          {#each DEVICE_ICON_CHOICES as choice, i (choice.value)}
            {@const glyph = choice.value === "" ? kindIcon(device.kind) : choice.icon}
            <button
              type="button"
              class="tile"
              class:selected={icon === choice.value}
              role="radio"
              aria-checked={icon === choice.value}
              tabindex={icon === choice.value ? 0 : -1}
              bind:this={tiles[i]}
              onclick={() => (icon = choice.value)}
              onkeydown={(event) => onTileKey(event, i)}
            >
              <span class="glyph" aria-hidden="true">
                {#if glyph}<Icon name={glyph} size={20} />{/if}
              </span>
              <span class="tile-label">{choice.label}</span>
            </button>
          {/each}
        </div>
      </div>
    {/if}
    <p class="actions">
      <button type="button" onclick={onclose}>Cancel</button>
      <button type="submit">Save</button>
    </p>
  </form>
</Modal>

<style>
  form {
    width: min(420px, 80vw);
  }
  label,
  .field {
    display: grid;
    gap: 6px;
  }
  .field {
    margin-top: 14px;
  }
  input {
    width: 100%;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    background: var(--surface-2);
    color: var(--text);
  }
  input::placeholder {
    color: var(--text-muted);
  }
  .hint {
    font-size: 12px;
  }
  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(88px, 1fr));
    gap: 6px;
  }
  .tile {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 10px 4px 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius-control);
    color: var(--text-muted);
    background: var(--surface-2);
    transition:
      background-color var(--dur-micro) var(--ease-enter),
      border-color var(--dur-micro) var(--ease-enter),
      color var(--dur-micro) var(--ease-enter);
  }
  .tile:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  .tile.selected {
    border-color: var(--accent);
    color: var(--text);
    background: color-mix(in srgb, var(--accent) 14%, var(--surface-2));
    box-shadow: 0 0 0 1px var(--accent);
  }
  .tile.selected .glyph {
    color: var(--accent);
  }
  .tile:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }
  /* "No icon" has no glyph; the slot keeps every label on one line. */
  .glyph {
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
  }
  .tile-label {
    font-size: 12px;
    font-weight: 600;
    line-height: 1.2;
    text-align: center;
  }
  .actions {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    margin: 16px 0 0;
  }
</style>
