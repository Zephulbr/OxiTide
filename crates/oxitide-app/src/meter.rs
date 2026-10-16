//! Level-meter emitter for the Hi-Fi Rack (Track 9.5, RK-D7): while the UI
//! asks for it, one thread reads the engine's `LevelTap` ~30 times a second and
//! emits `level_frame`. Nothing runs while Rack is not visible.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use oxitide_audio::meter::LevelFrame;
use serde::Serialize;

pub const LEVEL_FRAME_EVENT: &str = "level_frame";
pub const TICK: Duration = Duration::from_millis(33);
/// Tick while the tap is silent (paused, between tracks): resuming audio
/// reaches the meters within this, at a third of the wake-ups.
pub const IDLE_TICK: Duration = Duration::from_millis(100);
/// Silent frames still sent after audio stops, so the UI's needles and bars
/// see the drop and settle (it stops animating once at rest).
const SILENT_FRAMES_SENT: u32 = 2;

/// Track 13 (EF-D4): decides which frames are worth an event.
#[derive(Debug, Default)]
pub struct SilenceGate {
    silent_run: u32,
}

impl SilenceGate {
    /// Whether to emit this frame.
    pub fn admit(&mut self, frame: &LevelFrame) -> bool {
        let silent = frame.peak == [0.0, 0.0] && frame.rms == [0.0, 0.0];
        if !silent {
            self.silent_run = 0;
            return true;
        }
        self.silent_run = self.silent_run.saturating_add(1);
        self.silent_run <= SILENT_FRAMES_SENT
    }

    /// True while frames are being suppressed.
    pub fn idle(&self) -> bool {
        self.silent_run > SILENT_FRAMES_SENT
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LevelFrameDto {
    pub peak: [f32; 2],
    pub rms: [f32; 2],
    pub bands: Vec<f32>,
}

impl From<LevelFrame> for LevelFrameDto {
    fn from(frame: LevelFrame) -> Self {
        Self { peak: frame.peak, rms: frame.rms, bands: frame.bands.to_vec() }
    }
}

struct Running {
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Start/stop state machine around one ticking thread. Both calls are
/// idempotent.
#[derive(Default)]
pub struct MeterEmitter {
    running: Mutex<Option<Running>>,
}

impl MeterEmitter {
    #[cfg(test)]
    pub fn is_running(&self) -> bool {
        self.running.lock().unwrap().is_some()
    }

    /// Runs `tick` until [`stop`](Self::stop), sleeping [`TICK`] after an
    /// active tick and [`IDLE_TICK`] after one that reports idle (`false`).
    pub fn start(&self, mut tick: impl FnMut() -> bool + Send + 'static) {
        let mut running = self.running.lock().unwrap();
        if running.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-meter".into())
            .spawn(move || {
                let mut sleep = TICK;
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(sleep);
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    sleep = if tick() { TICK } else { IDLE_TICK };
                }
            })
            .expect("spawn meter thread");
        *running = Some(Running { stop, handle });
    }

    pub fn stop(&self) {
        let taken = self.running.lock().unwrap().take();
        if let Some(running) = taken {
            running.stop.store(true, Ordering::SeqCst);
            let _ = running.handle.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn start_ticks_and_stop_halts() {
        let emitter = MeterEmitter::default();
        let ticks = Arc::new(AtomicUsize::new(0));
        let counter = ticks.clone();
        emitter.start(move || {
            counter.fetch_add(1, Ordering::SeqCst);
            true
        });
        assert!(emitter.is_running());
        std::thread::sleep(TICK * 5);
        emitter.stop();
        assert!(!emitter.is_running());
        let seen = ticks.load(Ordering::SeqCst);
        assert!(seen >= 1, "emitter never ticked");
        std::thread::sleep(TICK * 3);
        assert_eq!(ticks.load(Ordering::SeqCst), seen, "ticks after stop");
    }

    #[test]
    fn start_and_stop_are_idempotent() {
        let emitter = MeterEmitter::default();
        emitter.stop();
        let ticks = Arc::new(AtomicUsize::new(0));
        let a = ticks.clone();
        let b = ticks.clone();
        emitter.start(move || {
            a.fetch_add(1, Ordering::SeqCst);
            true
        });
        emitter.start(move || {
            b.fetch_add(100, Ordering::SeqCst);
            true
        });
        std::thread::sleep(TICK * 4);
        emitter.stop();
        emitter.stop();
        assert!(ticks.load(Ordering::SeqCst) < 100, "second start must not spawn a thread");
    }

    #[test]
    fn the_gate_sends_two_silent_frames_then_idles_until_audio() {
        let mut gate = SilenceGate::default();
        let silent = LevelFrame::silent();
        let mut loud = LevelFrame::silent();
        loud.peak = [0.5, 0.4];
        loud.rms = [0.2, 0.1];
        assert!(gate.admit(&loud));
        assert!(gate.admit(&silent), "first silent frame lets the meters fall");
        assert!(gate.admit(&silent));
        assert!(!gate.idle());
        assert!(!gate.admit(&silent));
        assert!(gate.idle());
        assert!(!gate.admit(&silent));
        assert!(gate.admit(&loud), "audio resumes at once");
        assert!(!gate.idle());
    }

    #[test]
    fn dto_carries_all_bands() {
        let dto = LevelFrameDto::from(LevelFrame::silent());
        assert_eq!(dto.bands.len(), oxitide_audio::meter::BANDS);
    }
}
