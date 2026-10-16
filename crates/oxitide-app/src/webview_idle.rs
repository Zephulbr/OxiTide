//! WebView2 idles while the main window cannot be seen (Track 13, F7).
//!
//! WebView2 is most of the app's memory, and it only knows the host window is
//! hidden when the host says so: wry sets the controller visible once and
//! never again, so minimized or hidden to the tray the page kept rendering,
//! running animation frames and holding its caches. A subclass on the main
//! window watches `WM_SIZE` (minimize / restore) and `WM_SHOWWINDOW` (tray
//! hide / show) and sets the controller's `IsVisible` together with the
//! memory target: `Low` while hidden lets WebView2 trim, `Normal` on return.
//! Audio and the engine live in Rust and are unaffected; the page still
//! receives events while hidden (it is not suspended).

/// What a window message means for the webview: `Some(true)` it is visible
/// again, `Some(false)` it went out of sight, `None` no change.
#[cfg_attr(not(windows), allow(dead_code))]
fn visibility_change(msg: u32, wparam: usize) -> Option<bool> {
    const WM_SIZE: u32 = 0x0005;
    const WM_SHOWWINDOW: u32 = 0x0018;
    const SIZE_RESTORED: usize = 0;
    const SIZE_MINIMIZED: usize = 1;
    const SIZE_MAXIMIZED: usize = 2;
    match (msg, wparam) {
        (WM_SIZE, SIZE_MINIMIZED) => Some(false),
        (WM_SIZE, SIZE_RESTORED | SIZE_MAXIMIZED) => Some(true),
        (WM_SHOWWINDOW, 0) => Some(false),
        (WM_SHOWWINDOW, _) => Some(true),
        _ => None,
    }
}

#[cfg(windows)]
pub use imp::install;

#[cfg(not(windows))]
pub fn install(_window: &tauri::WebviewWindow) {}

#[cfg(windows)]
mod imp {
    use std::cell::RefCell;

    use tauri::WebviewWindow;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2Controller, ICoreWebView2_19, COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW,
        COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL,
    };
    use windows::core::Interface;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::WM_NCDESTROY;

    const SUBCLASS_ID: usize = 0x6f78_6964_6c65; // "oxidle"

    thread_local! {
        /// The main webview's controller, and whether it was last told it is
        /// visible. Both live on the main thread, like the window proc and
        /// every WebView2 call.
        static STATE: RefCell<Option<(ICoreWebView2Controller, bool)>> = const { RefCell::new(None) };
    }

    pub fn install(window: &WebviewWindow) {
        let hwnd = match window.hwnd() {
            Ok(hwnd) => hwnd,
            Err(e) => {
                tracing::warn!("webview idle hook unavailable: {e}");
                return;
            }
        };
        let captured = window.with_webview(|webview| {
            let controller = webview.controller();
            STATE.with(|state| *state.borrow_mut() = Some((controller, true)));
        });
        if let Err(e) = captured {
            tracing::warn!("webview idle hook unavailable: {e}");
            return;
        }
        // SAFETY: `hwnd` is the live main window; the proc removes itself on
        // WM_NCDESTROY.
        unsafe {
            let _ = SetWindowSubclass(hwnd, Some(proc), SUBCLASS_ID, 0);
        }
    }

    fn set_visible(visible: bool) {
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            let Some((controller, shown)) = state.as_mut() else { return };
            if *shown == visible {
                return;
            }
            *shown = visible;
            let level = if visible {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_NORMAL
            } else {
                COREWEBVIEW2_MEMORY_USAGE_TARGET_LEVEL_LOW
            };
            // SAFETY: plain COM calls on the thread that owns the controller.
            unsafe {
                if let Err(e) = controller.SetIsVisible(visible) {
                    tracing::debug!("webview visibility: {e}");
                }
                let target = controller
                    .CoreWebView2()
                    .and_then(|core| core.cast::<ICoreWebView2_19>());
                match target {
                    Ok(core) => {
                        let _ = core.SetMemoryUsageTargetLevel(level);
                    }
                    Err(e) => tracing::debug!("webview memory target unavailable: {e}"),
                }
            }
            tracing::debug!("webview {}", if visible { "visible" } else { "hidden: idle" });
        });
    }

    unsafe extern "system" fn proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if msg == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(hwnd, Some(proc), SUBCLASS_ID);
            STATE.with(|state| *state.borrow_mut() = None);
        } else if let Some(visible) = super::visibility_change(msg, wparam.0) {
            set_visible(visible);
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::visibility_change;

    #[test]
    fn minimize_and_hide_idle_the_webview_restore_and_show_wake_it() {
        assert_eq!(visibility_change(0x0005, 1), Some(false), "minimized");
        assert_eq!(visibility_change(0x0005, 0), Some(true), "restored");
        assert_eq!(visibility_change(0x0005, 2), Some(true), "maximized");
        assert_eq!(visibility_change(0x0005, 3), None, "max-show (another window)");
        assert_eq!(visibility_change(0x0018, 0), Some(false), "hidden to the tray");
        assert_eq!(visibility_change(0x0018, 1), Some(true), "shown");
        assert_eq!(visibility_change(0x0010, 0), None, "unrelated message");
    }
}
