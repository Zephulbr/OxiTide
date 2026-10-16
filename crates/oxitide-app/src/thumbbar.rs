//! Taskbar thumbnail toolbar (spec `2026-10-16-oxitide-taskbar-thumbbar-design.md`):
//! Previous / Play-Pause / Next under the window's taskbar preview on Windows.
//!
//! The buttons go through `ITaskbarList3`, which only accepts them once the
//! shell has created the window's taskbar button (`TaskbarButtonCreated`,
//! re-sent after an Explorer restart), and only once per button; later
//! changes are `ThumbBarUpdateButtons`. Clicks arrive at the main window as
//! `WM_COMMAND` / `THBN_CLICKED` and run through `tray::run`, the same path
//! as the tray menu. All COM calls stay on the main (STA) thread; the engine
//! subscriber hands state over with `run_on_main_thread`.
//!
//! The glyphs are rasterized here at the DPI's small-icon size instead of
//! shipping icon files, so they stay crisp at any scale.

use crate::tray::TrayAction;

/// What the buttons show: the player's state as far as the taskbar cares.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct View {
    pub playing: bool,
    pub has_track: bool,
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Glyph {
    Prev,
    Play,
    Pause,
    Next,
}

#[cfg_attr(not(windows), allow(dead_code))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Button {
    id: u32,
    glyph: Glyph,
    tip: &'static str,
    enabled: bool,
}

#[cfg_attr(not(windows), allow(dead_code))]
const ID_PREV: u32 = 1;
#[cfg_attr(not(windows), allow(dead_code))]
const ID_PLAY_PAUSE: u32 = 2;
#[cfg_attr(not(windows), allow(dead_code))]
const ID_NEXT: u32 = 3;

/// The three buttons, left to right, for a view. Without a track there is
/// nothing to drive, so all three are disabled.
#[cfg_attr(not(windows), allow(dead_code))]
fn buttons(view: View) -> [Button; 3] {
    let (glyph, tip) = if view.playing {
        (Glyph::Pause, "Pause")
    } else {
        (Glyph::Play, "Play")
    };
    let enabled = view.has_track;
    [
        Button {
            id: ID_PREV,
            glyph: Glyph::Prev,
            tip: "Previous",
            enabled,
        },
        Button {
            id: ID_PLAY_PAUSE,
            glyph,
            tip,
            enabled,
        },
        Button {
            id: ID_NEXT,
            glyph: Glyph::Next,
            tip: "Next",
            enabled,
        },
    ]
}

/// Map a clicked button id to its transport action.
#[cfg_attr(not(windows), allow(dead_code))]
fn action(id: u32) -> Option<TrayAction> {
    match id {
        ID_PREV => Some(TrayAction::Prev),
        ID_PLAY_PAUSE => Some(TrayAction::PlayPause),
        ID_NEXT => Some(TrayAction::Next),
        _ => None,
    }
}

type Point = (f64, f64);

/// The glyph as convex polygons in a 24-unit box (Lucide's grid), filled.
#[cfg_attr(not(windows), allow(dead_code))]
fn shapes(glyph: Glyph) -> Vec<Vec<Point>> {
    let rect = |x0: f64, x1: f64| vec![(x0, 5.0), (x1, 5.0), (x1, 19.0), (x0, 19.0)];
    let mirror = |shape: Vec<Point>| {
        shape
            .into_iter()
            .rev()
            .map(|(x, y)| (24.0 - x, y))
            .collect()
    };
    let next = vec![
        vec![(5.0, 5.0), (15.5, 12.0), (5.0, 19.0)],
        rect(16.5, 19.0),
    ];
    match glyph {
        Glyph::Play => vec![vec![(7.0, 4.5), (19.5, 12.0), (7.0, 19.5)]],
        Glyph::Pause => vec![rect(6.0, 10.0), rect(14.0, 18.0)],
        Glyph::Next => next,
        Glyph::Prev => next.into_iter().map(mirror).collect(),
    }
}

/// Whether `p` lies inside a convex polygon given in clockwise order (y down).
fn inside(shape: &[Point], p: Point) -> bool {
    shape
        .iter()
        .zip(shape.iter().cycle().skip(1))
        .all(|(a, b)| (b.0 - a.0) * (p.1 - a.1) - (b.1 - a.1) * (p.0 - a.0) >= 0.0)
}

