//! Autoplay (plan `2026-10-26-autoplay.md`): while the last queued track
//! plays, append similar tracks so playback carries on when the queue ends.
//!
//! An engine subscriber, so it works with the window hidden to the tray or
//! only the phone remote in use. Suggestions come from TIDAL's track radio
//! of the last track (artist radio when that is empty), skip what is queued,
//! already suggested or recently heard, and pass the content filters.
//! Appended items carry `QueueItem::autoplay`.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use oxitide_audio::engine::{EngineCmd, EngineEvent, PlaybackState};
use oxitide_audio::queue::{QueueItem, RepeatMode};
use oxitide_tidal::client::TidalClient;
use oxitide_tidal::models::Track;
use tauri::{AppHandle, Manager};

use crate::state::AppState;

/// Tracks appended per fill; the rest of a radio waits in the pool.
pub const BATCH: usize = 10;
/// Recent plays that rank below unheard suggestions.
const RECENT_PLAYS: usize = 100;
/// A failed fill is tried again on a later state after this long.
const RETRY_AFTER: Duration = Duration::from_secs(60);
/// How long a fill that failed offline waits for the network.
const OFFLINE_WAIT: Duration = Duration::from_secs(600);

#[derive(Default)]
pub struct Autoplay {
    enabled: AtomicBool,
    run: Mutex<Run>,
}

/// One stretch of autoplay, from the user's last track on.
#[derive(Default)]
struct Run {
    /// Suggestions fetched but not queued yet, best first.
    pool: Vec<QueueItem>,
    /// Every track suggested in this run (queued or pooled).
    picked: HashSet<u64>,
    /// The (track, queue revision) the last fill started for, so the burst
    /// of state events around one track asks once.
    attempted: Option<(u64, u64)>,
    in_flight: bool,
    failed_at: Option<Instant>,
}

impl Run {
    fn may_start(&self, key: (u64, u64)) -> bool {
        if self.in_flight {
            return false;
        }
        self.attempted != Some(key) || self.failed_at.is_some_and(|at| at.elapsed() >= RETRY_AFTER)
    }
}

/// The last queue item, when it is current and nothing follows it, and the
/// queue is playing or has just played out. Repeat All/One never autoplay.
pub fn fill_target(state: &PlaybackState) -> Option<&QueueItem> {
    if state.repeat != RepeatMode::Off || !(state.playing || state.ended) {
        return None;
    }
    let last = state.queue.len().checked_sub(1)?;
    if state.current_index != Some(last) {
        return None;
    }
    state.queue.get(last)
}

/// Radio tracks as autoplay items: the seed, queued, already suggested and
/// duplicate tracks dropped; recently heard ones moved after the rest.
fn candidates(
    tracks: &[Track],
    seed: u64,
    queued: &HashSet<u64>,
    picked: &HashSet<u64>,
    recent: &HashSet<u64>,
) -> Vec<QueueItem> {
    let mut seen = HashSet::new();
    let (fresh, heard): (Vec<&Track>, Vec<&Track>) = tracks
        .iter()
        .filter(|t| t.id != seed && !queued.contains(&t.id) && !picked.contains(&t.id) && seen.insert(t.id))
        .partition(|t| !recent.contains(&t.id));
    fresh
        .into_iter()
        .chain(heard)
        .map(|t| QueueItem { autoplay: true, ..crate::commands::queue_item_from_track(t) })
        .collect()
}

/// Up to `BATCH` pooled items that are not queued meanwhile; the pool keeps
/// the rest.
fn take_batch(pool: &mut Vec<QueueItem>, queued: &HashSet<u64>) -> Vec<QueueItem> {
    pool.retain(|item| !queued.contains(&item.track_id));
    let n = pool.len().min(BATCH);
    pool.drain(..n).collect()
}

/// Queue indices of the autoplay items after the current one, highest
/// first, so removing them in order keeps the rest valid.
pub fn prune_indices(queue: &[QueueItem], current: Option<usize>) -> Vec<usize> {
    let from = current.map_or(0, |c| c + 1);
    (from..queue.len()).rev().filter(|&i| queue[i].autoplay).collect()
}

