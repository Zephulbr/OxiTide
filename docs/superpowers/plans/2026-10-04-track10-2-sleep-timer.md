# Track 10.2 Sleep Timer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A player-bar sleep timer that arms for 15/30/45/60/custom minutes or the end of the current track, shows a live countdown, and fades the output to silence over ~5 s before pausing — keeping the queue and position so play resumes where it stopped.

**Architecture:** The audio engine gains one command, `EngineCmd::FadeAndPause { duration_ms }`: the worker thread ramps the output volume to zero using the same `AudioOutput::set_volume` path `SetVolume` uses (endpoint volume in exclusive mode, sample scaling otherwise), then pauses, restores the pre-fade volume, and emits `State`. Every explicit transport or volume action cancels the fade. The timer itself is UI state only (`sleepTimer.svelte.ts`, in-memory per QoL-D4): a wall-clock interval fires the duration mode, and the existing 500 ms `Position` events drive the end-of-track mode (fire once when ≤5 s remain). A moon control in the player bar opens the option menu and hosts the live countdown.

**Tech Stack:** Rust (Tauri 2, `std::time::Instant`, `Mutex`), Svelte 5 runes, TypeScript, vitest fake timers.

**Spec:** `docs/superpowers/specs/2026-10-04-oxitide-track10-qol-design.md` (§5.2, §6.2, §7; decisions QoL-D4, QoL-D12)

**Spec refinements (flagged for review):**
1. The spec names play/pause as the only fade cancellation. The plan also cancels an active fade on `SetVolume`, `PlayTracks`/`PlayShuffled`, and `Clear` — a volume slider move or a deliberate new play must not be overridden by a dying timer, and a stale fade must never pause a later context.
2. The worker's output-reopen path uses the fade-aware `current_volume` helper instead of the raw user volume, so a track-format change mid-fade cannot blip to full volume.
3. End-of-track mode fires the full `FadeAndPause(5000)` at the first tick showing ≤5 s, exactly as the spec writes it. If the track ends before the ramp does, the pause lands ≤500 ms into the next track at ≤6% volume — the spec's accepted granularity.

## Global Constraints

- Windows-first; no CI. Every check is manual, run by the implementer.
- Gates after every Rust/UI task: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- Engine tests use `NullOutput` and fake resolvers/fetchers only — never a real device. This plan adds no HTTP.
- Every new `#[tauri::command]` must be listed in `generate_handler![...]` in `crates/oxitide-app/src/main.rs` or `invoke` fails at runtime.
- IPC: `invoke` sends camelCase args; Rust params stay snake_case (`durationMs` ↔ `duration_ms`).
- Icons are inlined Lucide paths in `ui/src/lib/components/Icon.svelte` — no unicode glyphs. Components consume semantic tokens only; Svelte 5 runes only.
- The sleep timer is in-memory: no config keys, no persistence, no new remote origins.
- Do not edit `AGENTS.md` in this plan — the track completion note owns AGENTS.md updates (spec §10).
- Commit steps follow the repo's plan convention. If the owner prefers to commit themselves, stop after each task's green run instead.

## Review Focus

- **A track transition during an active fade must not jump the output back to full volume.** The reopen path uses the fade-aware `current_volume` helper; pinned by `current_volume_reads_the_fade_target_while_fading` (Task 1) and smoke step 3.
- **`FadeAndPause` on an already-paused (or empty) queue must change nothing.** Pinned by `fade_and_pause_on_a_paused_queue_changes_nothing` and `fade_and_pause_with_zero_duration_pauses_immediately` (Task 1).
- **Explicit user actions end the fade: play/pause, volume, new context, clear.** Pinned by `play_pause_cancels_an_active_fade_and_restores_volume` and `set_volume_cancels_an_active_fade` (Task 1), and the `start_context`/`Clear` hooks are exercised by the existing suites.
- **End-of-track mode fires exactly once, only while playing, and only inside the last five seconds.** Pinned by the store tests `end-of-track fires ... and disarms`, `end-of-track ignores pauses and zero durations` (Task 3).
- **The duration countdown is wall-clock and disarms on fire, cancel, and queue end.** Pinned by the store tests `counts a duration timer down ...`, `cancel disarms ...`, `an empty queue disarms ...`, `a natural queue end disarms ...` (Task 3).

---

## File Structure

**Rust**

- Modify `crates/oxitide-audio/src/engine.rs` — `FadeState`, `Inner.fade`, `current_volume`, `cancel_fade`, `EngineCmd::FadeAndPause` + handler, ramp integration in `run_session`, cancel hooks, tests.
- Modify `crates/oxitide-app/src/commands.rs` — `fade_and_pause` command.
- Modify `crates/oxitide-app/src/main.rs` — register the command.

