use std::sync::Arc;

use rand::seq::SliceRandom;
use rand::Rng;
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
    current: Option<usize>,
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
            current: None,
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

    pub fn play_context(&mut self, items: Vec<QueueItem>, start: usize) {
        self.replace_items(items);
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(start.min(self.items.len() - 1));
            self.playing = true;
        }
    }

    /// Start a fully shuffled context: the whole list (including its first
    /// item) is shuffled once, the shuffle flag turns on, and playback starts
    /// at the top of the shuffled order.
    pub fn play_context_shuffled(&mut self, items: Vec<QueueItem>) {
        self.play_context_shuffled_with(items, &mut rand::rng());
    }

    /// One Fisher–Yates shuffle of the whole list. Seeded by tests via `StdRng`.
    fn play_context_shuffled_with<R: Rng + ?Sized>(&mut self, mut items: Vec<QueueItem>, rng: &mut R) {
        items.shuffle(rng);
        self.replace_items(items);
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
    /// tail), the start index is clamped, and playback starts paused.
    pub fn restore(
        &mut self,
        items: Vec<QueueItem>,
        start: usize,
        shuffle: bool,
        repeat: RepeatMode,
    ) {
        self.replace_items(items);
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
        let Some(cur) = self.current else { return false };
        if self.items.is_empty() {
            return false;
        }
        if cur + 1 < self.items.len() {
            self.current = Some(cur + 1);
            self.playing = true;
            true
        } else if self.repeat == RepeatMode::All {
            self.current = Some(0);
            self.playing = true;
            true
        } else {
            self.playing = false;
            false
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub fn next(&mut self) -> bool {
        self.advance()
    }

    pub fn prev(&mut self) -> bool {
        let Some(cur) = self.current else { return false };
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
                self.current = Some(index);
                self.playing = true;
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
            Some(cur + 1)
        } else if self.repeat == RepeatMode::All {
            self.current = Some(0);
            self.playing = true;
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
        self.current = None;
        self.playing = false;
    }

    /// Append items without touching the current track or the playing state.
    pub fn push(&mut self, items: Vec<QueueItem>) {
        self.items_mut().extend(items);
    }

    /// Insert items right after the current track ("Play next"), keeping
    /// their order; with no current track they append. The current track and
    /// the playing state are untouched.
    pub fn insert_next(&mut self, items: Vec<QueueItem>) {
        match self.current {
            Some(cur) => {
                let at = (cur + 1).min(self.items.len());
                self.items_mut().splice(at..at, items);
            }
            None => self.items_mut().extend(items),
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

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    /// Enabling shuffles the items after the current index once; disabling
    /// leaves the shuffled order in place. The current item never moves.
    pub fn set_shuffle(&mut self, on: bool) {
        if on {
            self.shuffle_tail_with(&mut rand::rng());
        }
        self.shuffle = on;
    }

    /// One Fisher–Yates shuffle of the tail. Seeded by tests via `StdRng`.
    fn shuffle_tail_with<R: Rng + ?Sized>(&mut self, rng: &mut R) {
        match self.current {
            Some(current) if current + 1 < self.items.len() => {
                self.items_mut()[current + 1..].shuffle(rng);
            }
            Some(_) => {}
            None => self.items_mut().shuffle(rng),
        }
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64) -> QueueItem {
        QueueItem { track_id: id, title: format!("T{id}"), artist: String::new(), album: String::new(), duration_secs: 1, cover: None, artist_refs: Vec::new(), album_id: None, autoplay: false }
    }

    #[test]
    fn autoplay_flag_is_written_only_when_set() {
        let plain = serde_json::to_value(item(1)).unwrap();
        assert!(plain.get("autoplay").is_none());
        let suggested = QueueItem { autoplay: true, ..item(1) };
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
        assert_eq!(q.rev(), rev, "navigation and flags must not change the revision");
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
        assert!(Arc::ptr_eq(&snapshot, &q.shared_items()), "a snapshot must not copy");
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
    }

    #[test]
    fn set_shuffle_moves_only_the_tail() {
        use rand::SeedableRng;
        let mut q = queue(8);
        q.next(); // current = index 1
        let head: Vec<u64> = q.items()[..=1].iter().map(|i| i.track_id).collect();
        let tail_before: Vec<u64> = q.items()[2..].iter().map(|i| i.track_id).collect();
        q.shuffle_tail_with(&mut rand::rngs::StdRng::seed_from_u64(7));
        let head_after: Vec<u64> = q.items()[..=1].iter().map(|i| i.track_id).collect();
        let tail_after: Vec<u64> = q.items()[2..].iter().map(|i| i.track_id).collect();
        assert_eq!(head_after, head, "shuffle moved the current item or the head");
        let mut sorted_before = tail_before.clone();
        sorted_before.sort();
        let mut sorted_after = tail_after.clone();
        sorted_after.sort();
        assert_eq!(sorted_after, sorted_before, "shuffle changed the tail's items");
        assert_ne!(
            tail_after, tail_before,
            "seed 7 produced the identity order; pick another seed"
        );
    }

    #[test]
    fn set_shuffle_flag_and_tail() {
        let mut q = queue(8);
        q.next();
        let head: Vec<u64> = q.items()[..=1].iter().map(|i| i.track_id).collect();
        q.set_shuffle(true);
        assert!(q.shuffle());
        assert_eq!(q.items()[..=1].iter().map(|i| i.track_id).collect::<Vec<_>>(), head);
        assert_eq!(q.current().unwrap().track_id, 2);
    }

    #[test]
    fn disabling_shuffle_keeps_the_order() {
        use rand::SeedableRng;
        let mut q = queue(6);
        q.shuffle_tail_with(&mut rand::rngs::StdRng::seed_from_u64(3));
        let shuffled: Vec<u64> = q.items().iter().map(|i| i.track_id).collect();
        q.set_shuffle(false);
        assert!(!q.shuffle());
        assert_eq!(q.items().iter().map(|i| i.track_id).collect::<Vec<_>>(), shuffled);
    }

    #[test]
    fn shuffle_without_a_current_track_shuffles_everything() {
        use rand::SeedableRng;
        let mut q = Queue::default();
        q.push((1..=8).map(item).collect());
        q.shuffle_tail_with(&mut rand::rngs::StdRng::seed_from_u64(5));
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
