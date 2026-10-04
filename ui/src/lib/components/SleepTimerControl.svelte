<script lang="ts">
  import { formatDuration } from "../format";
  import { menu, type MenuItem } from "../menu.svelte";
  import type { SleepTimer } from "../sleepTimer.svelte";
  import Icon from "./Icon.svelte";
  import Modal from "./Modal.svelte";

  let { sleepTimer }: { sleepTimer: SleepTimer } = $props();

  const PRESETS = [15, 30, 45, 60];

  let customOpen = $state(false);
  let customMinutes = $state("30");

  const countdown = $derived(
    sleepTimer.remainingMs === null
      ? sleepTimer.mode === "end-of-track"
        ? "End"
        : null
      : formatDuration(Math.ceil(sleepTimer.remainingMs / 1000)),
  );

  const customValue = $derived(Number(customMinutes));
  const customValid = $derived(
    Number.isInteger(customValue) && customValue >= 1 && customValue <= 1440,
  );

  const title = $derived(
    sleepTimer.mode === "duration"
      ? `Sleep timer: ${sleepTimer.totalMinutes} min remaining`
      : sleepTimer.mode === "end-of-track"
        ? "Sleep timer: end of current track"
        : "Sleep timer",
  );

  function openMenu(event: MouseEvent) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu.show(rect.left, rect.bottom + 4, items());
  }

  function items(): MenuItem[] {
    return [
      {
        id: "off",
        label: "Off",
        selected: sleepTimer.mode === "off",
        run: () => sleepTimer.cancel(),
      },
      ...PRESETS.map((minutes) => ({
        id: `minutes-${minutes}`,
        label: `${minutes} minutes`,
        selected: sleepTimer.mode === "duration" && sleepTimer.totalMinutes === minutes,
        run: () => sleepTimer.armMinutes(minutes),
      })),
      {
        id: "custom",
        label: "Custom…",
        selected:
          sleepTimer.mode === "duration" && !PRESETS.includes(sleepTimer.totalMinutes ?? 0),
        run: () => {
          customMinutes = String(sleepTimer.totalMinutes ?? 30);
          customOpen = true;
        },
      },
      {
        id: "end-of-track",
        label: "End of current track",
        selected: sleepTimer.mode === "end-of-track",
        run: () => sleepTimer.armEndOfTrack(),
      },
    ];
  }

  function startCustom(event: SubmitEvent) {
    event.preventDefault();
    if (!customValid) return;
    sleepTimer.armMinutes(customValue);
    customOpen = false;
  }
</script>

<button
  class="moon"
  class:on={sleepTimer.mode !== "off"}
  onclick={openMenu}
  {title}
  aria-label="Sleep timer"
>
  <Icon name="moon" size={18} />
  {#if countdown !== null}<span class="count tnum">{countdown}</span>{/if}
</button>

{#if customOpen}
  <Modal title="Sleep timer" onclose={() => (customOpen = false)}>
    <form class="custom" onsubmit={startCustom}>
      <label for="sleep-minutes">Minutes (1–1440)</label>
      <input id="sleep-minutes" type="number" min="1" max="1440" step="1" bind:value={customMinutes} />
      <button type="submit" disabled={!customValid}>Start timer</button>
    </form>
  </Modal>
{/if}

<style>
  .moon {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    min-width: 30px;
    height: 30px;
    padding: 0 6px;
    border: 0;
    border-radius: 15px;
    background: transparent;
    color: var(--text-muted);
    cursor: pointer;
    transition:
      color var(--dur-micro),
      background-color var(--dur-micro);
  }
  .moon:hover {
    background: var(--surface-3);
  }
  .moon.on {
    color: var(--accent);
  }
  .count {
    font-size: 12px;
    white-space: nowrap;
  }
  .custom {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 220px;
  }
  .custom label {
    font-size: 13px;
    color: var(--text-muted);
  }
</style>