/// Track radio of `seed`, else artist radio of its first credited artist.
async fn radio_for(client: &TidalClient, seed: &QueueItem) -> Result<Vec<Track>, oxitide_tidal::Error> {
    let first = client.track_radio(seed.track_id).await;
    match first {
        Ok(tracks) if !tracks.is_empty() => return Ok(tracks),
        Ok(_) => {}
        Err(ref e) => tracing::debug!("autoplay: track radio for {} failed: {e}", seed.track_id),
    }
    match seed.artist_refs.first() {
        Some(artist) => client.artist_radio(artist.id).await,
        None => first,
    }
}

impl Autoplay {
    pub fn set_enabled(&self, enabled: bool) {
        self.enabled.store(enabled, Ordering::SeqCst);
        if !enabled {
            *self.run.lock().unwrap() = Run::default();
        }
    }

    pub fn enabled(&self) -> bool {
        self.enabled.load(Ordering::SeqCst)
    }

    /// The engine subscriber. Returns at once unless the last track is
    /// current; the fetch runs on the async runtime.
    pub fn on_event(&self, event: &EngineEvent, handle: &AppHandle) {
        if let EngineEvent::State(state) = event {
            self.consider(state, handle);
        }
    }

    /// Evaluate `state` now (the toggle turning on, the network returning).
    pub fn consider(&self, state: &PlaybackState, handle: &AppHandle) {
        if !self.enabled() {
            return;
        }
        let Some(target) = fill_target(state) else {
            return;
        };
        let key = (target.track_id, state.queue_rev);
        {
            let mut run = self.run.lock().unwrap();
            if !run.may_start(key) {
                return;
            }
            if !target.autoplay {
                // The user's own track is last: a new run seeds from it.
                run.pool.clear();
                run.picked.clear();
            }
            run.attempted = Some(key);
            run.in_flight = true;
        }
        let target = target.clone();
        let queued: HashSet<u64> = state.queue.iter().map(|item| item.track_id).collect();
        let handle = handle.clone();
        tauri::async_runtime::spawn(async move {
            let app = handle.state::<AppState>();
            app.autoplay.fill(&app, &handle, target, queued).await;
        });
    }

    async fn fill(&self, app: &AppState, handle: &AppHandle, target: QueueItem, queued: HashSet<u64>) {
        let result = self.suggest(app, &target, &queued).await;
        let items = {
            let mut run = self.run.lock().unwrap();
            run.in_flight = false;
            match result {
                Ok(items) if !items.is_empty() => {
                    run.failed_at = None;
                    items
                }
                Ok(_) => {
                    tracing::info!("autoplay: no suggestions after track {}", target.track_id);
                    run.failed_at = Some(Instant::now());
                    return;
                }
                Err(e) => {
                    tracing::warn!("autoplay: fill after track {} failed: {e}", target.track_id);
                    run.failed_at = Some(Instant::now());
                    drop(run);
                    if oxitide_audio::connectivity::global().is_offline() {
                        retry_when_online(handle.clone());
                    }
                    return;
                }
            }
        };
        if !self.enabled() {
            return;
        }
        let Some(player) = app.player() else {
            return;
        };
        let now = player.engine.state();
        // The user may have moved on (a new queue, a skip, repeat) while
        // the radio loaded: append only after the same last track.
        if fill_target(&now).map(|item| item.track_id) != Some(target.track_id) {
            return;
        }
        tracing::info!("autoplay: queued {} tracks after track {}", items.len(), target.track_id);
        let _ = player.engine.handle(EngineCmd::Enqueue { items });
        if now.ended {
            // The queue played out while the radio loaded: carry on.
            let _ = player.engine.handle(EngineCmd::Next);
        }
    }

