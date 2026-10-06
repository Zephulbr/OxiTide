# Windows SMTC + media keys — manual smoke checklist

Requires a real account and a real Windows session; record date and result per item.

- [ ] Play a track with the window focused: the media flyout shows title / artist / album and
      the album art; the play/pause icon matches the state.
- [ ] Hardware media keys (focus another app first): play/pause, next, and previous all drive
      OxiTide while its window is unfocused.
- [ ] The flyout scrubber advances during playback and never overshoots the track end.
- [ ] Dragging the flyout scrubber seeks OxiTide to that position.
- [ ] Pause: the flyout shows paused; media-key play resumes.
- [ ] A track with no album art still shows title / artist / album (flyout not blank).
- [ ] A track with unusual metadata does not break the flyout.
- [ ] Mini player mode: metadata and media keys still work.
- [ ] Queue clears / playback stops: the flyout clears instead of showing stale metadata.
- [ ] Settings → Connections: unchecking "Windows media controls" removes it from the flyout
      and media keys stop controlling OxiTide; playback is unaffected. Re-checking restores it
      without a restart.
- [ ] Restart the app: the flag is honored; a disabled setting stays disabled.
- [ ] Log out: the flyout entry is cleared and the app does not hang.
- [ ] Quit while playing: no hang, no orphaned media session.
- [ ] WebView interaction (typing, clicking, scrolling) is unaffected throughout.
