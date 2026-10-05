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
7. Record `Ctrl+Alt+Space` for Play / pause; the row shows "Ctrl + Alt + Space"
   and playback toggles from another focused app. With the OxiTide window
   focused, pressing Space still toggles exactly once (no double-fire).
8. Try to record `Ctrl+Alt+Space` for Next: a toast appears and Play / pause
   keeps its binding; restarting keeps it too.
9. Record a chord another app already owns (e.g. an OS-reserved one): a toast
   appears and the previous binding stays.
10. Clear a hotkey (Backspace/Delete while recording, or the Clear button):
    the row shows "Not set" and the accelerator no longer fires after restart.
11. Volume up/down hotkeys move the volume in 5% steps and the level survives a
    restart (`config.audio.volume`).
12. `%APPDATA%\OxiTide\config.toml` contains `close_to_tray = true|false` under
    `[ui]` and a `[hotkeys]` section with the recorded accelerators.
13. Known edges: (a) hand-edit `next = "Bogus+Key"` into `[hotkeys]`, relaunch:
    the app starts (warning logged), and any valid bindings listed after the
    malformed one may not register — remove the bad value to recover. (b) Start
    recording a hotkey, switch to another Settings tab, then press a chord: it
    is saved silently with the row no longer visible; press Esc before leaving
    the tab to cancel instead.
