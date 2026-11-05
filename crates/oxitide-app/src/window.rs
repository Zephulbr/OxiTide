//! Window geometry persistence: capture size/position/maximized state into
//! `config.toml` and restore it with an on-screen check.

use tauri::{AppHandle, LogicalSize, Manager, PhysicalPosition, PhysicalSize, Size, WebviewWindow};

use crate::config::Config;
use crate::state::AppState;

/// Show, unminimize and focus the main window (tray, second launch, MPRIS
/// Raise). On Linux a window shown from hidden gets its last full size back:
/// GTK on KWin Wayland can remap it at a stale, work-area size (2026-11-05
/// Linux run, L5).
pub fn show_main(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let was_hidden = !window.is_visible().unwrap_or(true);
    if was_hidden {
        mark_shown();
    }
    let _ = window.show();
    let _ = window.unminimize();
    if was_hidden {
        restore_last_size(app, &window);
    }
    let _ = window.set_focus();
}

/// When the window was last shown from hidden. The compositor's configure
/// for the remapped window arrives after `show` returns and can carry the
/// stale size, so resizes shortly after are neither recorded nor final.
static SHOWN_AT: std::sync::Mutex<Option<std::time::Instant>> = std::sync::Mutex::new(None);
const REMAP_SETTLE: std::time::Duration = std::time::Duration::from_millis(1500);

fn mark_shown() {
    *SHOWN_AT.lock().unwrap() = Some(std::time::Instant::now());
}

fn settling() -> bool {
    SHOWN_AT.lock().unwrap().is_some_and(|at| at.elapsed() < REMAP_SETTLE)
}

#[cfg(target_os = "linux")]
fn restore_last_size(app: &AppHandle, window: &WebviewWindow) {
    let state = app.state::<AppState>();
    if state.pre_mini().is_some() || window.is_maximized().unwrap_or(false) {
        tracing::debug!(target: "window", "show: mini or maximized; size left alone");
        return;
    }
    let Some((width, height)) = state.last_size() else {
        tracing::debug!(target: "window", "show: no size recorded yet");
        return;
    };
    tracing::debug!(target: "window", "show: restoring {width}x{height}");
    let _ = window.set_size(stored_size(width, height));
    // Again once the compositor has mapped the window: its first configure
    // can come after the call above and win.
    let window = window.clone();
    std::thread::spawn(move || {
        for delay in [150u64, 600] {
            std::thread::sleep(std::time::Duration::from_millis(delay));
            if !window.is_visible().unwrap_or(false) || window.is_maximized().unwrap_or(true) {
                return;
            }
            let now = window.inner_size().ok().map(|size| stored_extent(&window, size));
            if now.is_some_and(|now| now != (width, height)) {
                tracing::debug!(target: "window", "show: size is {now:?} after {delay} ms; restoring {width}x{height}");
                let _ = window.set_size(stored_size(width, height));
            }
        }
    });
}

#[cfg(not(target_os = "linux"))]
fn restore_last_size(_app: &AppHandle, _window: &WebviewWindow) {}

/// Linux: remember a compositor-given size of the full, unmaximized, visible
/// window (see [`show_main`]). Ignored in the mini player and while a
/// remapped window settles.
pub fn note_resized(app: &AppHandle, size: PhysicalSize<u32>) {
    if !cfg!(target_os = "linux") {
        return;
    }
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let full = state.pre_mini().is_none();
    let visible = window.is_visible().unwrap_or(false);
    let maximized = window.is_maximized().unwrap_or(true);
    let minimized = window.is_minimized().unwrap_or(true);
    let settling = settling();
    let extent = stored_extent(&window, size);
    let record = full && visible && !maximized && !minimized && !settling && size.width > 0 && size.height > 0;
    tracing::debug!(target: "window",
        "resized {}x{} (logical {extent:?}) visible={visible} maximized={maximized} minimized={minimized} settling={settling} recorded={record}",
        size.width, size.height);
    if record {
        state.set_last_size(extent);
    }
}

