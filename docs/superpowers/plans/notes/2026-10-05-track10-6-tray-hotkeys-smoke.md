# Track 10.6 tray and global hotkeys — GUI smoke checklist

Human, real account, Windows. Record pass/fail per step; steps 3–5 and 7–11 need
a track loaded unless stated. Linux caveats (tray needs libappindicator; Wayland
global shortcuts need a portal; left-click menu behavior) follow the Track 8
exemption pattern and are checked in the track completion note.

1. The tray icon appears when the app starts; hovering it shows "OxiTide".
2. Play a track: within a second the tooltip reads "Title - Artist". Tray
   Play/Pause, Next, and Previous control playback. End the queue: the tooltip
   returns to "OxiTide".
3. Press the window X with "Close to tray" on (default): the window hides, the
   app keeps playing, and the tray icon stays. Left-clicking the tray icon
   re-shows the window; the tray menu's Show / Hide also toggles it.
4. Tray menu → Quit exits fully. Relaunching restores the paused session and
   the window geometry from step 3.
5. Launch a second instance while the window is hidden: the window shows,
   unminimizes, and focuses; no second tray icon appears.
6. Settings → System: "Close to tray" is checked. Turn it off, press X: the app
   exits. Turn it on again: X hides.
7. Record `Space` for Play / pause; the row shows "Space" and a single press
   toggles playback exactly once while the OxiTide window is focused and while
   another app is focused (once a chord is registered the global handler is the
   only delivery path). Global hotkeys are temporarily unregistered while the
   recorder is active.
8. Try to record `Space` for Next: a modal appears reading `"Space" is already
   used by "Play / pause". Reassign it to "Next track"?`. Cancel keeps both
   bindings; repeating and choosing Reassign moves `Space` to Next and leaves
   Play / pause "Not set". Both outcomes survive a restart.
9. Record a chord another app already owns (e.g. an OS-reserved one): a toast
   appears ("Couldn't register … another app may be using it.") and the
   previous binding stays.
10. Clear a hotkey (Backspace/Delete while recording, or the Clear button):
    the row shows "Not set" and the accelerator no longer fires after restart.
11. Volume up/down hotkeys move the volume in 5% steps and the level survives a
    restart (`config.audio.volume`).
12. `%APPDATA%\OxiTide\config\config.toml` contains `close_to_tray = true|false`
    under `[ui]` and a `[hotkeys]` section with the recorded accelerators.
13. Known edges: (a) hand-edit `next = "Bogus+Key"` into the `[hotkeys]` section
    of `%APPDATA%\OxiTide\config\config.toml`, relaunch: the app starts with a
    warning logged and the remaining valid bindings still register; remove the
    bad value to recover. (b) Start recording a hotkey, then switch to another
    Settings tab: recording is cancelled and the global hotkeys are restored.
