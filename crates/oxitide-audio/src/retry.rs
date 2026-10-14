//! The one retry policy for playback's network recovery: the resumable
//! download (`fetch::download_ahead`) and the engine's wait for the network
//! both draw their waits from [`Backoff`], and both wake early through
//! [`crate::connectivity`] when the network comes back.

use std::time::{Duration, Instant};

use crate::connectivity::{self, Connectivity};

/// Waits between recovery attempts: quick at first (a blip or a Wi-Fi roam),
/// then every 15 s. Each attempt is one cheap request, and a reported
/// network change cuts any wait short.
#[cfg(not(test))]
const NETWORK_WAITS: [Duration; 7] = [
    Duration::from_millis(200),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
    Duration::from_secs(15),
];
#[cfg(test)]
const NETWORK_WAITS: [Duration; 2] = [Duration::from_millis(1), Duration::from_millis(5)];

/// How long recovery keeps trying while the system reports the network up
/// (or cannot tell). Time the system reports the network down does not
/// count: playback waits for it as long as it takes.
#[cfg(not(test))]
pub const NETWORK_LIMIT: Duration = Duration::from_secs(5 * 60);
#[cfg(test)]
pub const NETWORK_LIMIT: Duration = Duration::from_millis(300);

/// Waits for the paced fallback stream (no disk to download ahead to): about
/// 16 s, then the engine's wait takes over.
#[cfg(not(test))]
const QUICK_WAITS: [Duration; 6] = [
    Duration::from_millis(200),
    Duration::from_millis(500),
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
    Duration::from_secs(8),
];
#[cfg(test)]
const QUICK_WAITS: [Duration; 6] = [Duration::from_millis(1); 6];

pub struct Backoff {
    waits: &'static [Duration],
    /// `None` for the quick schedule, which ends when its waits run out.
    limit: Option<Duration>,
    attempt: usize,
    /// Start of the current failing stretch; moved forward while the system
    /// reports the network down.
    since: Instant,
}

impl Backoff {
    /// Recovery from a lost network, bounded by [`NETWORK_LIMIT`].
    pub fn network() -> Self {
        Self { waits: &NETWORK_WAITS, limit: Some(NETWORK_LIMIT), attempt: 0, since: Instant::now() }
    }

    /// A short, fixed number of retries.
    pub fn quick() -> Self {
        Self { waits: &QUICK_WAITS, limit: None, attempt: 0, since: Instant::now() }
    }

    /// The failed attempts so far.
    pub fn attempts(&self) -> usize {
        self.attempt
    }

    /// The wait before the next attempt, or `None` when the policy is spent.
    pub fn next_wait(&mut self) -> Option<Duration> {
        let wait = match self.limit {
            Some(limit) => {
                if connectivity::global().is_offline() {
                    // Waiting on a network that is known to be down is not
                    // failing; the limit counts from when it is back.
                    self.since = Instant::now();
                } else if self.since.elapsed() >= limit {
                    return None;
                }
                self.waits[self.attempt.min(self.waits.len() - 1)]
            }
            None => *self.waits.get(self.attempt)?,
        };
        self.attempt += 1;
        Some(wait)
    }

    /// Sleep before the next attempt: `wait`, cut short when the network
    /// comes back; a network backoff also keeps waiting while the network is
    /// known to be down. False when `cancelled` reports true first.
    pub fn sleep(&self, wait: Duration, cancelled: impl Fn() -> bool) -> bool {
        sleep_on(connectivity::global(), wait, self.limit.is_some(), cancelled)
    }
}

/// [`Backoff::sleep`] for a network backoff.
pub fn network_sleep(wait: Duration, cancelled: impl Fn() -> bool) -> bool {
    sleep_on(connectivity::global(), wait, true, cancelled)
}

/// While the network is known to be down (and `hold` is set), keep waiting
/// for it, re-checking at most every 30 s in case the report is stale.
const OFFLINE_RECHECK: Duration = Duration::from_secs(30);

fn sleep_on(conn: &Connectivity, wait: Duration, hold: bool, cancelled: impl Fn() -> bool) -> bool {
    let start_epoch = conn.epoch();
    let deadline = Instant::now() + wait;
    let recheck = deadline + OFFLINE_RECHECK;
    loop {
        if cancelled() {
            return false;
        }
        // The network changed and is not known to be down: try now.
        if conn.epoch() != start_epoch && !conn.is_offline() {
            return true;
        }
        let now = Instant::now();
        let holding = hold && conn.is_offline();
        if (now >= deadline && !holding) || now >= recheck {
            return true;
        }
        // Park until the deadline (or the recheck while holding), waking on
        // a network change and at least every 50 ms for `cancelled`.
        let until = if holding { recheck } else { deadline };
        conn.wait_change((until - now).min(Duration::from_millis(50)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_backoff_ends_after_its_waits() {
        let mut backoff = Backoff::quick();
        let waits: Vec<_> = std::iter::from_fn(|| backoff.next_wait()).collect();
        assert_eq!(waits.len(), QUICK_WAITS.len());
    }

    #[test]
    fn network_backoff_repeats_its_last_wait_until_the_limit() {
        let mut backoff = Backoff::network();
        for _ in 0..10 {
            assert!(backoff.next_wait().is_some());
        }
        assert_eq!(backoff.attempts(), 10);
        std::thread::sleep(NETWORK_LIMIT + Duration::from_millis(20));
        if !connectivity::global().is_offline() {
            assert!(backoff.next_wait().is_none(), "the limit did not end the backoff");
        }
    }

    #[test]
    fn a_network_change_cuts_a_wait_short() {
        let conn = Connectivity::new();
        conn.set_online(Some(false));
        let started = Instant::now();
        let waker = {
            let conn = conn.clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_millis(50));
                conn.set_online(Some(true));
            })
        };
        assert!(sleep_on(&conn, Duration::from_secs(10), true, || false));
        assert!(started.elapsed() < Duration::from_secs(2), "the wait was not cut short");
        waker.join().unwrap();
    }

    #[test]
    fn a_known_offline_network_holds_the_wait() {
        let conn = Connectivity::new();
        conn.set_online(Some(false));
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sleeper = {
            let (conn, done) = (conn.clone(), done.clone());
            std::thread::spawn(move || {
                sleep_on(&conn, Duration::from_millis(10), true, || false);
                done.store(true, std::sync::atomic::Ordering::SeqCst);
            })
        };
        std::thread::sleep(Duration::from_millis(150));
        assert!(!done.load(std::sync::atomic::Ordering::SeqCst), "waited out the timer while offline");
        conn.set_online(Some(true));
        sleeper.join().unwrap();
        assert!(done.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn cancel_ends_a_wait() {
        let conn = Connectivity::new();
        assert!(!sleep_on(&conn, Duration::from_secs(10), true, || true));
    }
}