    async fn suggest(
        &self,
        app: &AppState,
        target: &QueueItem,
        queued: &HashSet<u64>,
    ) -> Result<Vec<QueueItem>, String> {
        {
            let mut run = self.run.lock().unwrap();
            let batch = take_batch(&mut run.pool, queued);
            if !batch.is_empty() {
                return Ok(batch);
            }
        }
        let client = app
            .client
            .read()
            .map_err(|_| "state poisoned".to_string())?
            .clone()
            .ok_or_else(|| "not logged in".to_string())?;
        let tracks = radio_for(&client, target).await.map_err(|e| app.fail_tidal(e))?;
        let flags = app.track_flags();
        if let Some(store) = &flags {
            for track in &tracks {
                store.record_explicit(track.id, track.explicit);
            }
        }
        let recent: HashSet<u64> = match app.history() {
            Some(history) => tauri::async_runtime::spawn_blocking(move || {
                history.page(0, RECENT_PLAYS).0.into_iter().map(|r| r.track_id).collect()
            })
            .await
            .unwrap_or_default(),
            None => HashSet::new(),
        };
        let picked = self.run.lock().unwrap().picked.clone();
        let items = candidates(&tracks, target.track_id, queued, &picked, &recent);
        let ids: Vec<u64> = items.iter().map(|item| item.track_id).collect();
        let content = app.config().content;
        let (mut kept, _) = crate::commands::filter_playable_items(&client, &ids, items, flags.as_deref(), &content)
            .await
            .map_err(|e| app.fail_tidal(e))?;
        let mut run = self.run.lock().unwrap();
        run.picked.extend(kept.iter().map(|item| item.track_id));
        let rest = kept.split_off(kept.len().min(BATCH));
        run.pool = rest;
        Ok(kept)
    }

    /// Turning Autoplay off drops its upcoming suggestions; the current
    /// track keeps playing.
    pub fn prune(&self, engine: &oxitide_audio::engine::Engine) {
        let state = engine.state();
        for index in prune_indices(&state.queue, state.current_index) {
            let _ = engine.handle(EngineCmd::Remove(index));
        }
    }
}

