<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { tick, untrack } from "svelte";
  import { api, type DevicePrefsPatch, type OutputDevice, type OutputDeviceList, type OutputMode } from "../../api";
  import { menu, type MenuItem } from "../../menu.svelte";
  import { outputDevices } from "../../outputDeviceStore.svelte";
  import {
    DEVICE_ICON_CHOICES,
    OUTPUT_DEVICES_CHANGED_EVENT,
    connectionLabel,
    deviceError,
    deviceIcon,
    displayName,
    kindIcon,
    numberRuns,
    pickerView,
    supportsLine,
  } from "../../outputDevices";
  import Icon from "../Icon.svelte";
  import Segmented from "./Segmented.svelte";

  /// Settings → Playback → Output: one row per output, grouped by type, with a
  /// mode per device and nickname / pin / hide / icon in each row's menu (spec
  /// 2026-11-05-oxitide-output-device-picker-design.md §8.1). Rows are radios:
  /// the arrow keys move focus and Enter or Space selects, because selecting
  /// reopens the output. The selected device stays at the top of its group;
  /// the group's other rows scroll under it after a few rows, so the Output
  /// card stays close to its neighbours' height.
  let {
    icons,
    onlist,
    onerror,
  }: {
    /// `ui.device_icons`.
    icons: boolean;
    /// Every list the picker receives (Settings derives the fallback row from it).
    onlist?: (list: OutputDeviceList) => void;
    onerror: (error: unknown) => void;
  } = $props();

  const uid = $props.id();
  let list = $state<OutputDeviceList | null>(null);
  let showHidden = $state(false);
  let showAbsent = $state(false);
  let editing = $state<string | null>(null);
  let draft = $state("");
  let focusedId = $state<string | null>(null);
  let input = $state<HTMLInputElement | null>(null);
  /// Id moves already asked for this session (a USB DAC on another port).
  const movedOnce = new Set<string>();

  const MODE_OPTIONS: { value: OutputMode; label: string }[] = [
    { value: "shared", label: "Shared" },
    { value: "exclusive", label: "Exclusive" },
  ];

  const view = $derived(list ? pickerView(list, showHidden, showAbsent) : null);
  const selected = $derived(list?.selected ?? "");
  /// The roving tab stop: the focused row, else the selected one.
  const tabStop = $derived(focusedId ?? selected);

  async function load() {
    try {
      let next = await api.getOutputDevices();
      const moves = next.moved.filter(([from, to]) => !movedOnce.has(`${from}>${to}`));
      if (moves.length > 0) {
        for (const [from, to] of moves) {
          movedOnce.add(`${from}>${to}`);
          await api.moveDevicePrefs(from, to);
        }
        next = await api.getOutputDevices();
      }
      commit(next);
      onlist?.(next);
    } catch (e) {
      onerror(deviceError("Couldn't read the output devices", e));
    }
  }

  /// Show a list here and in the quality menu.
  function commit(next: OutputDeviceList) {
    list = next;
    outputDevices.set(next);
  }

  $effect(() => {
    untrack(() => void load());
    const unlisten = listen(OUTPUT_DEVICES_CHANGED_EVENT, () => void load());
    // Linux has no device watcher: coming back to the window re-reads the list.
    const onFocus = () => void load();
    window.addEventListener("focus", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      void unlisten.then((off) => off());
    };
  });

  function update(id: string, change: (device: OutputDevice) => OutputDevice) {
    if (!list) return;
    commit({ ...list, devices: list.devices.map((d) => (d.id === id ? change(d) : d)) });
  }

  function select(id: string) {
    if (!list || id === list.selected) return;
    const device = list.devices.find((d) => d.id === id);
    const name = device ? displayName(device) : "System default";
    commit({ ...list, selected: id });
    // The row moves to the top of its group (a new element), so focus follows it.
    refocus(id);
    api.setOutputDevice(id).then(load, (e) => {
      onerror(deviceError(`Couldn't switch to ${name}`, e));
      void load();
    });
  }

  function setMode(id: string, mode: OutputMode) {
    if (!list) return;
    if (id === "") commit({ ...list, system_default: { ...list.system_default, mode } });
    else update(id, (d) => ({ ...d, prefs: { ...d.prefs, mode } }));
    onlist?.(list);
    api.setDeviceMode(id, mode).catch((e) => {
      onerror(deviceError(`Couldn't switch to ${mode === "exclusive" ? "Exclusive" : "Shared"}`, e));
      void load();
    });
  }

  function patch(device: OutputDevice, change: DevicePrefsPatch) {
    update(device.id, (d) => ({ ...d, prefs: { ...d.prefs, ...change } }));
    if (list && change.pinned !== undefined) {
      const pinned = list.pinned.filter((id) => id !== device.id);
      commit({ ...list, pinned: change.pinned ? [...pinned, device.id] : pinned });
    }
    api.setDevicePrefs(device.id, change).then(
      () => list && onlist?.(list),
      (e) => {
        onerror(deviceError(`Couldn't save the change to ${displayName(device)}`, e));
        void load();
      },
    );
  }

  function startNickname(device: OutputDevice) {
    editing = device.id;
    draft = device.prefs.nickname;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  }

  function saveNickname(device: OutputDevice) {
    editing = null;
    patch(device, { nickname: draft.trim() });
    refocus(device.id);
  }

  function cancelNickname(id: string) {
    editing = null;
    refocus(id);
  }

  function refocus(id: string) {
    void tick().then(() => document.querySelector<HTMLElement>(`[data-pick="${CSS.escape(id)}"]`)?.focus());
  }

  function menuItems(device: OutputDevice): MenuItem[] {
    const pinned = device.prefs.pinned;
    const items: MenuItem[] = [
      { id: "nickname", label: "Nickname…", icon: "pencil", run: () => startNickname(device) },
      {
        id: "pin",
        label: pinned ? "Unpin from the quality menu" : "Pin to the quality menu",
        icon: pinned ? "pin-off" : "pin",
        run: () => patch(device, { pinned: !pinned }),
      },
      {
        id: "hide",
        label: device.prefs.hidden ? "Show in the list" : "Hide",
        icon: device.prefs.hidden ? "eye" : "eye-off",
        disabled: !device.prefs.hidden && device.id === selected,
        detail: !device.prefs.hidden && device.id === selected ? "Switch to another device first" : undefined,
        run: () => patch(device, { hidden: !device.prefs.hidden }),
      },
    ];
    for (const [i, choice] of DEVICE_ICON_CHOICES.entries()) {
      const current = device.prefs.icon || "";
      items.push({
        id: `icon-${choice.value || "auto"}`,
        label: choice.label,
        icon: choice.value === "" ? kindIcon(device.kind) : choice.icon,
        section: "Icon",
        divider: i === 0,
        selected: current === choice.value,
        trailing: current === choice.value ? "check" : undefined,
        run: () => patch(device, { icon: choice.value === "" ? "auto" : choice.value }),
      });
    }
    return items;
  }

  function openMenu(device: OutputDevice, button: HTMLElement) {
    const rect = button.getBoundingClientRect();
    menu.show(rect.right, rect.bottom + 4, menuItems(device), button, { align: "end" });
  }

  function onRowKeydown(event: KeyboardEvent, device: OutputDevice) {
    if (event.key === "ContextMenu" || (event.key === "F10" && event.shiftKey)) {
      event.preventDefault();
      openMenu(device, event.currentTarget as HTMLElement);
    }
  }

  function onListKeydown(event: KeyboardEvent) {
    const keys = ["ArrowDown", "ArrowUp", "Home", "End"];
    if (!keys.includes(event.key)) return;
    const target = event.target as HTMLElement | null;
    if (!target?.matches("[data-pick]")) return;
    const picks = [...(event.currentTarget as HTMLElement).querySelectorAll<HTMLElement>("[data-pick]")];
    const at = picks.indexOf(target);
    if (at < 0) return;
    event.preventDefault();
    // The arrows must not reach the global volume/seek shortcuts.
    event.stopPropagation();
    let next = at;
    if (event.key === "Home") next = 0;
    else if (event.key === "End") next = picks.length - 1;
    else next = Math.min(picks.length - 1, Math.max(0, at + (event.key === "ArrowDown" ? 1 : -1)));
    picks[next]?.focus();
  }

  const absentNames = $derived(view ? view.absent.map((d) => displayName(d)).join(", ") : "");