/// A monitor's work area in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MonitorRect {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// Minimum visible window area for a saved position to be restored.
const MIN_VISIBLE_WIDTH: i64 = 200;
const MIN_VISIBLE_HEIGHT: i64 = 100;

/// Mini-player and full-window size limits (spec §5.1).
pub const MINI_WIDTH: u32 = 300;
pub const MINI_HEIGHT: u32 = 430;
pub const MINI_MIN_WIDTH: u32 = 260;
pub const MINI_MIN_HEIGHT: u32 = 360;
/// The windowed minimum from `tauri.conf.json`; re-applied on mini exit.
pub const FULL_MIN_WIDTH: u32 = 720;
pub const FULL_MIN_HEIGHT: u32 = 520;

/// The full-window geometry stashed while the mini player is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FullGeometry {
    pub width: u32,
    pub height: u32,
    pub x: Option<i32>,
    pub y: Option<i32>,
    pub maximized: bool,
}

/// The geometry to persist on close: the pre-mini stash when mini is active,
/// otherwise the live window (the caller supplies the captured live state).
pub fn geometry_to_save(pre_mini: Option<FullGeometry>, live: FullGeometry) -> FullGeometry {
    pre_mini.unwrap_or(live)
}

/// The stash after a mini-mode request (MP-D2): `run` performs the window op
/// and the stash changes only when it succeeds, so a failed op leaves the
/// stash authoritative (enter: empty so a retry resizes; exit: intact for a
/// retry and for close-time capture, MP-D7). A repeated enter is a no-op
/// (§5.3). `captured` is the geometry to stash on enter; `None` on exit.
pub fn mini_stash_after<E>(
    active: bool,
    current: Option<FullGeometry>,
    captured: Option<FullGeometry>,
    run: impl FnOnce() -> Result<(), E>,
) -> Result<Option<FullGeometry>, E> {
    if active && current.is_some() {
        return Ok(current);
    }
    run()?;
    Ok(if active { captured } else { None })
}

/// The window size in the units it is stored in: physical pixels on Windows,
/// logical pixels on Linux. GTK only has integer scale factors, so on a
/// fractional display (1.75) it reports scale 2 and physical sizes twice the
/// logical ones; stored that way, a scale change restored the wrong size.
#[cfg(target_os = "linux")]
fn stored_extent(window: &WebviewWindow, size: PhysicalSize<u32>) -> (u32, u32) {
    let logical = size.to_logical::<f64>(window.scale_factor().unwrap_or(1.0));
    (logical.width.round() as u32, logical.height.round() as u32)
}

#[cfg(not(target_os = "linux"))]
fn stored_extent(_window: &WebviewWindow, size: PhysicalSize<u32>) -> (u32, u32) {
    (size.width, size.height)
}

/// A stored size (see [`stored_extent`]) as a size to apply.
#[cfg(target_os = "linux")]
fn stored_size(width: u32, height: u32) -> Size {
    Size::Logical(LogicalSize::new(width as f64, height as f64))
}

#[cfg(not(target_os = "linux"))]
fn stored_size(width: u32, height: u32) -> Size {
    Size::Physical(PhysicalSize::new(width, height))
}

/// Read the live window geometry for the pre-mini stash.
pub fn read_geometry(window: &WebviewWindow) -> tauri::Result<FullGeometry> {
    let (width, height) = stored_extent(window, window.inner_size()?);
    let position = window.outer_position().ok();
    Ok(FullGeometry {
        width,
        height,
        x: position.map(|p| p.x),
        y: position.map(|p| p.y),
        maximized: window.is_maximized().unwrap_or(false),
    })
}

