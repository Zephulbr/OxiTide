//! Ctrl+C, Ctrl+Break and closing the console window all set one flag, so a
//! run can still end the device session and delete the cloud queue. (A
//! receiver keeps a session whose controller just disappears: the bench came
//! back `joined: true` after a killed run.) On a window close Windows allows
//! about five seconds, which is enough for `endSession` and the delete.

use std::sync::atomic::{AtomicBool, Ordering};

use tokio::sync::Notify;

static FLAG: AtomicBool = AtomicBool::new(false);
static NOTIFY: Notify = Notify::const_new();

pub fn is_set() -> bool {
    FLAG.load(Ordering::SeqCst)
}

/// Resolves once an interrupt has arrived (immediately if one already has).
pub async fn wait() {
    loop {
        let notified = NOTIFY.notified();
        if is_set() {
            return;
        }
        notified.await;
    }
}

fn trigger() {
    FLAG.store(true, Ordering::SeqCst);
    NOTIFY.notify_waiters();
}

/// Starts listening for interrupts. Call once from inside the runtime.
/// `OXITIDE_PROBE_INTERRUPT_AFTER=<seconds>` triggers one on a timer, to test
/// the cleanup path without a console.
pub fn listen() {
    if let Some(secs) = std::env::var("OXITIDE_PROBE_INTERRUPT_AFTER")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
    {
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs_f64(secs)).await;
            trigger();
        });
    }
    tokio::spawn(async {
        #[cfg(windows)]
        {
            use tokio::signal::windows::{ctrl_break, ctrl_close};
            let (Ok(mut close), Ok(mut brk)) = (ctrl_close(), ctrl_break()) else {
                let _ = tokio::signal::ctrl_c().await;
                trigger();
                return;
            };
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {}
                _ = close.recv() => {}
                _ = brk.recv() => {}
            }
        }
        #[cfg(not(windows))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        trigger();
    });
}
