# Track 9.5 — Hi-Fi Rack smoke checklist

Human run with a real account: `npm --prefix ui run app:dev`, then Settings →
Appearance → Mode → **Hi-Fi Rack**. Spec:
`docs/superpowers/specs/2026-10-08-oxitide-track9-5-hifi-rack-design.md`.

Automated gates at hand-off (2026-10-08, branch `track9-5-hifi-rack`): `cargo test
--workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`,
`npm --prefix ui test` — all green.

## Shell and navigation
- [ ] Switching to Hi-Fi Rack swaps the sidebar + player bar for the rack, live; switching back restores the default shell with no leftovers (lamp colour, dark scheme).
- [ ] Source keys match the sidebar: Home, Explore, Feed, History, Library (playlists, collection, album/artist/mix pages), Setup. Ctrl+F still focuses Explore's search.
- [ ] Sleep, Mini, Setup and Power line up with the source keys (lamp row + 34px key row); the Sleep lamp lights while a timer runs.
- [ ] Library shows the sub-source key tray (Albums … Feed) on collection pages only.
- [ ] Back/forward keys, Alt+←/→ and mouse side buttons navigate.
- [ ] The Playlists unit under the EQ shows the playlist tree, scrolls on its own, and fills the column to the display's bottom edge; picking a playlist navigates. "All" opens the playlists page.
- [ ] Queue key (deck) opens the queue drawer; its close key and Esc close it.
- [ ] No unit stretches: the EQ keeps its natural height, keys keep their size, the deck's key row fits without clipping.
- [ ] Sleep timer and Mini keys behave as in the player bar.
- [ ] Skip link, Tab order and focus rings work across selector, deck, amp, EQ and the display.

## Deck
- [ ] Deck face setting: Transport (cover + up next), CD player (calendar, remaining, eject opens queue), Display (format line, bar meter, next).
- [ ] VFD: title, artist · album, 7-segment elapsed, annunciators (PLAY, BUF, RPT/RPT 1, SHUF, XFADE, TRK nn/nn); errors and the bit-perfect notice show on the sub line.
- [ ] VFD seek: click/drag seeks; arrows ±5 s, PageUp/PageDown ±10 %, Home/End.
- [ ] Transport keys, Shuffle, Repeat (off → all → one), ♥ favourite, Queue.
- [ ] Now Playing (cover click, expand key, existing shortcuts) expands the deck: the platter fills the deck, spins only while playing, tonearm follows progress; amp (meters not clipped), a compact spectrum and the five-line lyrics unit on the right; the VFD and keys form a centred strip; Esc / the collapse key return.
- [ ] Short window (< 680px tall): face windows hide; VFD and keys stay.

## Amp and EQ
- [ ] Meters setting: Needles, Bars (PROGRAM HOLD readout), LED (hold LED stays), Spectrum — all move with the music and fall to rest on pause.
- [ ] Now Playing extra unit: with amp meters = Spectrum, Auto shows the Level recorder; the head keys switch Spectrum / Recorder / Tape / Tuner / EQ / Off and the choice survives a restart; Off leaves Now playing in place with the keys in its head.
- [ ] Tape: packs wind left → right with the track, hubs turn only while playing (emptier reel faster), counter runs. Tuner: pointer glides along the minute scale, station readout shows time. EQ: faders and presets work and match the column EQ/Settings.
- [ ] Owner decision recorded: which extra-unit options to keep.
- [ ] Level recorder: L/R traces drift slowly left and show the song's dynamics (verse vs chorus); it stands still on pause and resumes on play; it follows lamp colour changes; nothing about it is distracting.
- [ ] Meter feel: needles swing smoothly (no jitter), sit mid-scale on a typical track and reach the red only on loud passages; bars/LEDs rise fast and fall slowly; nothing hugs the top.
- [ ] Now Playing: amp front is symmetric (DSP keys left, meters centred, larger knob right); Now playing unit shows lyrics left and track info + signal path right; Up next reads like a track list.
- [ ] Meters stop (no CPU/IPC) when the window is minimised or hidden to tray, and resume on restore.
- [ ] Volume knob: drag, wheel, arrows, Page keys, Home/End, double-click mute; LED ring follows; volume persists.
- [ ] Status LEDs: quality, Bit-perfect (green) or DSP, format (24/96).
- [ ] DSP keys toggle EQ / ReplayGain / Crossfade and Settings → Playback shows the same values (and vice versa).
- [ ] EQ unit: presets select; faders drag/wheel/keys; moving a fader with EQ off turns it on; "Custom" shows when no preset matches. Hidden below 1000px wide and when the setting is off.
- [ ] Bit-perfect still reports true with EQ/ReplayGain/crossfade off while the meters run (the tap only reads).

## Pages and Setup
- [ ] Album, playlist, artist, mix headers are glass panes with light-bar Play/Shuffle keys; track tables are panes with rows edge to edge.
- [ ] Home, Explore, Library grids and carousels sit in panes under printed labels with an engraved rule.
- [ ] Settings: tabs and segmented choices are key trays, sliders are faders, inputs/selects read as VFDs, checkboxes are lamps; every control still works.
- [ ] Rack group: Deck face, Amp meters, Lamp colour, EQ unit, Lamp brightness all apply live and persist across restart.

## Lamp colour
- [ ] Amber, Blue, Green, Red (over-zone turns hot white), White all read as lit screens; text stays legible at 40% brightness.
- [ ] Cover: lamp follows the album art's hue and glides on track change; a grey cover keeps amber.
- [ ] Atmosphere's dynamic colour is unaffected after switching back.

## Regression (other modes)
- [ ] Atmosphere and Instrument look identical to `main` (A/B a few pages: album, home, settings).
- [ ] Mini player from Rack wears the Rack skin (faceplate, VFD title, light-bar keys, queue sheet as a unit), works, and returns to Rack.
- [ ] Now Playing's Up next changer lists the next four tracks; clicking one plays from there.

## Perf (PM-D1)
- [ ] Default-mode S1 and S5 re-run under unchanged budgets.
- [ ] Rack: record S1 and S5 once; note meter CPU while playing.