/// Render a glyph as `size`×`size` white RGBA (straight alpha), 4×4
/// supersampled so the edges stay smooth at 16 px.
#[cfg_attr(not(windows), allow(dead_code))]
fn rasterize(glyph: Glyph, size: u32) -> Vec<u8> {
    const SS: u32 = 4;
    let shapes = shapes(glyph);
    let scale = 24.0 / f64::from(size);
    let mut out = Vec::with_capacity((size * size * 4) as usize);
    for y in 0..size {
        for x in 0..size {
            let mut hits = 0u32;
            for sy in 0..SS {
                for sx in 0..SS {
                    let p = (
                        (f64::from(x) + (f64::from(sx) + 0.5) / f64::from(SS)) * scale,
                        (f64::from(y) + (f64::from(sy) + 0.5) / f64::from(SS)) * scale,
                    );
                    if shapes.iter().any(|s| inside(s, p)) {
                        hits += 1;
                    }
                }
            }
            let alpha = (hits * 255 / (SS * SS)) as u8;
            out.extend_from_slice(&[255, 255, 255, alpha]);
        }
    }
    out
}

#[cfg(windows)]
pub use imp::{install, set_view};

#[cfg(not(windows))]
pub fn install(_window: &tauri::WebviewWindow) {}

#[cfg(not(windows))]
pub fn set_view(_app: &tauri::AppHandle, _view: View) {}

#[cfg(windows)]
mod imp {
    use std::cell::RefCell;
    use std::sync::{Mutex, OnceLock};

    use tauri::{AppHandle, Manager, WebviewWindow};
    use windows::core::w;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::Com::{
        CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
    };
    use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
    use windows::Win32::UI::Shell::{
        DefSubclassProc, ITaskbarList3, RemoveWindowSubclass, SetWindowSubclass, TaskbarList,
        THBF_DISABLED, THBF_ENABLED, THBN_CLICKED, THB_FLAGS, THB_ICON, THB_TOOLTIP, THUMBBUTTON,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        ChangeWindowMessageFilterEx, CreateIconFromResourceEx, DestroyIcon, RegisterWindowMessageW,
        HICON, LR_DEFAULTCOLOR, MSGFLT_ALLOW, SM_CXSMICON, WM_COMMAND, WM_DPICHANGED, WM_NCDESTROY,
    };

    use super::{action, buttons, rasterize, Glyph, View};

    const SUBCLASS_ID: usize = 0x6f78_7468_756d; // "oxthum"

    /// Per-window state. COM interfaces are not `Send`, so it lives on the
    /// main thread only.
    struct Bar {
        hwnd: HWND,
        taskbar: Option<ITaskbarList3>,
        added: bool,
        icons: Vec<(Glyph, HICON)>,
    }

    thread_local! {
        static BAR: RefCell<Option<Bar>> = const { RefCell::new(None) };
    }

    static APP: OnceLock<AppHandle> = OnceLock::new();
    static CREATED_MSG: OnceLock<u32> = OnceLock::new();
    /// The latest view, written from any thread and applied on the main one.
    static VIEW: Mutex<View> = Mutex::new(View {
        playing: false,
        has_track: false,
    });

    /// Hook the main window so the buttons appear once its taskbar button
    /// exists. Safe to call before the window is first shown.
    pub fn install(window: &WebviewWindow) {
        let _ = APP.set(window.app_handle().clone());
        let hwnd = match window.hwnd() {
            Ok(hwnd) => hwnd.0 as isize,
            Err(e) => {
                tracing::warn!("taskbar buttons unavailable: {e}");
                return;
            }
        };
        let run = window.run_on_main_thread(move || unsafe { attach(HWND(hwnd as _)) });
        if let Err(e) = run {
            tracing::warn!("taskbar buttons unavailable: {e}");
        }
    }

    /// Record the player's state and refresh the buttons on the main thread.
    pub fn set_view(app: &AppHandle, view: View) {
        {
            let mut current = VIEW.lock().unwrap_or_else(|e| e.into_inner());
            if *current == view {
                return;
            }
            *current = view;
        }
        let _ = app.run_on_main_thread(|| unsafe { sync() });
    }

