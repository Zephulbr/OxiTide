# ASIO output — owner smoke checklist (Plan 7.3)

- **Branch:** `track7-asio`
- **Spec:** `docs/superpowers/specs/2026-10-11-oxitide-track7-asio-output-design.md`
- **Automated (2026-10-11):** `cargo test -p oxitide-audio --lib -- --ignored asio_output`
  opened **Topping USB Audio Device (ASIO)** as Int32, `Exclusive`, bit-perfect, and completed a
  flush (so the driver's callbacks were running). **Realtek ASIO** refused `init` (expected
  when its hardware path is inactive; covered by the fallback item below).

Run with `npm --prefix ui run app:dev`, a real account, and no DAW holding the driver.

- [x] Settings → Playback lists both drivers as "… (ASIO)" after the WASAPI endpoints.
- [x] Select Topping (ASIO): the hint under Bit-perfect appears; playback restarts at the same
      position and plays; status shows Exclusive and bit-perfect (Rack shows EXCL).
- [x] 16/44.1 → 24/96 track change (Max quality): the driver reopens at the new rate and plays
      at the correct pitch.
- [x] Seek and the sleep-timer fade-pause: no stale audio after the seek; no tail after pause.
- [x] Volume below 100%: audible attenuation; the notice "volume applied in software;
      bit-perfect compromised" appears; back to 100% clears it.
- [x] EQ/ReplayGain on: bit-perfect off with the stages listed; audio still correct.
- [x] Select Realtek ASIO with fallback = "Fall back to shared output": playback continues on
      the system default with "ASIO unavailable (…); using shared output".
- [x] Same with fallback = "Stop with a message": playback stops with the reason.
- [x] Open the Topping control panel and change the buffer size while playing: playback pauses
      with "ASIO driver was reset or disconnected"; Play resumes at the position.
- [x] Unplug the Topping while playing: the app pauses with a notice (no hang/crash);
      replug + Play resumes.
- [x] Switch back to a WASAPI device: plays, and the ASIO driver is released (another ASIO host
      can open it).
- [x] Quit while playing on ASIO: the app exits cleanly.

**Closed 2026-10-26 (owner):** the remaining rows were marked done by the owner, not ticked row by row.
