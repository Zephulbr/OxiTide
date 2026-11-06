//! Position ticks skip the window while it cannot be seen (perf S12).
//!
//! A playing track sends a `position` event every 500 ms. Hidden to the tray
//! or minimized nothing shows them, yet each still crossed into WebView2 and
//! ran the page's listeners. While hidden the newest tick is held instead of
//! sent; it goes out first when the window shows again or when any other
//! event is sent (a `state`, `now_playing`, `error` … then lands on a current
//! position), so the UI is never behind once it can be seen or acted on.
//! CatJam's `beat` clock is held the same way: only the cat reads it, and the
//! newest clock is all it needs on return.
//!
//! The end-of-track sleep timer is the one hidden consumer: it watches the
//! ticks for the last seconds of a track. While it is armed the UI turns the
//! feed live (`set_position_feed_live`) and every event is sent as before.
//!
//! Visibility comes from `webview_idle` (Windows only); elsewhere the window
//! never counts as hidden and every tick is sent.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use crate::player::EngineEventDto;

pub const EVENT: &str = "playback_event";

/// The newest held event of each kind that waits while hidden.
#[derive(Default)]
struct Held {
    position: Option<EngineEventDto>,
    beat: Option<EngineEventDto>,
}

impl Held {
    fn send_all(&mut self, send: &mut impl FnMut(EngineEventDto)) {
        if let Some(tick) = self.position.take() {
            send(tick);
        }
        if let Some(clock) = self.beat.take() {
            send(clock);
        }
    }
}

/// Whether ticks should be held, and the ones waiting to go out.
pub struct PositionFeed {
    hidden: AtomicBool,
    live: AtomicBool,
    held: Mutex<Held>,
}

static FEED: PositionFeed = PositionFeed::new();

impl PositionFeed {
    pub const fn new() -> Self {
        Self {
            hidden: AtomicBool::new(false),
            live: AtomicBool::new(false),
            held: Mutex::new(Held { position: None, beat: None }),
        }
    }

    fn holding(&self) -> bool {
        self.hidden.load(Ordering::SeqCst) && !self.live.load(Ordering::SeqCst)
    }

    /// The events to send now, in order, for one engine event. Sending
    /// happens under the lock (`send`), so a held tick never overtakes a
    /// newer one.
    fn route(&self, event: EngineEventDto, mut send: impl FnMut(EngineEventDto)) {
        let mut held = self.held.lock().unwrap();
        if self.holding() {
            match event {
                EngineEventDto::Position { .. } => {
                    held.position = Some(event);
                    return;
                }
                EngineEventDto::Beat { .. } => {
                    held.beat = Some(event);
                    return;
                }
                _ => {}
            }
        }
        held.send_all(&mut send);
        send(event);
    }

    /// Send what is held if the window is visible (or the feed live) again.
    fn flush(&self, mut send: impl FnMut(EngineEventDto)) {
        let mut held = self.held.lock().unwrap();
        if !self.holding() {
            held.send_all(&mut send);
        }
    }
}

/// Forward one engine event to the desktop UI through the feed.
pub fn emit(app: &tauri::AppHandle, event: EngineEventDto) {
    FEED.route(event, |dto| crate::perf::emit_counted(app, EVENT, dto));
}

/// The main window went out of sight (`false`) or came back (`true`). Called
/// from the window proc, so the held tick is sent from another thread: an emit
/// on the main thread runs at once, inside whatever message is being handled.
pub fn set_window_visible(app: &tauri::AppHandle, visible: bool) {
    FEED.hidden.store(!visible, Ordering::SeqCst);
    if visible {
        spawn_flush(app);
    }
}

/// The UI needs every tick even while hidden (end-of-track sleep timer).
pub fn set_live(app: &tauri::AppHandle, live: bool) {
    FEED.live.store(live, Ordering::SeqCst);
    if live {
        spawn_flush(app);
    }
}

