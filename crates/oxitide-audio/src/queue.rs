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
    items: Vec<QueueItem>,
    current: Option<usize>,
    playing: bool,
    shuffle: bool,
    repeat: RepeatMode,
    volume: f32,
}

impl Default for Queue {
    fn default() -> Self {
        Self {
            items: Vec::new(),
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
        self.items = items;
        if self.items.is_empty() {
            self.current = None;
            self.playing = false;
        } else {
            self.current = Some(start.min(self.items.len() - 1));
            self.playing = true;
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

    pub fn reorder(&mut self, from: usize, to: usize) -> bool {
        if from >= self.items.len() || to >= self.items.len() || from == to {
            return false;
        }
        let item = self.items.remove(from);
        self.items.insert(to, item);
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
        self.items.remove(index);
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
        self.items.clear();
        self.current = None;
        self.playing = false;
    }

    /// Append items without touching the current track or the playing state.
    pub fn push(&mut self, items: Vec<QueueItem>) {
        self.items.extend(items);
    }

    /// Stop playback without clearing the queue (e.g. after a playback error).
    pub fn stop(&mut self) {
        self.playing = false;
    }

    pub fn set_volume(&mut self, v: f32) {
        self.volume = v.clamp(0.0, 1.0);
    }

    pub fn set_shuffle(&mut self, on: bool) {
        self.shuffle = on;
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) {
        self.repeat = mode;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(id: u64) -> QueueItem {
        QueueItem { track_id: id, title: format!("T{id}"), artist: String::new(), album: String::new(), duration_secs: 1, cover: None }
    }

    fn queue(n: u64) -> Queue {
        let mut q = Queue::default();
        q.play_context((1..=n).map(item).collect(), 0);
        q
    }

    #[test]
    fn play_context_sets_current_and_playing() {
        let q = queue(3);
        assert_eq!(q.current_index(), Some(0));
        assert!(q.is_playing());
        assert_eq!(q.current().unwrap().track_id, 1);
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
    fn volume_is_clamped() {
        let mut q = Queue::default();
        q.set_volume(2.0);
        assert_eq!(q.volume(), 1.0);
        q.set_volume(-1.0);
        assert_eq!(q.volume(), 0.0);
    }
}
