<script lang="ts">
  import { listen } from "@tauri-apps/api/event";
  import { untrack } from "svelte";
  import { api, type BeatClock, type CatjamCoat, type EngineEvent } from "../api";
  import { beatIndex, beatPhase, createPositionClock } from "../beatClock";
  import { CATJAM_SIZE, FRAMES, IDLE_INDEX, bobFrame, renderSheet, swayFrame } from "../catjamSprite";
  import type { Player } from "../player.svelte";

  /// CatJam (spec 2026-10-21-oxitide-catjam-kuroneko-design.md §5.6): a cat in
  /// the player bar, beside the track title, that bobs to the beat. Beats come from the engine's
  /// local analysis, which runs only while this is mounted. Per-frame work is a
  /// direct style write when the frame changes; no loop while paused, hidden,
  /// or without a beat clock.
  let { player, coat, scale = 100 }: { player: Player; coat: CatjamCoat; scale?: number } = $props();

  let el: HTMLDivElement | undefined = $state();
  const sheet = $derived(renderSheet(coat));
  /// Drawn size of one frame, in CSS px.
  const px = $derived((CATJAM_SIZE * scale) / 100);

  const position = createPositionClock();
  let beat: BeatClock | null = null;
  let raf = 0;
  let shown = -1;

  function currentTrack(): number | null {
    return player.state.current?.track_id ?? null;
  }

  function clockForCurrent(): BeatClock | null {
    return beat !== null && beat.track_id === currentTrack() ? beat : null;
  }

  function frameAt(now: number): number {
    const clock = clockForCurrent();
    const heard = position.heardAt(now);
    if (clock === null || heard === null) return IDLE_INDEX;
    if (clock.bpm === null) return swayFrame(now);
    return bobFrame(beatIndex(clock, heard), beatPhase(clock, heard));
  }

  function draw(now: number) {
    const frame = frameAt(now);
    if (frame === shown || !el) return;
    shown = frame;
    el.style.backgroundPosition = `${-frame * px}px 0`;
  }

  function wanted(): boolean {
    return player.state.playing && !document.hidden && clockForCurrent() !== null;
  }

  function tick(now: number) {
    raf = 0;
    draw(now);
    if (wanted()) raf = requestAnimationFrame(tick);
  }

  /// Start the loop when it should run, stop it when it should not; either
  /// way the frame is brought up to date once.
  function schedule() {
    draw(performance.now());
    if (wanted()) {
      if (raf === 0) raf = requestAnimationFrame(tick);
    } else if (raf !== 0) {
      cancelAnimationFrame(raf);
      raf = 0;
    }
  }

  function onEvent(event: EngineEvent) {
    if (event.type === "position") {
      position.sample(event.position_ms, event.lead_ms, performance.now());
    } else if (event.type === "beat") {
      // Kept even if the player has not named this track yet (events from
      // one advance can arrive in either order); `clockForCurrent` filters.
      beat = {
        track_id: event.track_id,
        bpm: event.bpm,
        period_ms: event.period_ms,
        anchor_ms: event.anchor_ms,
        confidence: event.confidence,
      };
      schedule();
    }
  }

  $effect(() => {
    let disposed = false;
    let unlisten: (() => void) | null = null;
    untrack(() => position.sample(player.state.position_ms, 0, performance.now(), true));
    void listen<EngineEvent>("playback_event", (e) => onEvent(e.payload)).then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
    void api.setBeatTracking(true).catch(() => {});
    const onVisibility = () => schedule();
    document.addEventListener("visibilitychange", onVisibility);
    return () => {
      disposed = true;
      unlisten?.();
      document.removeEventListener("visibilitychange", onVisibility);
      if (raf !== 0) cancelAnimationFrame(raf);
      raf = 0;
      void api.setBeatTracking(false).catch(() => {});
    };
  });

  $effect(() => {
    const playing = player.state.playing;
    currentTrack();
    untrack(() => {
      position.setPlaying(playing, performance.now());
      schedule();
    });
  });

  $effect(() => {
    if (!el) return;
    const size = px;
    el.style.backgroundImage = sheet ? `url(${sheet})` : "";
    el.style.width = `${size}px`;
    el.style.height = `${size}px`;
    el.style.backgroundSize = `${FRAMES.length * size}px ${size}px`;
    shown = -1;
    untrack(() => draw(performance.now()));
  });
</script>

<!-- `data-floor`: the player bar's hover light rises under it like under a
     control. -->
<div class="catjam" bind:this={el} aria-hidden="true" data-floor></div>

<style>
  /* In the player bar's left zone, between the title and the heart; purely
     decorative: it takes hover (for the bar's light) but has no click. */
  .catjam {
    flex: 0 0 auto;
    align-self: center;
    background-repeat: no-repeat;
    image-rendering: pixelated;
  }
</style>
