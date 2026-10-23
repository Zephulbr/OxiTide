//! Routes panics into the `tracing` log. The Windows GUI build has no console,
//! so without this a panic's message vanished with the thread (or the process).

use std::backtrace::Backtrace;
use std::panic::Location;
use std::sync::Mutex;

use tracing_appender::non_blocking::WorkerGuard;

/// The file writer's guard. Dropping it flushes the queued lines and stops the
/// writer thread, which a panic that ends the process does first.
pub static LOG_GUARD: Mutex<Option<WorkerGuard>> = Mutex::new(None);

/// Logs every panic (message, location, thread, backtrace) at `error`, then
/// runs the previous hook (the default prints to stderr). A main-thread panic
/// ends the process, so it also flushes the non-blocking file writer, which
/// would otherwise lose the report.
pub fn install() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let thread = std::thread::current();
        let name = thread.name().unwrap_or("<unnamed>");
        let report = report(
            name,
            info.location(),
            info.payload(),
            &Backtrace::force_capture(),
        );
        tracing::error!(target: "panic", "{report}");
        if name == "main" {
            // `try_lock`: a panic while the guard is held must not deadlock.
            if let Ok(mut guard) = LOG_GUARD.try_lock() {
                drop(guard.take());
            }
        }
        previous(info);
    }));
}

fn report(
    thread: &str,
    location: Option<&Location<'_>>,
    payload: &(dyn std::any::Any + Send),
    backtrace: &Backtrace,
) -> String {
    let location = location
        .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()))
        .unwrap_or_else(|| "<unknown location>".into());
    format!(
        "thread '{thread}' panicked at {location}: {}\nbacktrace:\n{backtrace}",
        message(payload)
    )
}

/// The panic message: `panic!` payloads are `&str` or `String`.
fn message(payload: &(dyn std::any::Any + Send)) -> &str {
    if let Some(s) = payload.downcast_ref::<&str>() {
        s
    } else if let Some(s) = payload.downcast_ref::<String>() {
        s
    } else {
        "<non-string panic payload>"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_reads_str_and_string_payloads() {
        let literal: Box<dyn std::any::Any + Send> = Box::new("boom");
        let formatted: Box<dyn std::any::Any + Send> = Box::new(format!("bad {}", 7));
        let other: Box<dyn std::any::Any + Send> = Box::new(42_u32);
        assert_eq!(message(literal.as_ref()), "boom");
        assert_eq!(message(formatted.as_ref()), "bad 7");
        assert_eq!(message(other.as_ref()), "<non-string panic payload>");
    }

    #[test]
    fn report_names_thread_location_and_message() {
        let payload: Box<dyn std::any::Any + Send> = Box::new(format!("ring overflow {}", 3));
        let here = Location::caller();
        let text = report(
            "worker",
            Some(here),
            payload.as_ref(),
            &Backtrace::disabled(),
        );
        let expected = format!(
            "thread 'worker' panicked at {}:{}:{}: ring overflow 3\n",
            here.file(),
            here.line(),
            here.column()
        );
        assert!(text.starts_with(&expected), "{text}");

        let text = report("worker", None, payload.as_ref(), &Backtrace::disabled());
        assert!(
            text.contains("at <unknown location>: ring overflow 3"),
            "{text}"
        );
    }
}