**UI**

- Modify `ui/src/lib/api.ts` — `fadeAndPause` wrapper.
- Create `ui/src/lib/sleepTimer.svelte.ts` — timer state machine (`SLEEP_FADE_MS`, `SleepTimer`, `createSleepTimer`).
- Create `ui/src/lib/sleepTimer.test.ts` — store tests with fake timers.
- Modify `ui/src/lib/components/Icon.svelte` — `moon` icon.
- Create `ui/src/lib/components/SleepTimerControl.svelte` — moon button, options menu, custom-minutes modal, countdown.
- Modify `ui/src/lib/components/PlayerBar.svelte` — mount the control.
- Modify `ui/src/App.svelte` — create the store, feed it player state, pass it to the bar.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-04-track10-2-sleep-timer-smoke.md` — human GUI checklist.

---

### Task 1: Engine `FadeAndPause`

**Files:**
- Modify: `crates/oxitide-audio/src/engine.rs` (`EngineCmd` ~line 62, `Inner` ~line 99, `Engine::new` ~line 202, `Engine` methods ~line 356, `handle` ~line 398, `run_session` ~line 1217, tests ~line 1482)

**Interfaces:**
- Consumes: `Queue::{is_playing, stop, set_volume}` (`queue.rs`), `Inner`, `AudioOutput`, `EngineEvent`.
- Produces: `EngineCmd::FadeAndPause { duration_ms: u64 }`; private `current_volume(inner) -> (f32, bool)` and `Engine::cancel_fade()` — consumed by Task 2.

- [ ] **Step 1: Write the failing tests**

Add to `crates/oxitide-audio/src/engine.rs` inside `mod tests` (after `restore_session_resumes_at_the_saved_position`, around line 2482):

```rust
    /// Review Focus: the sleep fade must ramp through intermediate volumes,
    /// reach silence, pause, and put the user's volume back.
    #[test]
    fn fade_and_pause_ramps_to_zero_then_pauses_and_restores_volume() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        let events = Arc::new(Mutex::new(Vec::new()));
        engine.subscribe(Arc::new({
            let events = events.clone();
            move |event| events.lock().unwrap().push(event)
        }));
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 150 }).unwrap();
        wait_for(|| !engine.state().playing);

        assert_eq!(engine.state().volume, 1.0, "pre-fade volume must be restored");
        wait_for(|| output.volumes().last() == Some(&1.0));
        let volumes = output.volumes();
        assert!(
            volumes.iter().any(|v| *v > 0.05 && *v < 0.95),
            "no intermediate ramp value: {volumes:?}"
        );
        assert!(
            volumes.iter().any(|v| v.abs() <= f32::EPSILON),
            "the ramp never reached silence: {volumes:?}"
        );
        assert_eq!(volumes.last(), Some(&1.0), "volume not restored after the fade");
        let done = events.lock().unwrap().iter().any(|event| {
            matches!(event, EngineEvent::State(state) if !state.playing && state.volume == 1.0)
        });
        assert!(done, "no completion State emitted");
    }

    /// Review Focus: play/pause during a fade cancels it and restores volume.
    #[test]
    fn play_pause_cancels_an_active_fade_and_restores_volume() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 60_000 }).unwrap();
        wait_for(|| output.volumes().iter().any(|v| *v < 0.95));

        engine.handle(EngineCmd::PlayPause).unwrap();
        assert!(!engine.state().playing);
        assert_eq!(engine.state().volume, 1.0);
        wait_for(|| output.volumes().last() == Some(&1.0));

        // Resuming must not inherit a leftover fade that pauses again.
        engine.handle(EngineCmd::PlayPause).unwrap();
        wait_for(|| engine.state().playing);
        std::thread::sleep(Duration::from_millis(300));
        assert!(engine.state().playing, "the cancelled fade must not pause later");
        assert_eq!(output.volumes().last(), Some(&1.0));
    }

    /// Review Focus: a volume change during a fade cancels it (the user wins).
    #[test]
    fn set_volume_cancels_an_active_fade() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 60_000 }).unwrap();
        wait_for(|| output.volumes().iter().any(|v| *v < 0.95));

        engine.handle(EngineCmd::SetVolume(0.25)).unwrap();
        wait_for(|| output.volumes().last() == Some(&0.25));
        std::thread::sleep(Duration::from_millis(300));
        assert_eq!(output.volumes().last(), Some(&0.25), "the fade kept ramping");
        assert!(engine.state().playing, "the fade must not pause after cancel");
    }

    /// Review Focus: FadeAndPause on an already-paused queue is inert.
    #[test]
    fn fade_and_pause_on_a_paused_queue_changes_nothing() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());
        engine.handle(EngineCmd::PlayPause).unwrap();
        assert!(!engine.state().playing);
        let before = output.volumes().len();

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 50 }).unwrap();
        std::thread::sleep(Duration::from_millis(200));

        let volumes = output.volumes();
        assert_eq!(volumes.len(), before, "a paused queue must not fade: {volumes:?}");
        assert_eq!(volumes.last(), Some(&1.0));
        assert!(!engine.state().playing);
    }

    /// Review Focus: a zero-duration fade pauses immediately, with no ramp.
    #[test]
    fn fade_and_pause_with_zero_duration_pauses_immediately() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 0 }).unwrap();
        wait_for(|| !engine.state().playing);
        assert!(
            output.volumes().iter().all(|v| *v == 1.0),
            "zero duration must not ramp: {:?}",
            output.volumes()
        );
    }

    /// Review Focus: the effective output volume follows the fade, not the user
    /// volume, and cancelling restores the user volume.
    #[test]
    fn current_volume_reads_the_fade_target_while_fading() {
        let (plan, files) = fixture_plan();
        let (engine, output) = engine_with(plan, files);
        engine.handle(EngineCmd::PlayTracks { items: vec![item(1)], start: 0 }).unwrap();
        engine.handle(EngineCmd::SetRepeat(RepeatMode::All)).unwrap();
        engine.handle(EngineCmd::SetVolume(0.8)).unwrap();
        wait_for(|| !output.samples().is_empty());

        engine.handle(EngineCmd::FadeAndPause { duration_ms: 60_000 }).unwrap();
        wait_for(|| current_volume(&engine.inner).0 < 0.8);
        let (volume, fading) = current_volume(&engine.inner);
        assert!(fading, "the fade must drive the output volume");
        assert!(volume < 0.8, "the fade target must win, got {volume}");

        engine.handle(EngineCmd::PlayPause).unwrap();
        let (volume, fading) = current_volume(&engine.inner);
        assert!(!fading, "play/pause must cancel the fade");
        assert!((volume - 0.8).abs() < f32::EPSILON, "user volume not restored: {volume}");
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p oxitide-audio fade_and_pause -v`
Expected: compile failure — no variant `FadeAndPause` (the `current_volume` test also fails: no function `current_volume`).

- [ ] **Step 3: Implement the engine changes**

In `crates/oxitide-audio/src/engine.rs`:

1. Add the variant to `EngineCmd` after `Clear` (line 87):

```rust
    Clear,
    /// Fade the output to silence over `duration_ms`, then pause at the current
    /// position. The pre-fade volume is restored for the next play; a
    /// play/pause or volume change cancels the fade.
    FadeAndPause { duration_ms: u64 },
    /// Restart the current track at its last position (after an error).
    Retry,
```

2. Add `FadeState` directly above `struct Inner` (line 99):

```rust
/// An in-flight sleep-timer fade: the worker ramps the output volume from
/// `base_volume` to zero over `duration`, then pauses and restores the volume.
struct FadeState {
    started: Instant,
    duration: Duration,
    base_volume: f32,
}
```

3. Add the field to `Inner` after `volume` (line 121):

```rust
    volume: AtomicU32, // f32 bits
    /// Active sleep-timer fade; the worker ramps the output volume to zero and
    /// then pauses.
    fade: Mutex<Option<FadeState>>,
```

4. Initialize it in `Engine::new` after `volume:` (line 219):

```rust
            volume: AtomicU32::new(1.0f32.to_bits()),
            fade: Mutex::new(None),
```

5. Add `current_volume` after `build_state` (line 171):

```rust
/// The volume the output should have right now and whether a sleep fade is
/// driving it. A fade overrides the user's volume until it completes or is
/// cancelled; the output-reopen path uses this too, so a format change
/// mid-fade never jumps back to full volume.
fn current_volume(inner: &Inner) -> (f32, bool) {
    let fade = inner.fade.lock().unwrap();
    match fade.as_ref() {
        Some(f) => {
            let duration = f.duration.as_secs_f32().max(0.001);
            let t = (f.started.elapsed().as_secs_f32() / duration).min(1.0);
            (f.base_volume * (1.0 - t), true)
        }
        None => (f32::from_bits(inner.volume.load(Ordering::SeqCst)), false),
    }
}
```

6. Add `cancel_fade` after `Engine::emit` (line 358):

```rust
    /// Cancel an in-flight sleep fade and restore the pre-fade volume. Called
    /// by every explicit transport or volume change so the user's action wins.
    fn cancel_fade(&self) {
        let base = self.inner.fade.lock().unwrap().take().map(|f| f.base_volume);
        if let Some(base) = base {
            self.inner.volume.store(base.to_bits(), Ordering::SeqCst);
            self.inner.queue.lock().unwrap().set_volume(base);
            self.inner.notify_worker();
        }
    }
```

7. Add the handler arm after the `Clear` arm (line 536), before `Retry`:

```rust
            EngineCmd::FadeAndPause { duration_ms } => {
                let playing = self.inner.queue.lock().unwrap().is_playing();
                if playing {
                    let base = f32::from_bits(self.inner.volume.load(Ordering::SeqCst));
                    if duration_ms == 0 || base <= 0.0 {
                        // Nothing to ramp: pause now.
                        self.inner.queue.lock().unwrap().stop();
                        self.inner.paused.store(true, Ordering::SeqCst);
                        self.inner.notify_worker();
                    } else {
                        *self.inner.fade.lock().unwrap() = Some(FadeState {
                            started: Instant::now(),
                            duration: Duration::from_millis(duration_ms),
                            base_volume: base,
                        });
                        self.inner.notify_worker();
                    }
                }
                self.emit(EngineEvent::State(self.state()));
            }
```

8. Cancel the fade in the explicit-action arms. Add `self.cancel_fade();` as the first line of each of:

- `EngineCmd::PlayPause => {` (line 432)
- `EngineCmd::SetVolume(v) => {` (line 498)
- `EngineCmd::Clear => {` (line 529)

For `Clear`, the arm then starts with:

```rust
            EngineCmd::Clear => {
                self.cancel_fade();
                *self.inner.stream_meta.lock().unwrap() = None;
```

For `PlayPause`, the arm then starts with:

```rust
            EngineCmd::PlayPause => {
                // An explicit transport action wins over a sleep fade.
                self.cancel_fade();
                // Only toggle when a track is current. On an empty queue
```

For `SetVolume`, the arm then starts with:

```rust
            EngineCmd::SetVolume(v) => {
                self.cancel_fade();
                let clamped = {
```

9. Do the same at the top of `start_context` (line 387):

```rust
    fn start_context(&self, mutate: impl FnOnce(&mut Queue)) {
        // Starting a new context is an explicit user action; end any fade.
        self.cancel_fade();
        *self.inner.stream_meta.lock().unwrap() = None;
```

10. Use the helper in the output-open path (lines 1199–1205). Replace:

```rust
            let volume = f32::from_bits(inner.volume.load(Ordering::SeqCst));
            if let Err(e) = output.set_volume(volume) {
```

with:

```rust
            // A format change mid-fade must not jump back to the user volume.
            let (volume, _) = current_volume(inner);
            if let Err(e) = output.set_volume(volume) {
```

11. Replace the inner-loop volume block (lines 1222–1232):

```rust
            let volume = f32::from_bits(inner.volume.load(Ordering::SeqCst));
            if (volume - last_volume).abs() >= f32::EPSILON {
                if let Err(e) = output.set_volume(volume) {
                    return SessionOutcome::OutputError {
                        message: e.to_string(),
                        track_id: stream.item.track_id,
                    };
                }
                last_volume = volume;
                *inner.output_status.lock().unwrap() = output.status();
            }
```

with:

```rust
            let (volume, fading) = current_volume(inner);
            // Fades step coarsely (endpoint volume calls are not free);
            // SetVolume changes are tracked exactly.
            let changed = if fading {
                volume <= f32::EPSILON || (volume - last_volume).abs() >= 0.01
            } else {
                (volume - last_volume).abs() >= f32::EPSILON
            };
            if changed {
                if let Err(e) = output.set_volume(volume) {
                    return SessionOutcome::OutputError {
                        message: e.to_string(),
                        track_id: stream.item.track_id,
                    };
                }
                last_volume = volume;
                *inner.output_status.lock().unwrap() = output.status();
            }
            if fading && volume <= f32::EPSILON {
                // The fade finished: pause here, keep queue and position, and
                // restore the user's volume so the next play is audible.
                let base = inner
                    .fade
                    .lock()
                    .unwrap()
                    .take()
                    .map(|f| f.base_volume)
                    .unwrap_or(volume);
                {
                    let mut q = inner.queue.lock().unwrap();
                    q.stop();
                    q.set_volume(base);
                }
                inner.volume.store(base.to_bits(), Ordering::SeqCst);
                inner.paused.store(true, Ordering::SeqCst);
                emit(inner, EngineEvent::State(build_state(inner)));
            }
```

- [ ] **Step 4: Run the engine tests to verify they pass**

Run: `cargo test -p oxitide-audio fade_and_pause; if ($?) { cargo test -p oxitide-audio current_volume }`
Expected: PASS (5 fade tests + 1 current_volume test).

- [ ] **Step 5: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-audio/src/engine.rs
git commit -m "feat(audio): add the sleep-timer fade-and-pause command"
```

---

### Task 2: `fade_and_pause` command

**Files:**
- Modify: `crates/oxitide-app/src/commands.rs` (after `set_volume`, line 1216)
- Modify: `crates/oxitide-app/src/main.rs` (register after `commands::set_volume,` line 350)

**Interfaces:**
- Consumes: `EngineCmd::FadeAndPause { duration_ms }` (Task 1).
- Produces (IPC): `fade_and_pause(duration_ms: u64)` — consumed by Task 4.

- [ ] **Step 1: Add the command**

In `crates/oxitide-app/src/commands.rs`, after `set_volume` (line 1216):

```rust
#[tauri::command]
pub fn fade_and_pause(state: State<'_, AppState>, duration_ms: u64) -> Result<(), String> {
    let player = state.player().ok_or_else(|| "not logged in".to_string())?;
    player
        .engine
        .handle(EngineCmd::FadeAndPause { duration_ms })
        .map_err(|e| e.to_string())
}
```

- [ ] **Step 2: Register it in `generate_handler!`**

In `crates/oxitide-app/src/main.rs`, after `commands::set_volume,` (line 350):

```rust
            commands::fade_and_pause,
```

- [ ] **Step 3: Run the full Rust gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }`
Expected: all green, including a `cargo check` of the new command (workspace tests compile the crate).

- [ ] **Step 4: Commit**

```bash
git add crates/oxitide-app/src/commands.rs crates/oxitide-app/src/main.rs
git commit -m "feat(playback): expose fade_and_pause to the UI"
```

---

### Task 3: Sleep-timer store

**Files:**
- Create: `ui/src/lib/sleepTimer.svelte.ts`
- Create: `ui/src/lib/sleepTimer.test.ts`

**Interfaces:**
- Consumes: nothing from earlier tasks (pure UI state).
- Produces: `SLEEP_FADE_MS: number`, `SleepTimerMode`, `SleepTimer` (`mode`, `totalMinutes`, `remainingMs`, `armMinutes(minutes)`, `armEndOfTrack()`, `cancel()`, `observe(positionMs, durationMs, playing, queueLength)`), `createSleepTimer(fire: (fadeMs: number) => void)` — consumed by Task 4.

- [ ] **Step 1: Write the failing tests**

Create `ui/src/lib/sleepTimer.test.ts`:

```ts
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createSleepTimer, SLEEP_FADE_MS } from "./sleepTimer.svelte";

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date("2026-10-04T12:00:00Z"));
});

afterEach(() => {
  vi.useRealTimers();
});

function setup() {
  const fires: number[] = [];
  const timer = createSleepTimer((fadeMs) => fires.push(fadeMs));
  return { timer, fires };
}

describe("sleep timer", () => {
  it("counts a duration timer down and fires the fade once at zero", () => {
    const { timer, fires } = setup();
    timer.armMinutes(15);
    expect(timer.mode).toBe("duration");
    expect(timer.totalMinutes).toBe(15);
    expect(timer.remainingMs).toBe(15 * 60_000);

    vi.advanceTimersByTime(60_000);
    expect(timer.remainingMs).toBe(14 * 60_000);
    expect(fires).toEqual([]);

    vi.advanceTimersByTime(14 * 60_000 + 1_000);
    expect(fires).toEqual([SLEEP_FADE_MS]);
    expect(timer.mode).toBe("off");
    expect(timer.remainingMs).toBeNull();
  });

  it("cancel disarms a running timer without firing", () => {
    const { timer, fires } = setup();
    timer.armMinutes(1);
    vi.advanceTimersByTime(30_000);
    timer.cancel();
    vi.advanceTimersByTime(120_000);
    expect(fires).toEqual([]);
    expect(timer.mode).toBe("off");
  });

  it("end-of-track fires the fade when five seconds remain and disarms", () => {
    const { timer, fires } = setup();
    timer.armEndOfTrack();
    timer.observe(54_600, 60_000, true, 1);
    expect(fires).toEqual([]);
    timer.observe(55_200, 60_000, true, 1);
    expect(fires).toEqual([SLEEP_FADE_MS]);
    expect(timer.mode).toBe("off");
    timer.observe(55_800, 60_000, true, 1);
    expect(fires).toEqual([SLEEP_FADE_MS]);
  });

  it("end-of-track ignores pauses and zero durations", () => {
    const { timer, fires } = setup();
    timer.armEndOfTrack();
    timer.observe(59_000, 60_000, false, 1);
    timer.observe(0, 0, true, 1);
    expect(fires).toEqual([]);
  });

  it("an empty queue disarms the timer", () => {
    const { timer, fires } = setup();
    timer.armMinutes(30);
    timer.observe(0, 0, false, 0);
    expect(timer.mode).toBe("off");
    vi.advanceTimersByTime(31 * 60_000);
    expect(fires).toEqual([]);
  });

  it("a natural queue end disarms the timer", () => {
    const { timer, fires } = setup();
    timer.armMinutes(30);
    timer.observe(60_000, 60_000, false, 3);
    expect(timer.mode).toBe("off");
    vi.advanceTimersByTime(31 * 60_000);
    expect(fires).toEqual([]);
  });

  it("ignores non-positive custom durations", () => {
    const { timer } = setup();
    timer.armMinutes(0);
    expect(timer.mode).toBe("off");
    timer.armMinutes(-5);
    expect(timer.mode).toBe("off");
  });

  it("re-arming replaces the previous duration", () => {
    const { timer, fires } = setup();
    timer.armMinutes(60);
    vi.advanceTimersByTime(10_000);
    timer.armMinutes(15);
    expect(timer.totalMinutes).toBe(15);
    vi.advanceTimersByTime(14 * 60_000 + 1_000);
    expect(fires).toEqual([SLEEP_FADE_MS]);
  });

  it("end-of-track reports no countdown", () => {
    const { timer } = setup();
    timer.armEndOfTrack();
    expect(timer.remainingMs).toBeNull();
    expect(timer.totalMinutes).toBeNull();
  });
});
```

- [ ] **Step 2: Run them to verify they fail**

Run: `npm --prefix ui test -- src/lib/sleepTimer.test.ts`
Expected: FAIL — cannot resolve `./sleepTimer.svelte`.

- [ ] **Step 3: Write the store**

Create `ui/src/lib/sleepTimer.svelte.ts`:

```ts
/// How long the sleep timer fades before pausing (spec §6.2).
export const SLEEP_FADE_MS = 5000;

export type SleepTimerMode = "off" | "duration" | "end-of-track";

export interface SleepTimer {
  readonly mode: SleepTimerMode;
  /// The armed duration in minutes (menu selection state); null otherwise.
  readonly totalMinutes: number | null;
  /// Milliseconds left on a duration timer; null for other modes.
  readonly remainingMs: number | null;
  armMinutes(minutes: number): void;
  armEndOfTrack(): void;
  cancel(): void;
  /// Fed from the player's 500 ms position ticks; fires end-of-track mode when
  /// five seconds remain, and disarms when the queue is over.
  observe(positionMs: number, durationMs: number, playing: boolean, queueLength: number): void;
}

/// The sleep timer is in-memory only (QoL-D4): a restart disarms it. `fire`
/// is called exactly once per armed timer with the fade length.
export function createSleepTimer(fire: (fadeMs: number) => void): SleepTimer {
  let mode = $state<SleepTimerMode>("off");
  let totalMinutes = $state<number | null>(null);
  let nowMs = $state(Date.now());
  let endsAt = 0;
  let ticker: ReturnType<typeof setInterval> | null = null;

  function stopTicker() {
    if (ticker !== null) {
      clearInterval(ticker);
      ticker = null;
    }
  }

  function fireOnce() {
    if (mode === "off") return;
    mode = "off";
    totalMinutes = null;
    stopTicker();
    fire(SLEEP_FADE_MS);
  }

  function armMinutes(minutes: number) {
    const rounded = Math.round(minutes);
    if (!Number.isFinite(rounded) || rounded < 1) return;
    totalMinutes = rounded;
    nowMs = Date.now();
    endsAt = nowMs + rounded * 60_000;
    mode = "duration";
    stopTicker();
    ticker = setInterval(() => {
      nowMs = Date.now();
      if (mode === "duration" && nowMs >= endsAt) fireOnce();
    }, 500);
  }

  function armEndOfTrack() {
    stopTicker();
    totalMinutes = null;
    mode = "end-of-track";
  }

  function cancel() {
    stopTicker();
    mode = "off";
    totalMinutes = null;
  }

  function observe(
    positionMs: number,
    durationMs: number,
    playing: boolean,
    queueLength: number,
  ) {
    if (mode === "off") return;
    if (queueLength === 0) {
      cancel();
      return;
    }
    if (!playing && durationMs > 0 && positionMs >= durationMs) {
      // The queue reached its natural end; nothing left to sleep through.
      cancel();
      return;
    }
    if (mode !== "end-of-track" || !playing || durationMs === 0) return;
    const remaining = durationMs - positionMs;
    if (remaining > 0 && remaining <= SLEEP_FADE_MS) fireOnce();
  }

  return {
    get mode() {
      return mode;
    },
    get totalMinutes() {
      return totalMinutes;
    },
    get remainingMs() {
      return mode === "duration" ? Math.max(0, endsAt - nowMs) : null;
    },
    armMinutes,
    armEndOfTrack,
    cancel,
    observe,
  };
}
```

- [ ] **Step 4: Run the store tests to verify they pass**

Run: `npm --prefix ui test -- src/lib/sleepTimer.test.ts`
Expected: PASS (9 tests).

- [ ] **Step 5: Run the UI gates**

Run: `npm --prefix ui run check; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 6: Commit**

```bash
git add ui/src/lib/sleepTimer.svelte.ts ui/src/lib/sleepTimer.test.ts
git commit -m "feat(ui): add the sleep-timer state machine"
```

---

### Task 4: Moon control and wiring

**Files:**
- Modify: `ui/src/lib/api.ts` (after `setVolume`, line 315)
- Modify: `ui/src/lib/components/Icon.svelte` (after `"bell"`, line 50)
- Create: `ui/src/lib/components/SleepTimerControl.svelte`
- Modify: `ui/src/lib/components/PlayerBar.svelte` (props line 15-27, imports, right zone line 104)
- Modify: `ui/src/App.svelte` (imports, store creation ~line 67, effect ~line 181, PlayerBar call line 548)

**Interfaces:**
- Consumes: `SLEEP_FADE_MS`/`createSleepTimer`/`SleepTimer` (Task 3); IPC `fade_and_pause` (Task 2).
- Produces: the player-bar moon control.

- [ ] **Step 1: Add the IPC wrapper**

In `ui/src/lib/api.ts`, after `setVolume` (line 315):

```ts
  fadeAndPause: (durationMs: number) => invoke<void>("fade_and_pause", { durationMs }),
```

- [ ] **Step 2: Add the moon icon**

In `ui/src/lib/components/Icon.svelte`, after the `"bell"` entry (line 50):

```ts
    "moon": '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z" />',
```

- [ ] **Step 3: Create the control**

Create `ui/src/lib/components/SleepTimerControl.svelte`:

```svelte
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
```

- [ ] **Step 4: Mount it in the player bar**

In `ui/src/lib/components/PlayerBar.svelte`:

1. Add the imports after `VolumeSlider.svelte` (line 13):

```ts
  import SleepTimerControl from "./SleepTimerControl.svelte";
  import type { SleepTimer } from "../sleepTimer.svelte";
```

2. Add the prop to the destructure (line 15-27):

```ts
  let {
    player,
    qualityDisplay,
    favorites,
    nav,
    mini,
    sleepTimer,
  }: {
    player: Player;
    qualityDisplay: QualityDisplay;
    favorites: Favorites;
    nav: Nav;
    mini: Mini;
    sleepTimer: SleepTimer;
  } = $props();
```

3. Mount it in the right zone after the queue button (line 104):

```svelte
      ><Icon name="list-music" size={18} /></button>

      <SleepTimerControl {sleepTimer} />

      <VolumeSlider {player} />
```

- [ ] **Step 5: Wire the store in App.svelte**

In `ui/src/App.svelte`:

1. Add the import next to `createPlayer` (line 43):

```ts
  import { createSleepTimer } from "./lib/sleepTimer.svelte";
```

2. Create the store after `const player = createPlayer(toasts);` (line 67):

```ts
  const sleepTimer = createSleepTimer((fadeMs) => {
    api.fadeAndPause(fadeMs).catch((e) => toasts.show(errorMessage(e), { severity: "error" }));
  });
```

3. Add the observation effect after the `player.init()` effect (line 176-181):

```ts
  // The sleep timer rides the same 500 ms position ticks as the seek bar;
  // end-of-track mode fires once the last five seconds begin.
  $effect(() => {
    sleepTimer.observe(
      player.state.position_ms,
      player.state.duration_ms,
      player.state.playing,
      player.state.queue.length,
    );
  });
```

4. Pass it to the bar (line 548):

```svelte
    <PlayerBar {player} {qualityDisplay} {favorites} {nav} {mini} {sleepTimer} />
```

- [ ] **Step 6: Run the UI gates**

Run: `npm --prefix ui run check; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 7: Run the full Windows gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 8: Commit**

```bash
git add ui/src/lib/api.ts ui/src/lib/components/Icon.svelte ui/src/lib/components/SleepTimerControl.svelte ui/src/lib/components/PlayerBar.svelte ui/src/App.svelte
git commit -m "feat(ui): add the player-bar sleep timer control"
```

---

### Task 5: Smoke checklist and verification

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-04-track10-2-sleep-timer-smoke.md`

**Interfaces:**
- Consumes: everything above.
- Produces: the human verification record for this plan.

- [ ] **Step 1: Write the checklist**

Create `docs/superpowers/plans/notes/2026-10-04-track10-2-sleep-timer-smoke.md`:

```markdown
# Track 10.2 sleep-timer smoke checklist (2026-10-04)

Human check with a real account and audio. Run `npm --prefix ui run app:dev`
(Vite + Tauri). Use the custom option with 1 minute for the expiry check.
Mark each item pass/fail with a note; failures become plan fixes, not silent
exceptions.

1. Duration fade
   - Play a track. Moon menu → Custom → 1 minute.
   - PASS = the moon shows a live countdown (1:00, 0:59, …); at 0:00 the volume
     ramps down over ~5 s, audio pauses at silence, and the volume slider is
     back at its prior value.
2. Resume
   - Press play after the fade. PASS = playback resumes at the paused position
     at full volume.
3. End of track
   - Pick a track with more than ~10 s left, arm End of current track, and seek
     to ~6 s remaining. PASS = the volume fades over the last ~5 s and playback
     pauses at/near the end; no audible sliver of the next track. Pressing play
     continues normally.
4. Off / cancel
   - Arm 15 minutes, then choose Off. PASS = the countdown clears and no fade
     happens later.
5. Queue clear
   - Arm 15 minutes, clear the queue. PASS = the moon returns to off.
6. Manual pause during the fade
   - Arm 1 minute and wait for the ramp to start, then press pause.
     PASS = volume returns to the prior level and playback stays paused.
7. Volume during the fade
   - Arm 1 minute and wait for the ramp to start, then move the volume slider.
     PASS = the fade stops, the chosen volume takes effect, and playback keeps
     playing.
8. Restart resets
   - Arm a timer, quit and relaunch. PASS = the timer is off.
9. Exclusive mode (only if configured)
   - Repeat 1 with exclusive output. PASS = same behavior, and the system
     volume is back to normal afterward.

Record results below.

- [ ] 1 duration fade
- [ ] 2 resume
- [ ] 3 end of track
- [ ] 4 off
- [ ] 5 queue clear
- [ ] 6 pause cancels
- [ ] 7 volume cancels
- [ ] 8 restart resets
- [ ] 9 exclusive
```

- [ ] **Step 2: Run the final gates**

Run: `cargo test --workspace; if ($?) { cargo clippy --workspace -- -D warnings }; if ($?) { npm --prefix ui run check }; if ($?) { npm --prefix ui test }`
Expected: all green.

- [ ] **Step 3: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-04-track10-2-sleep-timer-smoke.md
git commit -m "docs: add the Track 10.2 sleep-timer smoke checklist"
```

The human runs the checklist separately (AGENTS.md: GUI smoke checks require a
human, a real account, and audio). Failures become plan fixes, not silent
exceptions.

---

## Self-Review

- **Spec coverage (spec §5.2, §6.2, §7; QoL-D4, QoL-D12):**
  - `EngineCmd::FadeAndPause { duration_ms }` ramps on the same `AudioOutput::set_volume` path as `SetVolume` (endpoint volume in exclusive / sample scaling otherwise), pauses, restores the pre-fade volume, emits `State` → Task 1.
  - Play/pause during a fade cancels it and restores volume → Task 1 (plus volume/context/clear cancellation, declared as a refinement).
  - Moon control with Off / 15 / 30 / 45 / 60 / custom / End of current track and a live countdown → Tasks 3 + 4.
  - Duration mode fires `FadeAndPause(5000)` → Tasks 2 + 3 (`SLEEP_FADE_MS`).
  - End-of-track mode fires when 500 ms ticks show ≤5 s remaining → Task 3 `observe` + Task 4 effect.
  - In-memory, resets on restart, disarms after firing / queue end → Task 3 (no persistence anywhere).
  - Pause keeps queue and position → Task 1 (`q.stop()` leaves items/current/position; no generation bump).
  - Engine tests for `FadeAndPause`; UI tests for countdown/end-of-track firing; smoke checklist for the fade → Tasks 1, 3, 5.
- **Placeholder scan:** no TBD/TODO; every code step carries complete code.
- **Type consistency:** `FadeAndPause { duration_ms: u64 }` is identical in Tasks 1 and 2; `durationMs` → `duration_ms` IPC mapping matches the AGENTS.md contract; `createSleepTimer` / `SleepTimer` / `SLEEP_FADE_MS` names are identical in Tasks 3 and 4; `sleepTimer` is the prop name in both `PlayerBar` and `App`.
- **Review Focus:** each line names its pinning test or step; the five lines cover the uncovered input classes (mid-fade track transitions, paused/empty queue, user-action cancellation, tick-driven end-of-track edge cases, wall-clock countdown lifecycle).
