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

    /// Runs `tick` every [`TICK`] until [`stop`](Self::stop).
    pub fn start(&self, mut tick: impl FnMut() + Send + 'static) {
        let mut running = self.running.lock().unwrap();
        if running.is_some() {
            return;
        }
        let stop = Arc::new(AtomicBool::new(false));
        let flag = stop.clone();
        let handle = std::thread::Builder::new()
            .name("oxitide-meter".into())
            .spawn(move || {
                while !flag.load(Ordering::SeqCst) {
                    std::thread::sleep(TICK);
                    if flag.load(Ordering::SeqCst) {
                        break;
                    }
                    tick();
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
        });
        emitter.start(move || {
            b.fetch_add(100, Ordering::SeqCst);
        });
        std::thread::sleep(TICK * 4);
        emitter.stop();
        emitter.stop();
        assert!(ticks.load(Ordering::SeqCst) < 100, "second start must not spawn a thread");
    }

    #[test]
    fn dto_carries_all_bands() {
        let dto = LevelFrameDto::from(LevelFrame::silent());
        assert_eq!(dto.bands.len(), oxitide_audio::meter::BANDS);
    }
}