</script>

{#snippet detail(id: string, mode: OutputMode, asio: boolean, supports: string | null)}
  <div class="detail">
    {#if asio}
      <span class="always">Always exclusive</span>
    {:else}
      <Segmented label="Output mode" value={mode} options={MODE_OPTIONS} onchange={(m) => setMode(id, m)} />
    {/if}
    <!-- What is playing now is the player bar's to say; this is what the device can do. -->
    {#if supports}
      <span class="facts">{@render numbers(supports)}</span>
    {/if}
  </div>
{/snippet}

<!-- Rates and depths stand out from their units: "Up to **192** kHz · **24**-bit". -->
{#snippet numbers(text: string)}
  {#each numberRuns(text) as run, i (i)}{#if run.number}<span class="num">{run.text}</span>{:else}{run.text}{/if}{/each}
{/snippet}

{#snippet deviceRow(device: OutputDevice)}
  {@const isSel = device.id === selected}
  {@const icon = icons ? deviceIcon(device) : null}
  {@const descId = `${uid}-${device.id}-desc`}
  <div class="dev" class:sel={isSel} class:off={!device.connected} class:hidden-row={device.prefs.hidden}>
    {#if editing === device.id}
      <div class="pick editing">
        {#if icon}<span class="tile" aria-hidden="true"><Icon name={icon} size={18} /></span>{/if}
        <div class="edit">
          <input
            bind:this={input}
            bind:value={draft}
            aria-label={`Nickname for ${device.name}`}
            placeholder={device.name}
            maxlength="64"
            onkeydown={(e) => {
              e.stopPropagation();
              // preventDefault: focus returns to the row's radio before the
              // key finishes, and an unhandled Enter would then press it and
              // switch to this device.
              if (e.key === "Enter") {
                e.preventDefault();
                saveNickname(device);
              } else if (e.key === "Escape") {
                e.preventDefault();
                cancelNickname(device.id);
              }
            }}
          />
          <button type="button" class="btn primary" onclick={() => saveNickname(device)}>Save</button>
          <button type="button" class="btn" onclick={() => cancelNickname(device.id)}>Cancel</button>
        </div>
      </div>
    {:else}
      <button
        type="button"
        class="pick"
        role="radio"
        aria-checked={isSel}
        aria-describedby={descId}
        data-pick={device.id}
        tabindex={tabStop === device.id ? 0 : -1}
        title={device.system_name}
        disabled={!device.connected && !isSel}
        onclick={() => select(device.id)}
        onfocus={() => (focusedId = device.id)}
        onkeydown={(e) => onRowKeydown(e, device)}
      >
        {#if icon}<span class="tile" aria-hidden="true"><Icon name={icon} size={18} /></span>{/if}
        <span class="text">
          <span class="name">{displayName(device)}</span>
          <span class="sub" id={descId}>{connectionLabel(device)}</span>
        </span>
      </button>
    {/if}
    <div class="tools">
      <button
        type="button"
        class="ib pin"
        class:pinned={device.prefs.pinned}
        aria-pressed={device.prefs.pinned}
        aria-label={device.prefs.pinned ? `Unpin ${displayName(device)}` : `Pin ${displayName(device)} to the quality menu`}
        title={device.prefs.pinned ? "Pinned to the quality menu" : "Pin to the quality menu"}
        tabindex={tabStop === device.id ? 0 : -1}
        onclick={() => patch(device, { pinned: !device.prefs.pinned })}
      ><Icon name="pin" size={15} /></button>
      <button
        type="button"
        class="ib"
        aria-haspopup="menu"
        aria-label={`More for ${displayName(device)}`}
        title="More"
        tabindex={tabStop === device.id ? 0 : -1}
        onclick={(e) => openMenu(device, e.currentTarget)}
      ><Icon name="ellipsis" size={16} /></button>
    </div>
    {#if isSel}
      {@render detail(device.id, device.prefs.mode, device.group === "asio", supportsLine(device))}
    {/if}
  </div>
{/snippet}

{#if list && view}
  <div
    class="picker"
    role="radiogroup"
    aria-label="Audio device"
    tabindex="-1"
    onkeydown={onListKeydown}
  >
    <div class="dev" class:sel={selected === ""}>
      <button
        type="button"
        class="pick"
        role="radio"
        aria-checked={selected === ""}
        data-pick=""
        tabindex={tabStop === "" ? 0 : -1}
        onclick={() => select("")}
        onfocus={() => (focusedId = "")}
      >
        {#if icons}<span class="tile" aria-hidden="true"><Icon name="monitor-speaker" size={18} /></span>{/if}
        <span class="text">
          <span class="name">System default</span>
          <span class="sub">
            {list.system_default.now ? `Follows the system · now ${list.system_default.now}` : "Follows the system"}
          </span>
        </span>
      </button>
      {#if selected === ""}
        {@render detail("", list.system_default.mode, false, null)}
      {/if}
    </div>

    {#each view.groups as group (group.group)}
      <div class="group-label" aria-hidden="true">{group.label}</div>
      {#if group.selected}
        {@render deviceRow(group.selected)}
      {/if}
      {#if group.devices.length > 0}
        <div class="rows" class:under={group.selected}>
          {#each group.devices as device (device.id)}
            {@render deviceRow(device)}
          {/each}
        </div>
      {/if}
    {/each}
  </div>

  {#if view.hiddenCount > 0 || view.absent.length > 0}
    <div class="more">
      {#if view.hiddenCount > 0}
        <span>
          {view.hiddenCount} hidden ·
          <button type="button" class="link" onclick={() => (showHidden = !showHidden)}>{showHidden ? "Hide" : "Show"}</button>
        </span>
      {/if}
      {#if view.absent.length > 0}
        <span>
          {absentNames}: nothing connected ·
          <button type="button" class="link" onclick={() => (showAbsent = !showAbsent)}>{showAbsent ? "Hide" : "Show anyway"}</button>
        </span>
      {/if}
    </div>
  {/if}
{/if}

<style>
  .picker {
    display: flex;
    flex-direction: column;
    gap: 2px;
    width: 100%;
    outline: none;
  }
  /* The selected device sits at the top of its group, outside the scroll.
     The rest show about four and a half rows (62px each with the gap), or
     three and a half under the selected row, then scroll; the cut row says
     there is more. The wheel passes on to the page at either end. */
  .rows {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 280px;
    overflow-y: auto;
  }
  .rows.under {
    max-height: 216px;
  }
  .group-label {
    margin: 14px 12px 6px;
    color: var(--text-muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .dev {
    /* Never shrink: in a height-capped group, flex would squeeze the selected
       row back to its 52px min-height and its detail would spill over the
       next row. The group scrolls instead. */
    flex: none;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
    align-items: center;
    column-gap: 8px;
    min-height: 52px;
    padding: 4px 6px 4px 4px;
    border: 1px solid transparent;
    border-radius: 10px;
  }
  .dev:hover {
    background: var(--surface-1);
  }
  .dev.sel {
    border-color: color-mix(in srgb, var(--accent) 45%, transparent);
    background: color-mix(in srgb, var(--accent) 7%, var(--surface-1));
  }
  .pick {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
    padding: 6px;
    border-radius: 8px;
    color: inherit;
    text-align: left;
    background: none;
  }
  .pick:disabled {
    cursor: default;
  }
  .off .tile,
  .off .name,
  .off .sub,
  .hidden-row .name {
    opacity: 0.5;
  }
  /* A bare glyph, no plate: the card headers' icons sit on plates, and the
     device icons must not read as more headers. */
  .tile {
    display: grid;
    flex: none;
    place-items: center;
    width: 24px;
    height: 24px;
    margin-left: 4px;
    color: var(--text-muted);
  }
  .sel .tile {
    color: var(--accent);
  }
  .text {
    display: flex;
    flex-direction: column;
    gap: 1px;
    min-width: 0;
  }
  .name {
    overflow: hidden;
    font-size: 14px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .sub {
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 500;
  }
  .tools {
    display: flex;
    align-items: center;
    gap: 2px;
  }
  .ib {
    display: grid;
    place-items: center;
    width: 30px;
    height: 30px;
    border-radius: 8px;
    color: var(--text-muted);
  }
  .ib:hover {
    color: var(--text);
    background: var(--surface-3);
  }
  /* Pins show on pinned rows; elsewhere on hover or keyboard focus. */
  .pin:not(.pinned) {
    opacity: 0;
  }
  .dev:hover .pin,
  .pin:focus-visible {
    opacity: 1;
  }
  .pin.pinned {
    color: var(--text);
  }
  .detail {
    display: flex;
    flex-wrap: wrap;
    grid-column: 1 / -1;
    align-items: center;
    justify-content: space-between;
    gap: 10px 16px;
    margin: 4px 6px 6px;
    padding-top: 10px;
    border-top: 1px solid var(--line);
    animation: detail-in var(--dur-state) var(--ease-enter);
  }
  @keyframes detail-in {
    from {
      opacity: 0;
      transform: translateY(-4px);
    }
  }
  .always {
    color: var(--text-muted);
    font-size: 12.5px;
  }
  .facts {
    color: var(--text-muted);
    font-size: 12.5px;
    line-height: 1.5;
    font-variant-numeric: tabular-nums;
    text-align: right;
  }
  /* The detail sits inside the selected row, so its numbers share the
     selection's accent. */
  .num {
    color: var(--accent);
    font-weight: 700;
  }
  .editing {
    flex: 1;
  }
  .edit {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .edit input {
    flex: 1;
    min-width: 0;
    padding: 6px 10px;
    border: 1px solid color-mix(in srgb, var(--accent) 60%, transparent);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-1);
    font: inherit;
    font-size: 14px;
    font-weight: 600;
  }
  .edit input::placeholder {
    color: var(--text-muted);
    font-weight: 500;
  }
  .btn {
    padding: 6px 12px;
    border: 1px solid var(--line);
    border-radius: 7px;
    color: var(--text);
    background: var(--surface-2);
    font-size: 12.5px;
    font-weight: 600;
  }
  .btn.primary {
    border-color: transparent;
    color: var(--on-accent);
    background: var(--accent);
  }
  .more {
    display: flex;
    flex-wrap: wrap;
    gap: 6px 18px;
    margin: 10px 12px 0;
    color: var(--text-muted);
    font-size: 12.5px;
    font-weight: 600;
  }
  .link {
    color: var(--text);
    font-weight: 600;
  }
  .link:hover {
    text-decoration: underline;
  }
  @media (prefers-reduced-motion: reduce) {
    .detail {
      animation: none;
    }
  }
</style>