/// Apply a stashed geometry: size, position when known, then maximize.
pub fn apply_geometry(window: &WebviewWindow, geometry: FullGeometry) -> tauri::Result<()> {
    window.set_size(stored_size(geometry.width, geometry.height))?;
    if let (Some(x), Some(y)) = (geometry.x, geometry.y) {
        window.set_position(PhysicalPosition::new(x, y))?;
    }
    if geometry.maximized {
        window.maximize()?;
    }
    Ok(())
}

/// Enter the mini window. Unmaximizing first keeps the stash authoritative
/// (MP-D11): exit restores the maximized state from the stash.
pub fn enter_mini(window: &WebviewWindow, stash: FullGeometry) -> tauri::Result<()> {
    if stash.maximized {
        window.unmaximize()?;
    }
    window.set_min_size(Some(Size::Logical(LogicalSize::new(
        MINI_MIN_WIDTH as f64,
        MINI_MIN_HEIGHT as f64,
    ))))?;
    window.set_resizable(true)?;
    window.set_size(LogicalSize::new(MINI_WIDTH as f64, MINI_HEIGHT as f64))?;
    Ok(())
}

/// Leave the mini window: restore the full minimum, then the stash when there
/// is one (a defensive `None` leaves size/position untouched).
pub fn exit_mini(window: &WebviewWindow, stash: Option<FullGeometry>) -> tauri::Result<()> {
    window.set_min_size(Some(Size::Logical(LogicalSize::new(
        FULL_MIN_WIDTH as f64,
        FULL_MIN_HEIGHT as f64,
    ))))?;
    window.set_resizable(true)?;
    if let Some(geometry) = stash {
        apply_geometry(window, geometry)?;
    }
    Ok(())
}

/// Returns `(x, y)` when at least a 200x100 corner of the window intersects a
/// monitor's work area, else `None`.
pub fn visible_position(
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    monitors: &[MonitorRect],
) -> Option<(i32, i32)> {
    let visible = monitors.iter().any(|m| {
        let overlap_w =
            (x as i64 + width as i64).min(m.x as i64 + m.width as i64) - (x.max(m.x) as i64);
        let overlap_h =
            (y as i64 + height as i64).min(m.y as i64 + m.height as i64) - (y.max(m.y) as i64);
        overlap_w >= MIN_VISIBLE_WIDTH && overlap_h >= MIN_VISIBLE_HEIGHT
    });
    visible.then_some((x, y))
}

/// Apply the saved geometry. Maximized wins; a missing saved position leaves
/// the size from `tauri.conf.json` untouched (first run); an off-screen
/// position is ignored but the size is still restored.
pub fn apply(window: &WebviewWindow, config: &Config) -> tauri::Result<()> {
    let ui = &config.ui;
    if ui.window_maximized {
        return window.maximize();
    }
    if ui.window_x.is_none() && ui.window_y.is_none() {
        return Ok(());
    }
    let size = stored_size(ui.window_width, ui.window_height);
    window.set_size(size)?;
    if let (Some(x), Some(y)) = (ui.window_x, ui.window_y) {
        let extent = size.to_physical::<u32>(window.scale_factor()?);
        let monitors: Vec<MonitorRect> = window
            .available_monitors()?
            .iter()
            .map(|m| {
                let work = m.work_area();
                MonitorRect {
                    x: work.position.x,
                    y: work.position.y,
                    width: work.size.width,
                    height: work.size.height,
                }
            })
            .collect();
        if visible_position(x, y, extent.width, extent.height, &monitors).is_some() {
            window.set_position(PhysicalPosition::new(x, y))?;
        }
    }
    Ok(())
}

