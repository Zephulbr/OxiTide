use std::collections::VecDeque;

use oxitide_audio::queue::QueueItem;

pub const QUEUE_CAP: usize = 1_000;

#[derive(Debug, Clone, PartialEq)]
pub struct QueuedScrobble {
    pub item: QueueItem,
    pub started_at: i64,
}

#[derive(Default)]
pub struct ScrobbleQueue {
    items: VecDeque<QueuedScrobble>,
}

impl ScrobbleQueue {
    pub fn push(&mut self, entry: QueuedScrobble) {
        let duplicate = self
            .items
            .iter()
            .any(|e| e.item.track_id == entry.item.track_id && e.started_at == entry.started_at);
        if duplicate {
            return;
        }
        self.items.push_back(entry);
        if self.items.len() > QUEUE_CAP {
            let dropped = self.items.len() - QUEUE_CAP;
            for _ in 0..dropped {
                self.items.pop_front();
            }
            tracing::warn!(dropped, "scrobble queue full; dropped oldest queued scrobble(s)");
        }
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn front_batch(&self, count: usize) -> Vec<QueuedScrobble> {
        self.items.iter().take(count).cloned().collect()
    }

    pub fn remove_front(&mut self, count: usize) {
        for _ in 0..count {
            self.items.pop_front();
        }
    }

    pub fn clear(&mut self) {
        self.items.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(id: u64, at: i64) -> QueuedScrobble {
        QueuedScrobble {
            item: QueueItem {
                track_id: id,
                title: "T".into(),
                artist: "A".into(),
                album: String::new(),
                duration_secs: 200,
                cover: None,
            },
            started_at: at,
        }
    }

    #[test]
    fn dedupes_same_track_and_timestamp() {
        let mut queue = ScrobbleQueue::default();
        queue.push(entry(1, 100));
        queue.push(entry(1, 100));
        assert_eq!(queue.len(), 1);
        // A replay (same id, new timestamp) is a distinct scrobble.
        queue.push(entry(1, 200));
        assert_eq!(queue.len(), 2);
    }

    #[test]
    fn caps_by_dropping_oldest() {
        let mut queue = ScrobbleQueue::default();
        for id in 0..(QUEUE_CAP as u64 + 5) {
            queue.push(entry(id + 1, id as i64));
        }
        assert_eq!(queue.len(), QUEUE_CAP);
        assert_eq!(
            queue.front_batch(1)[0].item.track_id,
            6,
            "oldest entries should have been dropped"
        );
    }

    #[test]
    fn front_batch_and_remove_front_are_fifo() {
        let mut queue = ScrobbleQueue::default();
        queue.push(entry(1, 1));
        queue.push(entry(2, 2));
        queue.push(entry(3, 3));
        let batch = queue.front_batch(2);
        assert_eq!(batch.iter().map(|e| e.item.track_id).collect::<Vec<_>>(), vec![1, 2]);
        queue.remove_front(2);
        assert_eq!(queue.front_batch(1)[0].item.track_id, 3);
        queue.clear();
        assert!(queue.is_empty());
    }
}
