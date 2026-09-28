//! Window geometry persistence: capture size/position/maximized state into
//! `config.toml` and restore it with an on-screen check.

use tauri::{PhysicalPosition, PhysicalSize, Size, WebviewWindow};

use crate::config::Config;

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

/// Read the live window geometry for the pre-mini stash.
pub fn read_geometry(window: &WebviewWindow) -> tauri::Result<FullGeometry> {
    let size = window.inner_size()?;
    let position = window.outer_position().ok();
    Ok(FullGeometry {
        width: size.width,
        height: size.height,
        x: position.map(|p| p.x),
        y: position.map(|p| p.y),
        maximized: window.is_maximized().unwrap_or(false),
    })
}

/// Apply a stashed geometry: size, position when known, then maximize.
pub fn apply_geometry(window: &WebviewWindow, geometry: FullGeometry) -> tauri::Result<()> {
    window.set_size(PhysicalSize::new(geometry.width, geometry.height))?;
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
    window.set_min_size(Some(Size::Physical(PhysicalSize::new(
        MINI_MIN_WIDTH,
        MINI_MIN_HEIGHT,
    ))))?;
    window.set_resizable(true)?;
    window.set_size(PhysicalSize::new(MINI_WIDTH, MINI_HEIGHT))?;
    Ok(())
}

/// Leave the mini window: restore the full minimum, then the stash when there
/// is one (a defensive `None` leaves size/position untouched).
pub fn exit_mini(window: &WebviewWindow, stash: Option<FullGeometry>) -> tauri::Result<()> {
    window.set_min_size(Some(Size::Physical(PhysicalSize::new(
        FULL_MIN_WIDTH,
        FULL_MIN_HEIGHT,
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
    window.set_size(PhysicalSize::new(ui.window_width, ui.window_height))?;
    if let (Some(x), Some(y)) = (ui.window_x, ui.window_y) {
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
        if visible_position(x, y, ui.window_width, ui.window_height, &monitors).is_some() {
            window.set_position(PhysicalPosition::new(x, y))?;
        }
    }
    Ok(())
}

/// Read the window's current geometry into `config` (physical pixels).
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
        config.ui.window_width = size.width;
        config.ui.window_height = size.height;
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
}
