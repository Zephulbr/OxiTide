//! Custom window chrome. On Windows the main window runs undecorated and the UI
//! draws its own caption buttons (`ui/src/lib/components/WindowControls.svelte`)
//! and drag band; other platforms keep the system frame.
//!
//! Windows 11 only opens the Snap Layouts flyout when the window answers
//! `WM_NCHITTEST` with `HTMAXBUTTON`, and the WebView2 child swallows every hit
//! test over the client area. So a transparent native child window sits over
//! the HTML maximize button and answers for it, forwarding hover and clicks to
//! the UI as `titlebar://snap` events (`enter` / `leave` / `down` / `click`).
//! The UI reports where its button is with `set_snap_target` (CSS pixels,
//! anchored to the right edge so a resize never leaves the overlay stale), or
//! `None` to hide it (mini player). Approach adapted from tauri-plugin-frame
//! (MIT).

use serde::Deserialize;

/// The HTML maximize button's box in CSS pixels. `right` is the distance from
/// the window's right edge to the button's left edge.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
pub struct SnapTarget {
    pub right: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

/// Overlay rectangle in physical client pixels for a client width and DPI.
#[cfg_attr(not(windows), allow(dead_code))]
fn overlay_rect(target: SnapTarget, client_width: i32, dpi: u32) -> (i32, i32, i32, i32) {
    let scale = f64::from(dpi) / 96.0;
    let px = |v: f64| (v * scale).round() as i32;
    let width = px(target.width).max(1);
    let height = px(target.height).max(1);
    (client_width - px(target.right), px(target.top), width, height)
}

/// Whether this platform uses the custom chrome.
pub fn custom() -> bool {
    cfg!(windows)
}

#[cfg(windows)]
pub use imp::{install, set_target};

#[cfg(not(windows))]
pub fn install(_window: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
pub fn set_target(_target: Option<SnapTarget>) {}

#[cfg(windows)]
mod imp {
    use std::sync::{Mutex, OnceLock};

    use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
    use windows::core::{w, PCWSTR};
    use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
    use windows::Win32::Graphics::Gdi::{GetStockObject, HBRUSH, NULL_BRUSH};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        TrackMouseEvent, TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT,
    };
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DefWindowProcW, GetClientRect, RegisterClassExW, SetWindowPos,
        HTMAXBUTTON, HWND_TOP, SWP_ASYNCWINDOWPOS, SWP_HIDEWINDOW, SWP_NOACTIVATE,
        SWP_SHOWWINDOW, WINDOW_EX_STYLE, WM_DPICHANGED, WM_NCDESTROY, WM_NCHITTEST,
        WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE, WM_SIZE,
        WNDCLASSEXW, WS_CHILD, WS_CLIPSIBLINGS,
    };

    use super::{overlay_rect, SnapTarget};

    const CLASS: PCWSTR = w!("OxiTideSnapOverlay");
    const SUBCLASS_ID: usize = 0x6f78_736e_6170; // "oxsnap"
    const EVENT: &str = "titlebar://snap";

    struct Overlay {
        parent: isize,
        overlay: isize,
        target: Option<SnapTarget>,
        hovering: bool,
        pressing: bool,
    }

    static OVERLAY: Mutex<Option<Overlay>> = Mutex::new(None);
    static APP: OnceLock<AppHandle> = OnceLock::new();

    /// Drop the system frame and attach the snap overlay to `window`. Call
    /// before the window is first shown so the frame never flashes.
    pub fn install(window: &WebviewWindow) {
        if let Err(e) = window.set_decorations(false) {
            tracing::warn!("custom title bar unavailable: {e}");
            return;
        }
        let _ = APP.set(window.app_handle().clone());
        let parent = match window.hwnd() {
            Ok(hwnd) => hwnd.0 as isize,
            Err(e) => {
                tracing::warn!("snap layouts unavailable: {e}");
                return;
            }
        };
        let run = window.run_on_main_thread(move || unsafe { attach(parent) });
        if let Err(e) = run {
            tracing::warn!("snap layouts unavailable: {e}");
        }
    }

    /// Move (or with `None`, hide) the overlay over the UI's maximize button.
    pub fn set_target(target: Option<SnapTarget>) {
        let mut slot = OVERLAY.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(state) = slot.as_mut() {
            state.target = target;
            unsafe { position(state) };
        }
    }

