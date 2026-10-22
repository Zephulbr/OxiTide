<script lang="ts">
  import { formatDuration } from "../format";
  import { menu, menuButton, type MenuItem } from "../menu.svelte";
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

  // The moon rocks like a cradle when its menu opens and again when a timer
  // is set (AR-2); the keyed remount replays the swing.
  let sways = $state(0);
  function sway() {
    sways += 1;
  }

  /// Opens away from the nearer window edge: upward from the player bar,
  /// downward from Rack's header. A downward menu at the bottom used to be
  /// clamped back over the bar and its own button.
  function openMenu(button: HTMLElement) {
    sway();
    const rect = button.getBoundingClientRect();
    if (rect.top > window.innerHeight / 2) {
      menu.show(rect.left, rect.top - 8, items(), button, { placement: "above" });
    } else {
      menu.show(rect.left, rect.bottom + 4, items(), button);
    }
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
        run: () => {
          sleepTimer.armMinutes(minutes);
          sway();
        },
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
        run: () => {
          sleepTimer.armEndOfTrack();
          sway();
        },
      },
    ];
  }

  function startCustom(event: SubmitEvent) {
    event.preventDefault();
    if (!customValid) return;
    sleepTimer.armMinutes(customValue);
    customOpen = false;
    sway();
  }
</script>

<button
  class="moon"
  class:on={sleepTimer.mode !== "off"}
  data-floor
  use:menuButton={openMenu}
  {title}
  aria-label="Sleep timer"
>
  {#key sways}<span class="moon-ico" class:sway={sways > 0}><Icon name="moon" size={18} /></span>{/key}
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
  /* A damped swing about the crescent's centre: out, back past rest, settle. */
  .moon-ico {
    display: inline-grid;
    place-items: center;
  }
  .moon-ico.sway {
    animation: moon-sway 750ms var(--ease-in-out);
  }
  @keyframes moon-sway {
    0% { transform: rotate(0deg); }
    30% { transform: rotate(-22deg); }
    60% { transform: rotate(10deg); }
    82% { transform: rotate(-3deg); }
    100% { transform: rotate(0deg); }
  }
  @media (prefers-reduced-motion: reduce) {
    .moon-ico.sway {
      animation: none;
    }
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
