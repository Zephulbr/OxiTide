use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::{Rng, RngExt};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QueueItem {
    pub track_id: u64,
    pub title: String,
    #[serde(default)]
    pub artist: String,
    #[serde(default)]
    pub album: String,
    #[serde(default)]
    pub duration_secs: u32,
    #[serde(default)]
    pub cover: Option<String>,
    /// The credited artists with ids, for the UI's name links. Empty when the
    /// source carried names only (page-feed tracks, older sessions); `artist`
    /// stays the display string either way.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artist_refs: Vec<ArtistRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub album_id: Option<u64>,
    /// Appended by Autoplay when the queue ran out, not queued by the user.
    #[serde(default, skip_serializing_if = "is_false")]
    pub autoplay: bool,
}

fn is_false(value: &bool) -> bool {
    !*value
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtistRef {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RepeatMode {
    Off,
    All,
    One,
}

#[derive(Debug, Clone)]
pub struct Queue {
    /// Copy-on-write: a state snapshot shares the list instead of copying it,
    /// and the next edit clones it only while a snapshot still holds it.
    items: Arc<Vec<QueueItem>>,
    /// Bumped on every change to the items (not to the current index or the
    /// flags), so listeners can skip re-sending an unchanged list.
    rev: u64,
    /// Bumped when a new context replaces the list (a play, a restore, a
    /// clear); a queue jump over the same list and ordinary edits keep it.
    /// `extend_context` adds a context's late-loaded rest only while it is
    /// still the one queued.
    context: u64,
    current: Option<usize>,
    /// One past the last track the user added with Play next / Add to queue
    /// that has not played yet: those tracks sit right after the current one,
    /// and the next "Add to queue" lands here, so added tracks play before the
    /// rest of the context and in the order they were added. A positional
    /// marker: edits shift it like an index; it is spent (ignored) once the
    /// current track reaches it, and cleared by a new context, a clear, a
    /// restore, or a Repeat All wrap. Not persisted.
    added_end: Option<usize>,
    playing: bool,
    shuffle: bool,
    repeat: RepeatMode,
    volume: f32,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            items: Arc::new(Vec::new()),
            rev: 0,
            context: 0,
            current: None,
            added_end: None,
            playing: false,
            shuffle: false,
            repeat: RepeatMode::Off,
            volume: 1.0,
        }
    }
}

impl Queue {
    pub fn items(&self) -> &[QueueItem] {
        &self.items
    }
    /// The items as a shared list (a pointer copy).
    pub fn shared_items(&self) -> Arc<Vec<QueueItem>> {
        Arc::clone(&self.items)
    }
    /// Revision of the item list; changes whenever the items do.
    pub fn rev(&self) -> u64 {
        self.rev
    }
    /// The context the list belongs to (see `extend_context`).
    pub fn context(&self) -> u64 {
        self.context
    }
    /// The items for an edit; marks the list changed.
    fn items_mut(&mut self) -> &mut Vec<QueueItem> {
        self.rev += 1;
        Arc::make_mut(&mut self.items)
    }
    fn replace_items(&mut self, items: Vec<QueueItem>) {
        self.rev += 1;
        self.items = Arc::new(items);
    }
    pub fn current_index(&self) -> Option<usize> {
        self.current
    }
    pub fn current(&self) -> Option<&QueueItem> {
        self.current.and_then(|i| self.items.get(i))
    }
    pub fn is_playing(&self) -> bool {
        self.playing
    }
    pub fn shuffle(&self) -> bool {
        self.shuffle
    }
    pub fn repeat(&self) -> RepeatMode {
        self.repeat
    }
    pub fn volume(&self) -> f32 {
        self.volume
    }

    /// Where the run of user-added tracks ends: after the added tracks still
    /// to play, else right after the current track. `None` without a current
    /// track.
    fn added_boundary(&self) -> Option<usize> {
        let after = self.current? + 1;
        let end = self.added_end.map_or(after, |end| end.max(after));
        Some(end.min(self.items.len()))
    }

    /// Forget the added run once nothing of it is left to play.
    fn settle_added(&mut self) {
        if let (Some(end), Some(cur)) = (self.added_end, self.current) {
            if end <= cur + 1 {
                self.added_end = None;
            }
        }
    }