/// Wait (off the runtime) for the network, then try the fill again.
fn retry_when_online(handle: AppHandle) {
    let spawned = std::thread::Builder::new().name("oxitide-autoplay".into()).spawn(move || {
        let conn = oxitide_audio::connectivity::global();
        let started = Instant::now();
        while conn.is_offline() && started.elapsed() < OFFLINE_WAIT {
            conn.wait_change(Duration::from_secs(30));
        }
        if conn.is_offline() {
            return;
        }
        let app = handle.state::<AppState>();
        if let Some(player) = app.player() {
            app.autoplay.run.lock().unwrap().attempted = None;
            app.autoplay.consider(&player.engine.state(), &handle);
        }
    });
    if let Err(e) = spawned {
        tracing::warn!("autoplay: could not wait for the network: {e}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    use oxitide_audio::queue::ArtistRef;
    use oxitide_tidal::auth::SessionInfo;
    use oxitide_tidal::token_store::{MemoryStore, TokenSet};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn item(id: u64) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: "A".into(),
            album: "B".into(),
            duration_secs: 200,
            cover: None,
            artist_refs: vec![ArtistRef { id: 90 + id, name: "A".into() }],
            album_id: None,
            autoplay: false,
        }
    }

    fn state(ids: &[u64], current: Option<usize>, playing: bool) -> PlaybackState {
        let mut state = crate::remote::events::tests::playback(ids.iter().map(|&id| item(id)).collect(), current);
        state.playing = playing;
        state.repeat = RepeatMode::Off;
        state
    }

    fn track(id: u64) -> Track {
        serde_json::from_value(serde_json::json!({
            "id": id, "title": format!("R{id}"), "duration": 180,
            "artists": [{"id": 5, "name": "Band"}], "album": {"id": 6, "title": "LP", "cover": "c-d"}
        }))
        .unwrap()
    }

    #[test]
    fn fills_only_on_the_last_track() {
        assert_eq!(fill_target(&state(&[1, 2, 3], Some(2), true)).map(|i| i.track_id), Some(3));
        assert!(fill_target(&state(&[1, 2, 3], Some(1), true)).is_none(), "something follows");
        assert!(fill_target(&state(&[], None, false)).is_none());
    }

    #[test]
    fn a_pause_on_the_last_track_waits_but_an_end_fills() {
        let paused = state(&[1, 2], Some(1), false);
        assert!(fill_target(&paused).is_none());
        let ended = PlaybackState { ended: true, ..paused };
        assert_eq!(fill_target(&ended).map(|i| i.track_id), Some(2));
    }

    #[test]
    fn repeat_never_autoplays() {
        for repeat in [RepeatMode::All, RepeatMode::One] {
            let s = PlaybackState { repeat, ..state(&[1], Some(0), true) };
            assert!(fill_target(&s).is_none(), "{repeat:?}");
        }
    }

    #[test]
    fn candidates_skip_known_tracks_and_rank_recent_plays_last() {
        let tracks: Vec<Track> = [1, 2, 3, 3, 4, 5, 6].into_iter().map(track).collect();
        let queued = HashSet::from([2]);
        let picked = HashSet::from([4]);
        let recent = HashSet::from([3]);
        let items = candidates(&tracks, 1, &queued, &picked, &recent);
        let ids: Vec<u64> = items.iter().map(|i| i.track_id).collect();
        assert_eq!(ids, vec![5, 6, 3]);
        assert!(items.iter().all(|i| i.autoplay));
        assert_eq!(items[0].artist_refs[0].id, 5);
        assert_eq!(items[0].album_id, Some(6));
    }

    #[test]
    fn batches_come_from_the_pool_minus_what_got_queued() {
        let mut pool: Vec<QueueItem> = (1..=14).map(item).collect();
        let batch = take_batch(&mut pool, &HashSet::from([1, 2]));
        assert_eq!(batch.len(), BATCH);
        assert_eq!(batch[0].track_id, 3);
        assert_eq!(pool.iter().map(|i| i.track_id).collect::<Vec<_>>(), vec![13, 14]);
    }

    #[test]
    fn prune_removes_upcoming_suggestions_only() {
        let mut queue: Vec<QueueItem> = (1..=6).map(item).collect();
        for i in [1, 2, 4, 5] {
            queue[i].autoplay = true;
        }
        // Index 1 is current (keeps playing); 0 and 3 are the user's.
        assert_eq!(prune_indices(&queue, Some(1)), vec![5, 4, 2]);
        assert!(prune_indices(&queue, Some(5)).is_empty());
    }

    #[test]
    fn a_failed_fill_retries_only_after_a_while() {
        let mut run = Run { attempted: Some((1, 2)), ..Run::default() };
        assert!(!run.may_start((1, 2)), "same track and queue: asked already");
        assert!(run.may_start((1, 3)), "the queue changed");
        run.failed_at = Some(Instant::now() - RETRY_AFTER);
        assert!(run.may_start((1, 2)));
        run.in_flight = true;
        assert!(!run.may_start((9, 9)), "one fill at a time");
    }

    fn client(server: &MockServer) -> TidalClient {
        TidalClient::new(
            reqwest::Client::new(),
            server.uri(),
            format!("{}/token", server.uri()),
            SessionInfo { session_id: "s".into(), country_code: "GR".into(), user_id: 1 },
            TokenSet {
                access_token: "AT".into(),
                refresh_token: Some("RT".into()),
                token_type: "Bearer".into(),
                expires_at: None,
            },
            Arc::new(MemoryStore::default()),
        )
    }

    fn radio(ids: &[u64]) -> serde_json::Value {
        let items: Vec<serde_json::Value> = ids
            .iter()
            .map(|&id| serde_json::json!({"id": id, "title": format!("R{id}"), "type": "TRACK"}))
            .collect();
        serde_json::json!({ "items": items })
    }

    #[tokio::test]
    async fn radio_uses_the_track_radio() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/radio"))
            .respond_with(ResponseTemplate::new(200).set_body_json(radio(&[11, 12])))
            .mount(&server)
            .await;
        let tracks = radio_for(&client(&server), &item(7)).await.unwrap();
        assert_eq!(tracks.iter().map(|t| t.id).collect::<Vec<_>>(), vec![11, 12]);
    }

    #[tokio::test]
    async fn an_empty_or_missing_track_radio_falls_back_to_the_artist() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/tracks/7/radio"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;
        Mock::given(method("GET"))
            .and(path("/artists/97/radio"))
            .respond_with(ResponseTemplate::new(200).set_body_json(radio(&[21])))
            .mount(&server)
            .await;
        let tracks = radio_for(&client(&server), &item(7)).await.unwrap();
        assert_eq!(tracks[0].id, 21);

        let no_artist = QueueItem { artist_refs: Vec::new(), ..item(7) };
        assert!(radio_for(&client(&server), &no_artist).await.is_err());
    }
}