/// Read the window's current geometry into `config`: the position in
/// physical pixels, the size in its stored units (see [`stored_extent`]).
pub fn capture(window: &WebviewWindow, config: &mut Config) {
    // A minimized window reports ~(-32000, -32000) and a bogus size; keep the
    // last known values instead of poisoning the saved geometry.
    if window.is_minimized().unwrap_or(false) {
        return;
    }
    if let Ok(position) = window.outer_position() {
        config.ui.window_x = Some(position.x);
        config.ui.window_y = Some(position.y);
    }
    if let Ok(size) = window.inner_size() {
        (config.ui.window_width, config.ui.window_height) = stored_extent(window, size);
    }
    if let Ok(maximized) = window.is_maximized() {
        config.ui.window_maximized = maximized;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn monitor(x: i32, y: i32, width: u32, height: u32) -> MonitorRect {
        MonitorRect { x, y, width, height }
    }

    #[test]
    fn a_fully_visible_position_is_restored() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(100, 100, 1200, 800, &monitors), Some((100, 100)));
    }

    #[test]
    fn an_offscreen_position_is_rejected() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(2500, 200, 1200, 800, &monitors), None);
        assert_eq!(visible_position(-1500, 200, 1200, 800, &monitors), None);
    }

    #[test]
    fn a_partially_visible_position_is_restored_when_enough_shows() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(1700, 900, 1200, 800, &monitors), Some((1700, 900)));
    }

    #[test]
    fn a_sliver_of_visibility_is_rejected() {
        let monitors = [monitor(0, 0, 1920, 1080)];
        assert_eq!(visible_position(1900, 1060, 1200, 800, &monitors), None);
        assert_eq!(visible_position(-1150, -750, 1200, 800, &monitors), None);
    }

    /// Review Focus: no monitors reported must ignore the position, not panic.
    #[test]
    fn no_monitors_rejects_every_position() {
        assert_eq!(visible_position(0, 0, 1200, 800, &[]), None);
    }

    #[test]
    fn a_second_monitor_counts() {
        let monitors = [monitor(0, 0, 1920, 1080), monitor(1920, 0, 2560, 1440)];
        assert_eq!(visible_position(2000, 100, 1200, 800, &monitors), Some((2000, 100)));
    }

    #[test]
    fn geometry_to_save_prefers_the_stash() {
        let live = FullGeometry {
            width: 1200,
            height: 800,
            x: Some(10),
            y: Some(20),
            maximized: false,
        };
        let stash = FullGeometry {
            width: 1000,
            height: 700,
            x: Some(30),
            y: Some(40),
            maximized: true,
        };
        assert_eq!(geometry_to_save(Some(stash), live), stash);
        assert_eq!(geometry_to_save(None, live), live);
    }

    #[test]
    fn mini_stash_after_commits_only_when_the_op_succeeds() {
        let captured = FullGeometry {
            width: 1200,
            height: 800,
            x: Some(10),
            y: Some(20),
            maximized: false,
        };

        // Enter success commits the captured geometry.
        assert_eq!(
            mini_stash_after(true, None, Some(captured), || Ok::<(), &str>(())).unwrap(),
            Some(captured)
        );
        // Enter failure commits nothing.
        assert!(mini_stash_after(true, None, Some(captured), || Err::<(), &str>("nope")).is_err());
        // Exit success clears the stash.
        assert_eq!(
            mini_stash_after(false, Some(captured), None, || Ok::<(), &str>(())).unwrap(),
            None
        );
        // Exit failure returns Err, so the caller never clears the stash.
        assert!(mini_stash_after(false, Some(captured), None, || Err::<(), &str>("nope")).is_err());
    }

    #[test]
    fn mini_stash_after_skips_a_repeated_enter() {
        let current = FullGeometry {
            width: 1,
            height: 2,
            x: None,
            y: None,
            maximized: true,
        };
        let captured = FullGeometry {
            width: 3,
            height: 4,
            x: None,
            y: None,
            maximized: false,
        };
        let called = std::cell::Cell::new(false);
        let next = mini_stash_after(true, Some(current), Some(captured), || {
            called.set(true);
            Ok::<(), &str>(())
        })
        .unwrap();
        assert_eq!(next, Some(current));
        assert!(!called.get());
    }
}