    /// Play `items` from `start` in their order. A queue jump re-sends the
    /// queue itself (`EngineCmd::PlayTracks`): when the track list is the same,
    /// the added-tracks marker and the context survive the jump.
    pub fn play_context(&mut self, items: Vec<QueueItem>, start: usize) {
        let same = items.len() == self.items.len()
            && items
                .iter()
                .zip(self.items.iter())
                .all(|(a, b)| a.track_id == b.track_id);
        self.added_end = if same { self.added_end } else { None };
        if !same {
            self.context += 1;
        }
        self.replace_items(items);
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(start.min(self.items.len() - 1));
            self.playing = true;
            self.settle_added();
            // A queue jump straight to the last track.
            self.prepare_next_pass();
        }
    }

    /// Start a new context picked by the user (a row, Play all). With shuffle
    /// on, the picked item plays first and every other item, earlier ones
    /// included, follows in a shuffled order; with it off this is
    /// `play_context`. Queue jumps keep the order and use `play_context`.
    pub fn play_new_context(&mut self, items: Vec<QueueItem>, start: usize) {
        self.play_new_context_with(items, start, &mut rand::rng());
    }

    fn play_new_context_with<R: Rng + ?Sized>(
        &mut self,
        items: Vec<QueueItem>,
        start: usize,
        rng: &mut R,
    ) {
        self.play_context(items, start);
        // A new context, even the same list again, starts with nothing added,
        // and the rest of an earlier play of that list must not join it.
        self.added_end = None;
        self.context += 1;
        if self.shuffle {
            self.shuffle_around_current_with(rng);
        }
    }

    /// Start a fully shuffled context: the whole list (including its first
    /// item) is shuffled once, the shuffle flag turns on, and playback starts
    /// at the top of the shuffled order.
    pub fn play_context_shuffled(&mut self, items: Vec<QueueItem>) {
        self.play_context_shuffled_with(items, &mut rand::rng());
    }

    /// One Fisher–Yates shuffle of the whole list. Seeded by tests via `StdRng`.
    fn play_context_shuffled_with<R: Rng + ?Sized>(
        &mut self,
        mut items: Vec<QueueItem>,
        rng: &mut R,
    ) {
        items.shuffle(rng);
        self.replace_items(items);
        self.added_end = None;
        self.context += 1;
        self.shuffle = true;
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(0);
            self.playing = true;
        }
    }

    /// Restore a saved context exactly as persisted: the item order is kept
    /// (even with shuffle on — unlike `set_shuffle`, which re-shuffles the
    /// list), the start index is clamped, and playback starts paused.
    pub fn restore(
        &mut self,
        items: Vec<QueueItem>,
        start: usize,
        shuffle: bool,
        repeat: RepeatMode,
    ) {
        self.replace_items(items);
        self.added_end = None;
        self.context += 1;
        self.shuffle = shuffle;
        self.repeat = repeat;
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(start.min(self.items.len() - 1));
            self.playing = false;
        }
    }

    pub fn toggle_play_pause(&mut self) {
        if self.current.is_some() {
            self.playing = !self.playing;
        }
    }

    fn advance(&mut self) -> bool {
        let Some(cur) = self.current else {
            return false;
        };
        if self.items.is_empty() {
            return false;
        }
        if cur + 1 < self.items.len() {
            self.current = Some(cur + 1);
            self.playing = true;
            self.settle_added();
            self.prepare_next_pass();
            true
        } else if self.repeat == RepeatMode::All {
            self.current = Some(0);
            self.playing = true;
            self.added_end = None;
            true
        } else {
            self.playing = false;
            false
        }
    }

    /// Shuffle with Repeat All: once the last track is current, the tracks
    /// before it take a new order, so every pass plays differently. Done now
    /// rather than at the wrap so the next pass's first track is known while
    /// the last one plays (the engine preloads it for a gapless loop), and the
    /// track that just played never starts the next pass. Lists of two or
    /// fewer have no other order to take.
    fn prepare_next_pass(&mut self) {
        self.prepare_next_pass_with(&mut rand::rng());
    }

    fn prepare_next_pass_with<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        if !self.shuffle || self.repeat != RepeatMode::All {
            return;
        }
        match self.current {
            Some(last) if last >= 2 && last + 1 == self.items.len() => {
                self.items_mut()[..last].shuffle(rng);
                // Nothing added is left after the last track.
                self.added_end = None;
            }
            _ => {}
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        self.advance()
    }

    pub fn prev(&mut self) -> bool {
        let Some(cur) = self.current else {
            return false;
        };
        if cur == 0 {
            return false;
        }
        self.current = Some(cur - 1);
        self.playing = true;
        true
    }

    /// What `on_track_ended` would select, without mutating. Used to prefetch
    /// the next track while the current one plays.
    pub fn peek_next_index(&self) -> Option<usize> {
        let cur = self.current?;
        if self.items.is_empty() {
            return None;
        }
        if self.repeat == RepeatMode::One {
            return Some(cur);
        }
        if cur + 1 < self.items.len() {
            return Some(cur + 1);
        }
        if self.repeat == RepeatMode::All {
            return Some(0);
        }
        None
    }

    pub fn on_track_ended(&mut self) -> Option<usize> {
        match self.peek_next_index() {
            Some(index) => {
                let wrapped = self.current.is_some_and(|cur| index < cur);
                self.current = Some(index);
                self.playing = true;
                if wrapped {
                    self.added_end = None;
                } else {
                    self.settle_added();
                }
                if self.repeat != RepeatMode::One {
                    self.prepare_next_pass();
                }
                Some(index)
            }
            // End of queue with repeat off: stop, keep the last item current.
            None => {
                self.playing = false;
                None
            }
        }
    }

    /// Advance past the current track after it proved unplayable. Unlike
    /// `on_track_ended`, repeat-one never re-selects the failed track; it
    /// falls through to the next index (or stops at the end with repeat off).
    pub fn skip_current(&mut self) -> Option<usize> {
        let cur = self.current?;
        if self.items.is_empty() {
            return None;
        }
        if cur + 1 < self.items.len() {
            self.current = Some(cur + 1);
            self.playing = true;
            self.settle_added();
            self.prepare_next_pass();
            Some(cur + 1)
        } else if self.repeat == RepeatMode::All {
            self.current = Some(0);
            self.playing = true;
            self.added_end = None;
            Some(0)
        } else {
            self.playing = false;
            None
        }
    }

    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        let items = self.items_mut();
        let item = items.remove(from);
        items.insert(to, item);
        // The marker moves like a remove at `from`, then an insert at `to`: a
        // track dragged into the added run joins it, one dragged out leaves it.
        // A track moved within the run stays in it, its last slot included.
        if let Some(end) = self.added_end {
            let in_run = self.current.is_some_and(|cur| cur < from) && from < end;
            let end = if from < end { end - 1 } else { end };
            let joins = if in_run { to <= end } else { to < end };
            self.added_end = Some(if joins { end + 1 } else { end });
        }
        if let Some(cur) = self.current {
            if cur == from {
                self.current = Some(to);
            } else if from < cur && to >= cur {
                self.current = Some(cur - 1);
            } else if from > cur && to <= cur {
                self.current = Some(cur + 1);
            }
        }
        true
    }

    pub fn remove(&mut self, index: usize) -> bool {
        if index >= self.items.len() {
            return false;
        }
        self.items_mut().remove(index);
        if let Some(end) = self.added_end {
            if index < end {
                self.added_end = Some(end - 1);
            }
        }
        match self.current {
            None => {}
            Some(cur) if index < cur => self.current = Some(cur - 1),
            Some(cur) if index == cur => {
                if self.items.is_empty() {
                    self.current = None;
                    self.playing = false;
                } else {
                    self.current = Some(cur.min(self.items.len() - 1));
                }
            }
            Some(_) => {}
        }
        true
    }

    pub fn clear(&mut self) {
        self.replace_items(Vec::new());
        self.context += 1;
        self.current = None;
        self.added_end = None;
        self.playing = false;
    }

    /// "Add to queue": the items go after the tracks the user added earlier
    /// that are still to play, else right after the current track, so added
    /// tracks play before the rest of the context, in the order they were
    /// added (like TIDAL's and Spotify's queues). Without a current track, and
    /// for Autoplay's suggestions, they append. The current track and the
    /// playing state are untouched.
    pub fn push(&mut self, items: Vec<QueueItem>) {
        if items.is_empty() {
            return;
        }
        let suggested = items.iter().all(|item| item.autoplay);
        match self.added_boundary() {
            Some(at) if !suggested => {
                let count = items.len();
                self.items_mut().splice(at..at, items);
                self.added_end = Some(at + count);
            }
            _ => self.items_mut().extend(items),
        }
    }

    /// Add the rest of a context that started from the rows already loaded (a
    /// search play queues an artist's first tracks, a Tracks play the loaded
    /// favorites, then the others once loaded; issues #3 and #7). Applies
    /// only while `context` (the one the play returned) is still queued: a new
    /// play, a restore or a clear drops it, an edit does not, so adding a song
    /// while the rest loads keeps the whole list. The items go after the
    /// tracks the user added and before Autoplay's suggestions. With shuffle
    /// on, they are shuffled and merged into that range at random slots (the
    /// upcoming tracks keep their order), so the context stays one uniform
    /// shuffle; otherwise they go there in order. Linear: a Tracks play can
    /// send ~10k items, and this runs on the main thread.
    pub fn extend_context(&mut self, items: Vec<QueueItem>, context: u64) -> bool {
        self.extend_context_with(items, context, &mut rand::rng())
    }

    fn extend_context_with<R: Rng + ?Sized>(
        &mut self,
        items: Vec<QueueItem>,
        context: u64,
        rng: &mut R,
    ) -> bool {
        if self.context != context || items.is_empty() {
            return false;
        }
        let Some(start) = self.added_boundary() else {
            return false;
        };
        let end = self.items[start..]
            .iter()
            .position(|item| item.autoplay)
            .map_or(self.items.len(), |offset| start + offset);
        let shuffle = self.shuffle;
        let list = self.items_mut();
        if shuffle {
            let mut items = items;
            items.shuffle(rng);
            let upcoming: Vec<QueueItem> = list.drain(start..end).collect();
            let merged = merge_at_random(upcoming, items, rng);
            list.splice(start..start, merged);
        } else {
            list.splice(end..end, items);
        }
        true
    }

    /// Insert items right after the current track ("Play next"), keeping
    /// their order; with no current track they append. They join the added
    /// run (ahead of earlier added tracks), so a later "Add to queue" still
    /// lands after them. The current track and the playing state are untouched.
    pub fn insert_next(&mut self, items: Vec<QueueItem>) {
        if items.is_empty() {
            return;
        }
        match (self.current, self.added_boundary()) {
            (Some(cur), Some(end)) => {
                let at = (cur + 1).min(self.items.len());
                let count = items.len();
                self.items_mut().splice(at..at, items);
                self.added_end = Some(end + count);
            }
            _ => self.items_mut().extend(items),
        }
    }

    /// Insert items at slot `index` (before the item now there; past the end
    /// appends), keeping their order. The current track stays current: an
    /// insert at or before it shifts its index. The playing state is untouched.
    pub fn insert_at(&mut self, index: usize, items: Vec<QueueItem>) {
        if items.is_empty() {
            return;
        }
        let at = index.min(self.items.len());
        let count = items.len();
        self.items_mut().splice(at..at, items);
        // A drop inside the added run joins it; at its end or later it does not.
        if let Some(end) = self.added_end {
            if at < end {
                self.added_end = Some(end + count);
            }
        }
        if let Some(cur) = self.current {
            if at <= cur {
                self.current = Some(cur + count);
            }
        }
    }

    /// Stop playback without clearing the queue (e.g. after a playback error).
    pub fn stop(&mut self) {
        self.playing = false;
    }

    /// Resume playback without touching the queue (e.g. after a retry).
    pub fn resume(&mut self) {
        if self.current.is_some() {
            self.playing = true;
        }
    }

    /// A non-finite value is ignored: `clamp` passes NaN through, and a NaN
    /// volume never compares as changed, so it would stick silently.
    pub fn set_volume(&mut self, v: f32) {
        if v.is_finite() {
            self.volume = v.clamp(0.0, 1.0);
        }
    }

    /// Enabling shuffles the whole list once (the items before the current
    /// one too, not only those after it): the current item moves to the top
    /// and keeps playing, the rest follow in a shuffled order. Disabling
    /// leaves the shuffled order in place. Enabling it when it is already on
    /// changes nothing (a repeated command must not rescramble the queue).
    pub fn set_shuffle(&mut self, on: bool) {
        if on && !self.shuffle {
            self.shuffle_around_current_with(&mut rand::rng());
        }
        self.shuffle = on;
    }

    /// One Fisher–Yates shuffle of every item but the current one, which
    /// moves to index 0 (it is still current). Tracks the user added that are
    /// still to play are not shuffled: they follow the current one in their
    /// order. Without a current item the whole list shuffles. Seeded by tests
    /// via `StdRng`.
    fn shuffle_around_current_with<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        match self.current {
            Some(current) if self.items.len() > 1 => {
                let end = self.added_boundary().unwrap_or(current + 1);
                let items = self.items_mut();
                let added: Vec<QueueItem> = items.drain(current + 1..end).collect();
                let item = items.remove(current);
                items.shuffle(rng);
                items.insert(0, item);
                let count = added.len();
                items.splice(1..1, added);
                self.current = Some(0);
                self.added_end = (count > 0).then_some(1 + count);
            }
            Some(_) => {}
            None => self.items_mut().shuffle(rng),
        }
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        let was = self.repeat;
        self.repeat = mode;
        // Repeat All turned on while the last track plays.
        if was != RepeatMode::All {
            self.prepare_next_pass();
        }
    }
}