    unsafe fn attach(parent: isize) {
        let instance: HINSTANCE = match GetModuleHandleW(None) {
            Ok(module) => module.into(),
            Err(e) => {
                tracing::warn!("snap layouts unavailable: {e}");
                return;
            }
        };
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(overlay_proc),
            hInstance: instance,
            hbrBackground: HBRUSH(GetStockObject(NULL_BRUSH).0),
            lpszClassName: CLASS,
            ..Default::default()
        };
        // A second registration (e.g. a re-attach) fails harmlessly.
        RegisterClassExW(&class);
        // Created hidden; the first `set_target` shows it. No WS_EX_LAYERED or
        // WS_EX_TRANSPARENT: either would stop it receiving hit tests.
        let overlay = match CreateWindowExW(
            WINDOW_EX_STYLE(0),
            CLASS,
            CLASS,
            WS_CHILD | WS_CLIPSIBLINGS,
            0,
            0,
            0,
            0,
            Some(HWND(parent as _)),
            None,
            Some(instance),
            None,
        ) {
            Ok(hwnd) => hwnd,
            Err(e) => {
                tracing::warn!("snap layouts unavailable: {e}");
                return;
            }
        };
        let mut slot = OVERLAY.lock().unwrap_or_else(|e| e.into_inner());
        *slot = Some(Overlay {
            parent,
            overlay: overlay.0 as isize,
            target: None,
            hovering: false,
            pressing: false,
        });
        drop(slot);
        let _ = SetWindowSubclass(HWND(parent as _), Some(parent_proc), SUBCLASS_ID, 0);
    }

    /// Place the overlay from the stored target and the parent's client size.
    unsafe fn position(state: &Overlay) {
        let overlay = HWND(state.overlay as _);
        let flags = SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE;
        let Some(target) = state.target else {
            let _ = SetWindowPos(overlay, Some(HWND_TOP), 0, 0, 0, 0, flags | SWP_HIDEWINDOW);
            return;
        };
        let parent = HWND(state.parent as _);
        let mut client = RECT::default();
        if GetClientRect(parent, &mut client).is_err() {
            return;
        }
        let (x, y, width, height) = overlay_rect(target, client.right, GetDpiForWindow(parent));
        let _ = SetWindowPos(overlay, Some(HWND_TOP), x, y, width, height, flags | SWP_SHOWWINDOW);
    }

    fn emit(kind: &'static str) {
        if let Some(app) = APP.get() {
            let _ = app.emit(EVENT, kind);
        }
    }

    unsafe extern "system" fn parent_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        match msg {
            WM_SIZE | WM_DPICHANGED => {
                let slot = OVERLAY.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(state) = slot.as_ref() {
                    position(state);
                }
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(parent_proc), SUBCLASS_ID);
                *OVERLAY.lock().unwrap_or_else(|e| e.into_inner()) = None;
            }
            _ => {}
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }

    /// Update the hover/press flags and return the events to send, so no event
    /// is emitted while the state lock is held.
    fn transition(update: impl FnOnce(&mut Overlay) -> &'static [&'static str]) {
        let events = {
            let mut slot = OVERLAY.lock().unwrap_or_else(|e| e.into_inner());
            match slot.as_mut() {
                Some(state) => update(state),
                None => &[],
            }
        };
        for kind in events {
            emit(kind);
        }
    }

    unsafe extern "system" fn overlay_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        match msg {
            WM_NCHITTEST => return LRESULT(HTMAXBUTTON as isize),
            WM_NCMOUSEMOVE => {
                transition(|s| {
                    if s.hovering {
                        &[]
                    } else {
                        s.hovering = true;
                        &["enter"]
                    }
                });
                let mut track = TRACKMOUSEEVENT {
                    cbSize: std::mem::size_of::<TRACKMOUSEEVENT>() as u32,
                    dwFlags: TME_LEAVE | TME_NONCLIENT,
                    hwndTrack: hwnd,
                    dwHoverTime: 0,
                };
                let _ = TrackMouseEvent(&mut track);
                return LRESULT(0);
            }
            WM_NCMOUSELEAVE => {
                transition(|s| {
                    s.hovering = false;
                    s.pressing = false;
                    &["leave"]
                });
                return LRESULT(0);
            }
            // Swallow the press so Windows' own caption-button tracking never
            // starts; the UI performs the maximize on `click`.
            WM_NCLBUTTONDOWN => {
                transition(|s| {
                    s.pressing = true;
                    &["down"]
                });
                return LRESULT(0);
            }
            WM_NCLBUTTONUP => {
                transition(|s| {
                    let click = s.pressing;
                    s.pressing = false;
                    if click {
                        &["click"]
                    } else {
                        &[]
                    }
                });
                return LRESULT(0);
            }
            _ => {}
        }
        DefWindowProcW(hwnd, msg, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TARGET: SnapTarget = SnapTarget { right: 92.0, top: 0.0, width: 46.0, height: 32.0 };

    #[test]
    fn overlay_is_anchored_to_the_right_edge() {
        assert_eq!(overlay_rect(TARGET, 1200, 96), (1108, 0, 46, 32));
        // A resize moves the overlay with the edge; the target is unchanged.
        assert_eq!(overlay_rect(TARGET, 800, 96), (708, 0, 46, 32));
    }

    #[test]
    fn overlay_scales_with_dpi() {
        // 150%: 92 -> 138, 46 -> 69, 32 -> 48.
        assert_eq!(overlay_rect(TARGET, 1800, 144), (1662, 0, 69, 48));
    }

    #[test]
    fn overlay_never_collapses_to_zero() {
        let tiny = SnapTarget { right: 0.0, top: 0.0, width: 0.1, height: 0.1 };
        assert_eq!(overlay_rect(tiny, 100, 96), (100, 0, 1, 1));
    }
}
