<script lang="ts">
  // Caption buttons for the undecorated window (Windows; see lib/titlebar.ts).
  // Pinned to the top-right corner over every shell. The maximize button has a
  // native overlay on top of it (crates/oxitide-app/src/titlebar.rs) so Windows
  // 11 can open Snap Layouts on hover; that overlay owns the pointer there, so
  // its hover/press/click arrive as `titlebar://snap` events instead of DOM
  // events. `maximizable` is false in the mini player (no maximize, no overlay).
  import { listen } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { api } from "../api";
  import Icon from "./Icon.svelte";

  let { maximizable = true }: { maximizable?: boolean } = $props();

  const win = getCurrentWindow();
  let maximized = $state(false);
  let snapHover = $state(false);
  let snapPress = $state(false);
  let maxButton: HTMLButtonElement | undefined = $state();

  function toggleMaximize() {
    void win.toggleMaximize();
  }

  // Maximized state drives the maximize/restore glyph.
  $effect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    const sync = () => win.isMaximized().then((m) => (maximized = m)).catch(() => {});
    void sync();
    void win.onResized(sync).then((un) => (cancelled ? un() : (stop = un)));
    return () => {
      cancelled = true;
      stop?.();
    };
  });

  $effect(() => {
    let stop: (() => void) | undefined;
    let cancelled = false;
    void listen<string>("titlebar://snap", ({ payload }) => {
      if (payload === "enter") snapHover = true;
      else if (payload === "leave") snapHover = snapPress = false;
      else if (payload === "down") snapPress = true;
      else if (payload === "click") {
        snapPress = false;
        toggleMaximize();
      }
    }).then((un) => (cancelled ? un() : (stop = un)));
    return () => {
      cancelled = true;
      stop?.();
    };
  });

  // Tell Rust where the maximize button is (right-anchored, so window resizes
  // keep it aligned); hide the overlay whenever the button is gone.
  $effect(() => {
    const button = maxButton;
    if (!maximizable || !button) return;
    const report = () => {
      const r = button.getBoundingClientRect();
      void api
        .setSnapTarget({ right: window.innerWidth - r.left, top: r.top, width: r.width, height: r.height })
        .catch(() => {});
    };
    report();
    window.addEventListener("resize", report);
    return () => {
      window.removeEventListener("resize", report);
      snapHover = snapPress = false;
      void api.setSnapTarget(null).catch(() => {});
    };
  });
</script>

<div class="window-controls" role="group" aria-label="Window controls">
  <button class="cap" onclick={() => void win.minimize()} aria-label="Minimize" title="Minimize">
    <Icon name="minus" size={16} />
  </button>
  {#if maximizable}
    <button
      class="cap"
      class:hover={snapHover}
      class:press={snapPress}
      bind:this={maxButton}
      onclick={toggleMaximize}
      aria-label={maximized ? "Restore" : "Maximize"}
      title={maximized ? "Restore" : "Maximize"}
    >
      <Icon name={maximized ? "copy" : "square"} size={maximized ? 14 : 13} />
    </button>
  {/if}
  <button class="cap close" onclick={() => void win.close()} aria-label="Close" title="Close">
    <Icon name="x" size={17} />
  </button>
</div>

<style>
  .window-controls {
    position: fixed;
    top: 0;
    right: 0;
    z-index: 1000; /* over dialogs and toasts: the window stays closable */
    display: flex;
  }
  .cap {
    display: grid;
    place-items: center;
    width: 46px;
    height: var(--titlebar-h);
    padding: 0;
    border-radius: 0;
    color: var(--text-muted);
    transition:
      background var(--dur-micro) var(--ease-enter),
      color var(--dur-micro) var(--ease-enter);
  }
  /* Caption buttons do not do the global press-scale. */
  .cap:not(:disabled):active {
    transform: none;
  }
  .cap:hover,
  .cap.hover {
    background: color-mix(in srgb, var(--text) 10%, transparent);
    color: var(--text);
  }
  .cap:active,
  .cap.press {
    background: color-mix(in srgb, var(--text) 16%, transparent);
  }
  .cap.close:hover {
    background: var(--caption-close);
    color: var(--on-caption-close);
  }
  .cap.close:active {
    background: color-mix(in srgb, var(--caption-close) 80%, black);
    color: var(--on-caption-close);
  }
  .cap:focus-visible {
    outline-offset: -2px;
  }
</style>