/// Interleave `b` into `a` at uniformly random slots, keeping each list's own
/// order: every interleaving is equally likely (each next slot takes from a
/// list with the odds of its remaining share). With `b` shuffled first this
/// matches inserting each of `b` at a random slot one by one, in linear time.
fn merge_at_random<T, R: Rng + ?Sized>(a: Vec<T>, b: Vec<T>, rng: &mut R) -> Vec<T> {
    let (mut left_a, mut left_b) = (a.len(), b.len());
    let mut merged = Vec::with_capacity(left_a + left_b);
    let (mut a, mut b) = (a.into_iter(), b.into_iter());
    while left_a + left_b > 0 {
        if rng.random_range(0..left_a + left_b) < left_b {
            merged.extend(b.next());
            left_b -= 1;
        } else {
            merged.extend(a.next());
            left_a -= 1;
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64) -> QueueItem {
        QueueItem {
            track_id: id,
            title: format!("T{id}"),
            artist: String::new(),
            album: String::new(),
            duration_secs: 1,
            cover: None,
            artist_refs: Vec::new(),
            album_id: None,
            autoplay: false,
        }
    }

    #[test]
    fn autoplay_flag_is_written_only_when_set() {
        let plain = serde_json::to_value(item(1)).unwrap();
        assert!(plain.get("autoplay").is_none());
        let suggested = QueueItem {
            autoplay: true,
            ..item(1)
        };
        let json = serde_json::to_string(&suggested).unwrap();
        assert!(json.contains(r#""autoplay":true"#));
        assert_eq!(serde_json::from_str::<QueueItem>(&json).unwrap(), suggested);
        let old: QueueItem = serde_json::from_str(r#"{"track_id":1,"title":"T"}"#).unwrap();
        assert!(!old.autoplay);
    }

    fn queue(n: u64) -> Queue {
        let mut q = Queue::default();
        q.play_context((1..=n).map(item).collect(), 0);
        q
    }

    #[test]
    fn rev_moves_with_item_edits_only() {
        let mut q = queue(4);
        let rev = q.rev();
        q.next();
        q.prev();
        q.set_repeat(RepeatMode::All);
        q.set_volume(0.5);
        q.toggle_play_pause();
        q.stop();
        q.resume();
        q.set_shuffle(false);
        assert_eq!(
            q.rev(),
            rev,
            "navigation and flags must not change the revision"
        );
        for edit in [
            |q: &mut Queue| q.push(vec![item(9)]),
            |q: &mut Queue| q.insert_next(vec![item(8)]),
            |q: &mut Queue| q.insert_at(1, vec![item(7)]),
            |q: &mut Queue| {
                q.reorder(0, 1);
            },
            |q: &mut Queue| {
                q.remove(0);
            },
            |q: &mut Queue| q.set_shuffle(true),
            |q: &mut Queue| q.play_context(vec![item(1)], 0),
            |q: &mut Queue| q.clear(),
        ] {
            let before = q.rev();
            edit(&mut q);
            assert!(q.rev() > before, "an item edit kept the revision");
        }
    }

    #[test]
    fn a_shared_snapshot_is_unaffected_by_later_edits() {
        let mut q = queue(3);
        let snapshot = q.shared_items();
        assert!(
            Arc::ptr_eq(&snapshot, &q.shared_items()),
            "a snapshot must not copy"
        );
        q.push(vec![item(4)]);
        assert_eq!(snapshot.len(), 3);
        assert_eq!(q.items().len(), 4);
    }

    #[test]
    fn play_context_sets_current_and_playing() {
        let q = queue(3);
        assert_eq!(q.current_index(), Some(0));
        assert!(q.is_playing());
        assert_eq!(q.current().unwrap().track_id, 1);
    }

    #[test]
    fn play_context_shuffled_starts_a_permutation_from_the_top() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.play_context_shuffled_with(
            (1..=8).map(item).collect(),
            &mut rand::rngs::StdRng::seed_from_u64(11),
        );
        assert!(q.shuffle());
        assert!(q.is_playing());
        assert_eq!(q.current_index(), Some(0));
        let order: Vec<u64> = q.items().iter().map(|i| i.track_id).collect();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=8).collect::<Vec<_>>(), "not a permutation");
        assert_ne!(order, (1..=8).collect::<Vec<_>>(), "order was not shuffled");
    }

    #[test]
    fn play_context_shuffled_handles_empty() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.play_context_shuffled_with(Vec::new(), &mut rand::rngs::StdRng::seed_from_u64(1));
        assert!(q.shuffle());
        assert_eq!(q.current_index(), None);
        assert!(!q.is_playing());
        assert!(q.items().is_empty());
    }

    #[test]
    fn next_at_end_stops_when_repeat_off() {
        let mut q = queue(2);
        assert!(q.next());
        assert_eq!(q.current_index(), Some(1));
        assert!(!q.next());
        assert!(!q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn next_wraps_when_repeat_all() {
        let mut q = queue(2);
        q.set_repeat(RepeatMode::All);
        q.next();
        assert!(q.next());
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn on_track_ended_repeats_one() {
        let mut q = queue(3);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.on_track_ended(), Some(0));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn on_track_ended_stops_at_end_when_repeat_off() {
        let mut q = queue(2);
        q.next(); // current = last item
        assert_eq!(q.on_track_ended(), None);
        assert!(!q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn peek_next_index_matches_on_track_ended() {
        for repeat in [RepeatMode::Off, RepeatMode::All, RepeatMode::One] {
            for start in 0..3 {
                let mut q = Queue::default();
                q.play_context((1..=3).map(item).collect(), start);
                q.set_repeat(repeat);
                let mut expected = q.clone();
                assert_eq!(
                    q.peek_next_index(),
                    expected.on_track_ended(),
                    "repeat {repeat:?}, start {start}"
                );
            }
        }
    }

    #[test]
    fn peek_next_index_is_none_when_stopped_at_end() {
        let mut q = queue(2);
        q.next(); // last item
        assert_eq!(q.peek_next_index(), None);
        // Peeks never mutate.
        assert!(q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn skip_current_advances_to_the_next_track() {
        let mut q = queue(3);
        assert_eq!(q.skip_current(), Some(1));
        assert_eq!(q.current_index(), Some(1));
        assert!(q.is_playing());
    }

    #[test]
    fn skip_current_ignores_repeat_one() {
        let mut q = queue(3);
        q.set_repeat(RepeatMode::One);
        assert_eq!(q.skip_current(), Some(1));
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn skip_current_wraps_when_repeat_all() {
        let mut q = queue(2);
        q.next(); // current = last item
        q.set_repeat(RepeatMode::All);
        assert_eq!(q.skip_current(), Some(0));
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn skip_current_stops_at_end_when_repeat_off() {
        let mut q = queue(2);
        q.next(); // current = last item
        assert_eq!(q.skip_current(), None);
        assert!(!q.is_playing());
        assert_eq!(q.current_index(), Some(1));
    }

    #[test]
    fn prev_at_start_is_a_noop() {
        let mut q = queue(3);
        assert!(!q.prev());
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn reorder_preserves_current_item() {
        let mut q = queue(3);
        q.next(); // current = index 1 (track 2)
        assert!(q.reorder(0, 2));
        assert_eq!(q.current().unwrap().track_id, 2);
        assert_eq!(q.items()[2].track_id, 1);
    }

    #[test]
    fn insert_next_lands_after_current_in_order() {
        let mut q = queue(3);
        q.next(); // current track 2 at index 1
        q.insert_next(vec![item(8), item(9)]);
        let ids: Vec<_> = q.items().iter().map(|i| i.track_id).collect();
        assert_eq!(ids, vec![1, 2, 8, 9, 3]);
        assert_eq!(q.current_index(), Some(1));
        assert!(q.is_playing());
    }

    #[test]
    fn insert_next_without_current_appends() {
        let mut q = Queue::default();
        q.insert_next(vec![item(1), item(2)]);
        let ids: Vec<_> = q.items().iter().map(|i| i.track_id).collect();
        assert_eq!(ids, vec![1, 2]);
        assert_eq!(q.current_index(), None);
    }

    fn ids(q: &Queue) -> Vec<u64> {
        q.items().iter().map(|i| i.track_id).collect()
    }

    fn suggested(id: u64) -> QueueItem {
        QueueItem {
            autoplay: true,
            ..item(id)
        }
    }

    #[test]
    fn add_to_queue_plays_before_the_rest_of_the_context() {
        let mut q = queue(4); // playing 1
        q.push(vec![item(8)]);
        assert_eq!(ids(&q), vec![1, 8, 2, 3, 4]);
        assert_eq!(q.current_index(), Some(0));
        assert!(q.is_playing());
    }

    #[test]
    fn later_adds_follow_earlier_ones_in_order() {
        let mut q = queue(3);
        q.push(vec![item(8)]);
        q.push(vec![item(9), item(10)]);
        assert_eq!(ids(&q), vec![1, 8, 9, 10, 2, 3]);
    }

    #[test]
    fn play_next_jumps_the_added_run_and_later_adds_go_after_it() {
        let mut q = queue(3);
        q.push(vec![item(8)]);
        q.insert_next(vec![item(7)]);
        assert_eq!(ids(&q), vec![1, 7, 8, 2, 3]);
        q.push(vec![item(9)]);
        assert_eq!(ids(&q), vec![1, 7, 8, 9, 2, 3]);
    }

    #[test]
    fn the_added_run_shrinks_as_it_plays() {
        let mut q = queue(3);
        q.push(vec![item(8), item(9)]);
        q.next(); // 8 plays; 9 still added
        q.push(vec![item(10)]);
        assert_eq!(ids(&q), vec![1, 8, 9, 10, 2, 3]);
        q.next(); // 9
        q.next(); // 10: the run is spent
        q.push(vec![item(11)]);
        assert_eq!(ids(&q), vec![1, 8, 9, 10, 11, 2, 3]);
        q.next(); // 11
        q.next(); // 2: back in the context
        q.push(vec![item(12)]);
        assert_eq!(ids(&q), vec![1, 8, 9, 10, 11, 2, 12, 3]);
    }

    #[test]
    fn add_to_queue_on_the_last_track_or_with_nothing_current_appends() {
        let mut q = queue(2);
        q.next(); // last track
        q.push(vec![item(8)]);
        assert_eq!(ids(&q), vec![1, 2, 8]);
        let mut empty = Queue::default();
        empty.push(vec![item(1), item(2)]);
        assert_eq!(ids(&empty), vec![1, 2]);
        assert_eq!(empty.current_index(), None);
    }

    #[test]
    fn autoplay_appends_and_user_adds_go_before_it() {
        let mut q = queue(2);
        q.next(); // last track: Autoplay fills
        q.push(vec![suggested(50), suggested(51)]);
        assert_eq!(ids(&q), vec![1, 2, 50, 51]);
        q.push(vec![item(8)]);
        assert_eq!(ids(&q), vec![1, 2, 8, 50, 51]);
        // Suggestions never extend the added run.
        q.push(vec![suggested(52)]);
        q.push(vec![item(9)]);
        assert_eq!(ids(&q), vec![1, 2, 8, 9, 50, 51, 52]);
    }

    #[test]
    fn a_new_context_or_clear_forgets_the_added_run() {
        let mut q = queue(3);
        q.push(vec![item(8)]);
        q.play_new_context((1..=3).map(item).collect(), 0);
        q.push(vec![item(9)]);
        assert_eq!(ids(&q), vec![1, 9, 2, 3]);
        q.clear();
        q.play_context((1..=3).map(item).collect(), 0);
        q.push(vec![item(10)]);
        assert_eq!(ids(&q), vec![1, 10, 2, 3]);
    }

    #[test]
    fn a_queue_jump_keeps_the_added_run() {
        let mut q = queue(4);
        q.push(vec![item(8), item(9)]); // 1 8 9 2 3 4
        let same = q.items().to_vec();
        q.play_context(same, 1); // jump to 8 (the UI re-sends the queue)
        q.push(vec![item(10)]);
        assert_eq!(ids(&q), vec![1, 8, 9, 10, 2, 3, 4]);
    }

    #[test]
    fn edits_shift_the_added_run() {
        let mut q = queue(4);
        q.push(vec![item(8), item(9)]); // 1 8 9 2 3 4, run ends at 3
        q.remove(1); // 1 9 2 3 4
        q.push(vec![item(10)]);
        assert_eq!(ids(&q), vec![1, 9, 10, 2, 3, 4]);
        q.reorder(4, 1); // drag 3 into the run: 1 3 9 10 2 4
        q.push(vec![item(11)]);
        assert_eq!(ids(&q), vec![1, 3, 9, 10, 11, 2, 4]);
        q.reorder(1, 6); // drag 3 out to the end: 1 9 10 11 2 4 3
        q.insert_at(2, vec![item(12)]); // dropped inside the run
        q.push(vec![item(13)]);
        assert_eq!(ids(&q), vec![1, 9, 12, 10, 11, 13, 2, 4, 3]);
        q.insert_at(6, vec![item(14)]); // at the run's end: not added
        q.push(vec![item(15)]);
        assert_eq!(ids(&q), vec![1, 9, 12, 10, 11, 13, 15, 14, 2, 4, 3]);
    }

    #[test]
    fn reordering_within_the_added_run_keeps_it_whole() {
        let mut q = queue(3);
        q.push(vec![item(8), item(9)]); // 1 8 9 2 3
        q.reorder(1, 2); // 1 9 8 2 3
        q.push(vec![item(10)]);
        assert_eq!(ids(&q), vec![1, 9, 8, 10, 2, 3]);
        q.reorder(3, 1); // 1 10 9 8 2 3
        q.push(vec![item(11)]);
        assert_eq!(ids(&q), vec![1, 10, 9, 8, 11, 2, 3]);
    }

    #[test]
    fn shuffle_keeps_the_added_run_next_and_in_order() {
        use rand::SeedableRng;
        let mut q = queue(8);
        q.next(); // playing 2
        q.push(vec![item(20), item(21)]);
        q.shuffle_around_current_with(&mut rand::rngs::StdRng::seed_from_u64(5));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(&ids(&q)[..3], &[2, 20, 21]);
        let mut rest = ids(&q)[3..].to_vec();
        rest.sort_unstable();
        assert_eq!(rest, vec![1, 3, 4, 5, 6, 7, 8]);
        // The run is still the added run.
        q.push(vec![item(22)]);
        assert_eq!(&ids(&q)[..4], &[2, 20, 21, 22]);
    }

    #[test]
    fn after_a_repeat_all_wrap_adds_go_right_after_the_current_track() {
        let mut q = queue(2);
        q.set_repeat(RepeatMode::All);
        q.next(); // 2 (last)
        q.insert_at(1, vec![item(8)]); // before the current one: 1 8 2
        q.push(vec![item(9)]); // 1 8 2 9, run ends after 9
        assert_eq!(ids(&q), vec![1, 8, 2, 9]);
        q.next(); // 9
        assert_eq!(q.on_track_ended(), Some(0)); // wrap to 1
        q.push(vec![item(10)]);
        assert_eq!(ids(&q), vec![1, 10, 8, 2, 9]);
    }

    #[test]
    fn insert_at_before_current_keeps_it_playing() {
        let mut q = queue(3);
        q.next(); // current track 2 at index 1
        q.insert_at(0, vec![item(8), item(9)]);
        assert_eq!(ids(&q), vec![8, 9, 1, 2, 3]);
        assert_eq!(q.current_index(), Some(3));
        assert_eq!(q.current().unwrap().track_id, 2);
        assert!(q.is_playing());
    }

    #[test]
    fn insert_at_current_slot_lands_before_it() {
        let mut q = queue(3);
        q.next(); // current track 2 at index 1
        q.insert_at(1, vec![item(8)]);
        assert_eq!(ids(&q), vec![1, 8, 2, 3]);
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn insert_at_after_current_keeps_index() {
        let mut q = queue(3);
        q.insert_at(2, vec![item(8)]);
        assert_eq!(ids(&q), vec![1, 2, 8, 3]);
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.peek_next_index(), Some(1));
    }

    #[test]
    fn insert_at_past_the_end_appends() {
        let mut q = queue(2);
        q.insert_at(99, vec![item(8), item(9)]);
        assert_eq!(ids(&q), vec![1, 2, 8, 9]);
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn insert_at_into_empty_queue_sets_no_current() {
        let mut q = Queue::default();
        q.insert_at(0, vec![item(1), item(2)]);
        assert_eq!(ids(&q), vec![1, 2]);
        assert_eq!(q.current_index(), None);
        assert!(!q.is_playing());
    }

    #[test]
    fn insert_at_with_no_items_changes_nothing() {
        let mut q = queue(2);
        let rev = q.rev();
        q.insert_at(1, Vec::new());
        assert_eq!(q.rev(), rev);
        assert_eq!(ids(&q), vec![1, 2]);
    }

    #[test]
    fn remove_before_current_shifts_index() {
        let mut q = queue(3);
        q.next(); // current track 2 at index 1
        assert!(q.remove(0));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn remove_current_advances_to_next() {
        let mut q = queue(3);
        assert!(q.remove(0));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn clear_empties_the_queue() {
        let mut q = queue(2);
        q.clear();
        assert!(q.items().is_empty());
        assert_eq!(q.current_index(), None);
        assert!(!q.is_playing());
    }

    #[test]
    fn push_appends_without_touching_current() {
        let mut q = queue(2);
        q.next(); // current = track 2
        q.push(vec![item(3), item(4)]);
        assert_eq!(q.items().len(), 4);
        assert_eq!(q.current_index(), Some(1));
        assert!(q.is_playing());
    }

    #[test]
    fn stop_keeps_items_but_clears_playing() {
        let mut q = queue(2);
        q.stop();
        assert!(!q.is_playing());
        assert_eq!(q.items().len(), 2);
    }

    #[test]
    fn resume_keeps_items_and_sets_playing() {
        let mut q = queue(2);
        q.stop();
        q.resume();
        assert!(q.is_playing());
        assert_eq!(q.items().len(), 2);
    }

    #[test]
    fn resume_without_a_current_track_is_a_noop() {
        let mut q = Queue::default();
        q.resume();
        assert!(!q.is_playing());
    }

    #[test]
    fn volume_is_clamped() {
        let mut q = Queue::default();
        q.set_volume(2.0);
        assert_eq!(q.volume(), 1.0);
        q.set_volume(-1.0);
        assert_eq!(q.volume(), 0.0);
        q.set_volume(0.4);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            q.set_volume(bad);
            assert_eq!(q.volume(), 0.4, "{bad} is ignored");
        }
    }

    /// Issue #3: shuffling from the middle of a list must take in the tracks
    /// before the current one too, not only those after it.
    #[test]
    fn shuffle_from_the_middle_takes_in_the_earlier_tracks() {
        use rand::SeedableRng;
        let mut q = queue(8);
        q.play_context((1..=8).map(item).collect(), 4); // current = track 5
        q.shuffle_around_current_with(&mut rand::rngs::StdRng::seed_from_u64(7));
        assert_eq!(
            q.current_index(),
            Some(0),
            "the current item moves to the top"
        );
        assert_eq!(q.current().unwrap().track_id, 5, "the current item changed");
        assert!(q.is_playing());
        let order = ids(&q);
        let mut sorted = order.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=8).collect::<Vec<_>>(), "not a permutation");
        // Every track plays after the current one, the earlier ones included.
        let upcoming: Vec<u64> = order[1..].to_vec();
        for earlier in 1..5 {
            assert!(
                upcoming.contains(&earlier),
                "track {earlier} was left behind"
            );
        }
        assert_ne!(
            upcoming,
            vec![1, 2, 3, 4, 6, 7, 8],
            "seed 7 produced the identity order; pick another seed"
        );
    }

    #[test]
    fn set_shuffle_keeps_the_current_track_playing() {
        let mut q = queue(8);
        q.next();
        q.set_shuffle(true);
        assert!(q.shuffle());
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current().unwrap().track_id, 2);
        assert_eq!(q.items().len(), 8);
    }

    #[test]
    fn enabling_shuffle_twice_keeps_the_order() {
        let mut q = queue(8);
        q.next();
        q.set_shuffle(true);
        let order = ids(&q);
        let rev = q.rev();
        q.set_shuffle(true);
        assert_eq!(ids(&q), order);
        assert_eq!(q.rev(), rev);
    }

    #[test]
    fn set_shuffle_on_a_single_item_keeps_it() {
        let mut q = queue(1);
        q.set_shuffle(true);
        assert_eq!(ids(&q), vec![1]);
        assert_eq!(q.current_index(), Some(0));
    }

    /// Issue #3: a new context started while shuffle is on plays the picked
    /// track first and shuffles everything else.
    #[test]
    fn a_new_context_with_shuffle_on_starts_at_the_pick_and_shuffles_the_rest() {
        use rand::SeedableRng;
        let mut q = queue(3);
        q.set_shuffle(true);
        q.play_new_context_with(
            (10..=17).map(item).collect(),
            5,
            &mut rand::rngs::StdRng::seed_from_u64(7),
        );
        assert!(q.shuffle(), "the shuffle flag stays on");
        assert!(q.is_playing());
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(q.current().unwrap().track_id, 15, "the picked track plays");
        let mut sorted = ids(&q);
        sorted.sort_unstable();
        assert_eq!(sorted, (10..=17).collect::<Vec<_>>(), "not a permutation");
        assert_ne!(
            ids(&q),
            vec![15, 10, 11, 12, 13, 14, 16, 17],
            "seed 7 produced the identity order; pick another seed"
        );
    }

    #[test]
    fn extend_context_appends_in_order_to_the_play_context() {
        let mut q = Queue::default();
        q.play_new_context((1..=3).map(item).collect(), 0);
        let rev = q.rev();
        assert!(q.extend_context(vec![item(4), item(5)], q.context()));
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5]);
        assert_eq!(q.current_index(), Some(0));
        assert!(q.rev() > rev);
    }

    /// Issue #7: a song added while the rest of Tracks loads must not leave
    /// the queue holding only the first page.
    #[test]
    fn extend_context_survives_queue_edits_after_the_added_tracks() {
        let mut q = Queue::default();
        q.play_new_context((1..=4).map(item).collect(), 0);
        let context = q.context();
        q.push(vec![item(9)]);
        assert_eq!(ids(&q), vec![1, 9, 2, 3, 4]);
        assert!(q.remove(4));
        assert!(q.extend_context(vec![item(5), item(6)], context));
        assert_eq!(ids(&q), vec![1, 9, 2, 3, 5, 6]);
        assert_eq!(q.current_index(), Some(0));
        // The added run is intact: the next "Add to queue" still follows it.
        q.push(vec![item(8)]);
        assert_eq!(ids(&q), vec![1, 9, 8, 2, 3, 5, 6]);
    }

    #[test]
    fn extend_context_lands_before_autoplay_suggestions() {
        let mut q = Queue::default();
        q.play_new_context((1..=2).map(item).collect(), 1);
        let context = q.context();
        q.push(vec![suggested(20), suggested(21)]);
        assert!(q.extend_context(vec![item(3), item(4)], context));
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 20, 21]);
    }

    #[test]
    fn extend_context_is_dropped_by_a_new_context() {
        let mut q = Queue::default();
        q.play_new_context((1..=3).map(item).collect(), 0);
        let context = q.context();
        q.play_new_context(vec![item(7)], 0);
        assert!(!q.extend_context(vec![item(4)], context));
        assert_eq!(ids(&q), vec![7]);

        let context = q.context();
        q.play_context_shuffled(vec![item(7), item(8)]);
        assert!(!q.extend_context(vec![item(4)], context));

        let context = q.context();
        q.restore(vec![item(7)], 0, false, RepeatMode::Off);
        assert!(!q.extend_context(vec![item(4)], context));

        let context = q.context();
        q.clear();
        q.push(vec![item(7)]);
        assert!(!q.extend_context(vec![item(4)], context));
        assert_eq!(ids(&q), vec![7]);
    }

    /// Two clicks in a list that is still loading send the same rows twice;
    /// the first play's rest must not join the second's on top of its own.
    #[test]
    fn replaying_the_same_list_starts_a_new_context() {
        let mut q = Queue::default();
        q.play_new_context((1..=3).map(item).collect(), 0);
        let first = q.context();
        q.play_new_context((1..=3).map(item).collect(), 2);
        assert!(!q.extend_context(vec![item(4)], first));
        assert!(q.extend_context(vec![item(4)], q.context()));
        assert_eq!(ids(&q), vec![1, 2, 3, 4]);
    }

    /// The queue rail's jump re-sends the queue itself: the rest still loading
    /// for that queue still joins it.
    #[test]
    fn a_queue_jump_keeps_the_context() {
        let mut q = Queue::default();
        q.play_new_context((1..=3).map(item).collect(), 0);
        let context = q.context();
        q.play_context((1..=3).map(item).collect(), 2);
        assert!(q.extend_context(vec![item(4)], context));
        assert_eq!(ids(&q), vec![1, 2, 3, 4]);
    }

    #[test]
    fn merge_at_random_keeps_both_orders_and_draws_every_interleaving() {
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(3);
        let mut counts = std::collections::HashMap::new();
        for _ in 0..6000 {
            let merged = merge_at_random(vec![1, 2], vec![10, 20], &mut rng);
            let a: Vec<i32> = merged.iter().copied().filter(|v| *v < 10).collect();
            let b: Vec<i32> = merged.iter().copied().filter(|v| *v >= 10).collect();
            assert_eq!((a, b), (vec![1, 2], vec![10, 20]));
            *counts.entry(merged).or_insert(0) += 1;
        }
        // C(4, 2) = 6 interleavings, ~1000 each.
        assert_eq!(counts.len(), 6);
        assert!(
            counts.values().all(|n| (850..=1150).contains(n)),
            "{counts:?}"
        );
    }

    /// A Tracks play while the favorites load: ~100 rows, then the rest.
    #[test]
    fn extend_context_takes_a_whole_collection_in_one_go() {
        let mut q = Queue::default();
        q.set_shuffle(true);
        q.play_new_context((1..=100).map(item).collect(), 0);
        let context = q.context();
        assert!(q.extend_context((101..=10_000).map(item).collect(), context));
        let mut sorted = ids(&q);
        assert_eq!(sorted.len(), 10_000);
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=10_000).collect::<Vec<_>>());
        assert_eq!(q.current_index(), Some(0));
    }

    #[test]
    fn extend_context_with_shuffle_keeps_added_tracks_next() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.set_shuffle(true);
        q.play_new_context((1..=4).map(item).collect(), 0);
        let context = q.context();
        q.push(vec![item(9)]);
        q.push(vec![suggested(20)]);
        assert!(q.extend_context_with(
            (10..=19).map(item).collect(),
            context,
            &mut rand::rngs::StdRng::seed_from_u64(7),
        ));
        let order = ids(&q);
        assert_eq!(order[..2], [1, 9], "the current track, then the added one");
        assert_eq!(*order.last().unwrap(), 20, "the suggestion stays last");
        assert_eq!(order.len(), 4 + 1 + 1 + 10);
    }

    #[test]
    fn extend_context_with_shuffle_scatters_after_the_current_track() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.set_shuffle(true);
        q.play_new_context((1..=4).map(item).collect(), 2);
        let current = q.current().unwrap().track_id;
        let context = q.context();
        assert!(q.extend_context_with(
            (10..=19).map(item).collect(),
            context,
            &mut rand::rngs::StdRng::seed_from_u64(7),
        ));
        assert_eq!(q.current_index(), Some(0));
        assert_eq!(
            q.current().unwrap().track_id,
            current,
            "the playing track stays"
        );
        let mut sorted = ids(&q);
        sorted.sort_unstable();
        let mut expected: Vec<u64> = (1..=4).chain(10..=19).collect();
        expected.sort_unstable();
        assert_eq!(sorted, expected);
        let tail: Vec<u64> = ids(&q).into_iter().filter(|id| *id >= 10).collect();
        assert_ne!(
            tail,
            (10..=19).collect::<Vec<_>>(),
            "seed 7 kept the late tracks in order; pick another seed"
        );
    }

    /// Shuffle + Repeat All: reaching the last track reorders the tracks before
    /// it, so the next pass differs and its first track is known in advance.
    #[test]
    fn the_last_track_of_a_shuffled_repeat_pass_reshuffles_the_next_pass() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.play_context((1..=8).map(item).collect(), 7); // flags off: in order
        q.shuffle = true;
        q.repeat = RepeatMode::All;
        let before = ids(&q);
        q.prepare_next_pass_with(&mut rand::rngs::StdRng::seed_from_u64(7));
        let after = ids(&q);
        assert_eq!(after[7], 8, "the playing last track stays last");
        assert_eq!(q.current_index(), Some(7));
        assert_ne!(
            after[..7],
            before[..7],
            "seed 7 kept the order; pick another seed"
        );
        let mut sorted = after.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, (1..=8).collect::<Vec<_>>());
        // The wrap then starts the new order, never the track that just ended.
        assert_eq!(q.peek_next_index(), Some(0));
        assert_ne!(q.items()[0].track_id, 8);
    }

    #[test]
    fn every_path_onto_the_last_track_prepares_the_next_pass() {
        fn shuffled_repeat() -> Queue {
            let mut q = Queue::default();
            q.play_context((1..=12).map(item).collect(), 10);
            q.shuffle = true;
            q.repeat = RepeatMode::All;
            q
        }
        type Path = (&'static str, fn(&mut Queue));
        let paths: [Path; 4] = [
            ("track ended", |q| {
                q.on_track_ended();
            }),
            ("next", |q| {
                q.next();
            }),
            ("skip", |q| {
                q.skip_current();
            }),
            ("jump", |q| {
                let items = q.items().to_vec();
                q.play_context(items, 11);
            }),
        ];
        for (name, path) in paths {
            let mut q = shuffled_repeat();
            let rev = q.rev();
            path(&mut q);
            assert_eq!(q.current_index(), Some(11), "{name}");
            assert!(q.rev() > rev, "{name}: the pass was not reshuffled");
        }
    }

    #[test]
    fn turning_repeat_all_on_at_the_last_track_prepares_the_next_pass() {
        let mut q = Queue::default();
        q.play_context((1..=12).map(item).collect(), 11);
        q.shuffle = true;
        let rev = q.rev();
        q.set_repeat(RepeatMode::All);
        assert!(q.rev() > rev);
    }

    #[test]
    fn no_reshuffle_without_shuffle_or_repeat_all_or_before_the_last_track() {
        for (shuffle, repeat, start) in [
            (false, RepeatMode::All, 10),
            (true, RepeatMode::Off, 10),
            (true, RepeatMode::One, 11),
            (true, RepeatMode::All, 9),
        ] {
            let mut q = Queue::default();
            q.play_context((1..=12).map(item).collect(), start);
            q.shuffle = shuffle;
            q.repeat = repeat;
            let order = ids(&q);
            q.on_track_ended();
            assert_eq!(ids(&q), order, "{shuffle} {repeat:?} {start}");
        }
    }

    #[test]
    fn the_wrap_itself_keeps_the_prepared_order() {
        let mut q = Queue::default();
        q.play_context((1..=12).map(item).collect(), 10);
        q.shuffle = true;
        q.repeat = RepeatMode::All;
        q.on_track_ended(); // onto the last track: next pass prepared
        let prepared = ids(&q);
        let first = q.items()[0].track_id;
        assert_eq!(q.on_track_ended(), Some(0));
        assert_eq!(
            ids(&q),
            prepared,
            "the preloaded first track must stay first"
        );
        assert_eq!(q.current().unwrap().track_id, first);
    }

    #[test]
    fn a_new_context_with_shuffle_off_keeps_the_order() {
        let mut q = Queue::default();
        q.play_new_context((1..=5).map(item).collect(), 2);
        assert_eq!(ids(&q), vec![1, 2, 3, 4, 5]);
        assert_eq!(q.current_index(), Some(2));
        assert!(!q.shuffle());
    }

    #[test]
    fn disabling_shuffle_keeps_the_order() {
        use rand::SeedableRng;
        let mut q = queue(6);
        q.shuffle_around_current_with(&mut rand::rngs::StdRng::seed_from_u64(3));
        let shuffled: Vec<u64> = q.items().iter().map(|i| i.track_id).collect();
        q.set_shuffle(false);
        assert!(!q.shuffle());
        assert_eq!(
            q.items().iter().map(|i| i.track_id).collect::<Vec<_>>(),
            shuffled
        );
    }

    #[test]
    fn shuffle_without_a_current_track_shuffles_everything() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.push((1..=8).map(item).collect());
        q.shuffle_around_current_with(&mut rand::rngs::StdRng::seed_from_u64(5));
        let mut ids: Vec<u64> = q.items().iter().map(|i| i.track_id).collect();
        ids.sort();
        assert_eq!(ids, (1..=8).collect::<Vec<_>>());
        assert_eq!(q.current_index(), None);
    }

    /// Review Focus: restoring a shuffled session must not re-shuffle it.
    #[test]
    fn restore_keeps_exact_order_and_starts_paused() {
        let mut q = Queue::default();
        q.restore((1..=3).map(item).collect(), 1, true, RepeatMode::All);
        assert_eq!(q.current_index(), Some(1));
        assert!(!q.is_playing());
        assert!(q.shuffle());
        assert_eq!(q.repeat(), RepeatMode::All);
        assert_eq!(
            q.items().iter().map(|i| i.track_id).collect::<Vec<_>>(),
            vec![1, 2, 3]
        );
    }

    #[test]
    fn restore_clamps_the_start_index_and_handles_empty() {
        let mut q = Queue::default();
        q.restore((1..=2).map(item).collect(), 99, false, RepeatMode::Off);
        assert_eq!(q.current_index(), Some(1));
        q.restore(Vec::new(), 0, false, RepeatMode::Off);
        assert_eq!(q.current_index(), None);
        assert!(!q.is_playing());
    }
}
