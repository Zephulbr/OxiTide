# Taskbar thumbnail toolbar — owner smoke (Windows)

Spec: `docs/superpowers/specs/2026-10-16-oxitide-taskbar-thumbbar-design.md`.

**Result: PASS, 2026-10-24** (owner, Windows, real account). All nine rows confirmed.

- [x] Fresh start, before anything plays: hovering the taskbar button shows Previous / Play /
      Next, all disabled (greyed).
- [x] Start a track: buttons enable; the middle one shows Pause with tooltip "Pause".
- [x] Click Pause → playback pauses and the button turns into Play; click Play → resumes.
- [x] Next and Previous change the track (Previous may restart the current one first).
- [x] State changed elsewhere (in-app button, media key, tray, phone remote) is reflected in the
      preview's play/pause icon.
- [x] Mini player: the toolbar still works from the taskbar preview.
- [x] Glyphs are crisp at 100 % and at 150 %/200 % display scale (move the window to a monitor
      with a different scale and re-check).
- [x] Restart Explorer (Task Manager → Windows Explorer → Restart): the buttons come back.
- [x] Light and dark taskbar themes: glyphs stay visible (they are white, matching other apps).
