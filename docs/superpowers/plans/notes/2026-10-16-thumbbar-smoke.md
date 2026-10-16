# Taskbar thumbnail toolbar — owner smoke (Windows)

Spec: `docs/superpowers/specs/2026-10-16-oxitide-taskbar-thumbbar-design.md`.

- [ ] Fresh start, before anything plays: hovering the taskbar button shows Previous / Play /
      Next, all disabled (greyed).
- [ ] Start a track: buttons enable; the middle one shows Pause with tooltip "Pause".
- [ ] Click Pause → playback pauses and the button turns into Play; click Play → resumes.
- [ ] Next and Previous change the track (Previous may restart the current one first).
- [ ] State changed elsewhere (in-app button, media key, tray, phone remote) is reflected in the
      preview's play/pause icon.
- [ ] Mini player: the toolbar still works from the taskbar preview.
- [ ] Glyphs are crisp at 100 % and at 150 %/200 % display scale (move the window to a monitor
      with a different scale and re-check).
- [ ] Restart Explorer (Task Manager → Windows Explorer → Restart): the buttons come back.
- [ ] Light and dark taskbar themes: glyphs stay visible (they are white, matching other apps).