fn spawn_flush(app: &tauri::AppHandle) {
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("oxitide-position-flush".into())
        .spawn(move || FEED.flush(|dto| crate::perf::emit_counted(&app, EVENT, dto)));
    if let Err(e) = spawned {
        tracing::debug!("position flush thread: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tick(position_ms: u64) -> EngineEventDto {
        EngineEventDto::Position { position_ms, duration_ms: 200_000, lead_ms: 0 }
    }

    fn error() -> EngineEventDto {
        EngineEventDto::Error { code: "x".into(), message: "y".into() }
    }

    fn route(feed: &PositionFeed, event: EngineEventDto) -> Vec<EngineEventDto> {
        let mut sent = Vec::new();
        feed.route(event, |dto| sent.push(dto));
        sent
    }

    fn flush(feed: &PositionFeed) -> Vec<EngineEventDto> {
        let mut sent = Vec::new();
        feed.flush(|dto| sent.push(dto));
        sent
    }

    #[test]
    fn visible_sends_every_tick() {
        let feed = PositionFeed::new();
        assert_eq!(route(&feed, tick(500)), vec![tick(500)]);
        assert_eq!(route(&feed, tick(1000)), vec![tick(1000)]);
    }

    #[test]
    fn hidden_holds_only_the_newest_tick_and_sends_it_on_show() {
        let feed = PositionFeed::new();
        feed.hidden.store(true, Ordering::SeqCst);
        assert!(route(&feed, tick(500)).is_empty());
        assert!(route(&feed, tick(1000)).is_empty());
        assert!(flush(&feed).is_empty(), "still hidden");
        feed.hidden.store(false, Ordering::SeqCst);
        assert_eq!(flush(&feed), vec![tick(1000)]);
        assert!(flush(&feed).is_empty(), "sent once");
    }

    #[test]
    fn hidden_sends_other_events_after_the_held_tick() {
        let feed = PositionFeed::new();
        feed.hidden.store(true, Ordering::SeqCst);
        route(&feed, tick(1500));
        assert_eq!(route(&feed, error()), vec![tick(1500), error()]);
        assert_eq!(route(&feed, error()), vec![error()], "nothing held");
    }

    #[test]
    fn a_tick_after_show_sends_the_held_one_first() {
        let feed = PositionFeed::new();
        feed.hidden.store(true, Ordering::SeqCst);
        route(&feed, tick(1500));
        feed.hidden.store(false, Ordering::SeqCst);
        assert_eq!(route(&feed, tick(2000)), vec![tick(1500), tick(2000)]);
        assert!(flush(&feed).is_empty(), "the late flush finds nothing");
    }

    fn beat(anchor_ms: u64) -> EngineEventDto {
        EngineEventDto::Beat { track_id: 1, bpm: Some(120.0), period_ms: 500.0, anchor_ms, confidence: 0.9 }
    }

    #[test]
    fn hidden_holds_the_newest_beat_clock_too() {
        let feed = PositionFeed::new();
        feed.hidden.store(true, Ordering::SeqCst);
        assert!(route(&feed, beat(1000)).is_empty());
        assert!(route(&feed, tick(1500)).is_empty());
        assert!(route(&feed, beat(2000)).is_empty());
        assert_eq!(route(&feed, error()), vec![tick(1500), beat(2000), error()]);
        route(&feed, beat(3000));
        feed.hidden.store(false, Ordering::SeqCst);
        assert_eq!(flush(&feed), vec![beat(3000)]);
    }

    #[test]
    fn live_feed_sends_ticks_while_hidden() {
        let feed = PositionFeed::new();
        feed.hidden.store(true, Ordering::SeqCst);
        route(&feed, tick(500));
        feed.live.store(true, Ordering::SeqCst);
        assert_eq!(flush(&feed), vec![tick(500)]);
        assert_eq!(route(&feed, tick(1000)), vec![tick(1000)]);
    }
}