    unsafe fn attach(hwnd: HWND) {
        // tao already initialized COM on this thread; this is a no-op then.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        let created =
            *CREATED_MSG.get_or_init(|| RegisterWindowMessageW(w!("TaskbarButtonCreated")));
        // An elevated process would otherwise never see Explorer's messages.
        let _ = ChangeWindowMessageFilterEx(hwnd, created, MSGFLT_ALLOW, None);
        let _ = ChangeWindowMessageFilterEx(hwnd, WM_COMMAND, MSGFLT_ALLOW, None);
        BAR.set(Some(Bar {
            hwnd,
            taskbar: None,
            added: false,
            icons: Vec::new(),
        }));
        let _ = SetWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID, 0);
        // The taskbar button may already exist (the window was shown before
        // this ran); otherwise this fails quietly and the message retries.
        add();
    }

    /// (Re)create the taskbar interface and add the buttons. Runs on every
    /// `TaskbarButtonCreated`, since an Explorer restart drops them.
    unsafe fn add() {
        with_bar(|bar| {
            let taskbar = match CoCreateInstance::<_, ITaskbarList3>(
                &TaskbarList,
                None,
                CLSCTX_INPROC_SERVER,
            ) {
                Ok(taskbar) => taskbar,
                Err(e) => {
                    tracing::warn!("taskbar buttons unavailable: {e}");
                    return;
                }
            };
            if taskbar.HrInit().is_err() {
                return;
            }
            rebuild_icons(bar);
            let list = thumb_buttons(bar);
            match taskbar.ThumbBarAddButtons(bar.hwnd, &list) {
                Ok(()) => bar.added = true,
                // Already added for this taskbar button: update instead.
                Err(_) if bar.added => {
                    let _ = taskbar.ThumbBarUpdateButtons(bar.hwnd, &list);
                }
                Err(e) => tracing::debug!("taskbar buttons not added yet: {e}"),
            }
            bar.taskbar = Some(taskbar);
        });
    }

    /// Push the current view onto the buttons.
    unsafe fn sync() {
        with_bar(|bar| {
            let (Some(taskbar), true) = (&bar.taskbar, bar.added) else {
                return;
            };
            let _ = taskbar.ThumbBarUpdateButtons(bar.hwnd, &thumb_buttons(bar));
        });
    }

    /// Run `f` on the bar with it moved out of the slot, so a message that
    /// re-enters the window proc during a COM call finds no bar (and skips)
    /// instead of hitting a held `RefCell` borrow.
    fn with_bar(f: impl FnOnce(&mut Bar)) {
        let Some(mut bar) = BAR.take() else { return };
        f(&mut bar);
        BAR.set(Some(bar));
    }

    fn thumb_buttons(bar: &Bar) -> Vec<THUMBBUTTON> {
        let view = *VIEW.lock().unwrap_or_else(|e| e.into_inner());
        buttons(view)
            .iter()
            .map(|b| {
                let mut tip = [0u16; 260];
                for (slot, unit) in tip.iter_mut().zip(b.tip.encode_utf16()) {
                    *slot = unit;
                }
                let icon = bar
                    .icons
                    .iter()
                    .find(|(g, _)| *g == b.glyph)
                    .map(|(_, i)| *i);
                THUMBBUTTON {
                    dwMask: THB_ICON | THB_TOOLTIP | THB_FLAGS,
                    iId: b.id,
                    hIcon: icon.unwrap_or_default(),
                    szTip: tip,
                    dwFlags: if b.enabled {
                        THBF_ENABLED
                    } else {
                        THBF_DISABLED
                    },
                    ..Default::default()
                }
            })
            .collect()
    }

    /// Render every glyph at the window's current small-icon size.
    unsafe fn rebuild_icons(bar: &mut Bar) {
        for (_, icon) in bar.icons.drain(..) {
            let _ = DestroyIcon(icon);
        }
        let size = GetSystemMetricsForDpi(SM_CXSMICON, GetDpiForWindow(bar.hwnd)).max(16) as u32;
        for glyph in [Glyph::Prev, Glyph::Play, Glyph::Pause, Glyph::Next] {
            match icon(glyph, size) {
                Some(icon) => bar.icons.push((glyph, icon)),
                None => tracing::warn!("taskbar icon {glyph:?} failed to render"),
            }
        }
    }

    /// A glyph as an `HICON`, through a PNG `CreateIconFromResourceEx` reads.
    unsafe fn icon(glyph: Glyph, size: u32) -> Option<HICON> {
        use image::ImageEncoder;
        let pixels = rasterize(glyph, size);
        let mut png = Vec::new();
        image::codecs::png::PngEncoder::new(&mut png)
            .write_image(&pixels, size, size, image::ExtendedColorType::Rgba8)
            .ok()?;
        let size = size as i32;
        CreateIconFromResourceEx(&png, true, 0x0003_0000, size, size, LR_DEFAULTCOLOR).ok()
    }

    unsafe extern "system" fn window_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _id: usize,
        _data: usize,
    ) -> LRESULT {
        if Some(&msg) == CREATED_MSG.get() {
            add();
        } else if msg == WM_COMMAND && (wparam.0 >> 16) as u32 & 0xffff == THBN_CLICKED {
            let id = (wparam.0 & 0xffff) as u32;
            if let (Some(app), Some(action)) = (APP.get(), action(id)) {
                crate::tray::run(app, action);
                return LRESULT(0);
            }
        } else if msg == WM_DPICHANGED {
            with_bar(|bar| unsafe { rebuild_icons(bar) });
            sync();
        } else if msg == WM_NCDESTROY {
            let _ = RemoveWindowSubclass(hwnd, Some(window_proc), SUBCLASS_ID);
            if let Some(mut bar) = BAR.take() {
                for (_, icon) in bar.icons.drain(..) {
                    let _ = DestroyIcon(icon);
                }
            }
        }
        DefSubclassProc(hwnd, msg, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn alpha(pixels: &[u8], size: u32, x: u32, y: u32) -> u8 {
        pixels[((y * size + x) * 4 + 3) as usize]
    }

    #[test]
    fn play_pause_follows_the_player() {
        let paused = buttons(View {
            playing: false,
            has_track: true,
        });
        assert_eq!((paused[1].glyph, paused[1].tip), (Glyph::Play, "Play"));
        let playing = buttons(View {
            playing: true,
            has_track: true,
        });
        assert_eq!((playing[1].glyph, playing[1].tip), (Glyph::Pause, "Pause"));
        assert_eq!(playing.map(|b| b.id), [ID_PREV, ID_PLAY_PAUSE, ID_NEXT]);
    }

    #[test]
    fn buttons_are_disabled_without_a_track() {
        assert!(buttons(View::default()).iter().all(|b| !b.enabled));
        assert!(buttons(View {
            playing: false,
            has_track: true
        })
        .iter()
        .all(|b| b.enabled));
    }

    #[test]
    fn button_ids_map_to_transport() {
        assert_eq!(action(ID_PREV), Some(TrayAction::Prev));
        assert_eq!(action(ID_PLAY_PAUSE), Some(TrayAction::PlayPause));
        assert_eq!(action(ID_NEXT), Some(TrayAction::Next));
        assert_eq!(action(0), None);
    }

    #[test]
    fn glyphs_are_filled_in_the_middle_and_clear_at_the_corners() {
        for glyph in [Glyph::Prev, Glyph::Play, Glyph::Next] {
            let px = rasterize(glyph, 16);
            assert_eq!(px.len(), 16 * 16 * 4);
            assert_eq!(alpha(&px, 16, 0, 0), 0, "{glyph:?}");
            assert_eq!(alpha(&px, 16, 15, 15), 0, "{glyph:?}");
            assert_eq!(alpha(&px, 16, 7, 8), 255, "{glyph:?}");
        }
        // Pause has a gap between its bars.
        let pause = rasterize(Glyph::Pause, 16);
        assert_eq!(alpha(&pause, 16, 5, 8), 255);
        assert_eq!(alpha(&pause, 16, 8, 8), 0);
    }

    #[test]
    fn previous_mirrors_next() {
        let size = 20;
        let prev = rasterize(Glyph::Prev, size);
        let next = rasterize(Glyph::Next, size);
        for y in 0..size {
            for x in 0..size {
                assert_eq!(
                    alpha(&prev, size, x, y),
                    alpha(&next, size, size - 1 - x, y),
                    "({x},{y})"
                );
            }
        }
    }

    #[test]
    fn edges_are_antialiased() {
        let px = rasterize(Glyph::Play, 16);
        let partial = px.chunks(4).filter(|p| p[3] > 0 && p[3] < 255).count();
        assert!(partial > 0);
    }
}
