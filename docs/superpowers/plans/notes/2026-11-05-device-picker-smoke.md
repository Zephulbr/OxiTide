# Output device picker: owner smoke checklist

- **Spec:** `specs/2026-11-05-oxitide-output-device-picker-design.md`
- **Plan:** `plans/2026-11-05-output-device-picker.md` (Task 12)
- **Needs:** a real account, the owner's devices (TOPPING DX5, HyperX DuoCast, LG TV over HDMI,
  C27JG5x over DP, Realtek optical, the Topping and Realtek ASIO drivers), and for the Linux rows
  the CachyOS laptop with the MOONDROP Dawn Pro.

## Windows

- [x] **First start after the update.** The device you had selected is still selected. If the
      old "Bit-perfect output (exclusive mode)" switch was on, that device now shows
      **Exclusive** and every other device shows Shared. `config.toml` no longer contains
      `exclusive = true` after any settings change.
- [x] **Clean names.** Settings → Playback → Output lists "DX5", "HyperX DuoCast", "LG TV",
      "C27JG5x" and "Realtek Digital Output". The ASIO group shows "Topping USB Audio Device"
      and "Realtek ASIO" ("Wrapper over Windows audio"). Hovering a name shows the full system
      name.
- [x] **Capabilities.** No row has a BP tag. A second or two after the list opens, the
      selected DX5 row reads "Up to 384 kHz · 32-bit, hardware volume", with 384 and 32 in the
      accent.
- [x] **Layout.** The selected device sits first in its group with its detail, and the other
      rows scroll under it (about three and a half visible); the wheel passes on to the page at
      either end. Selecting another row moves it to the top, keeping keyboard focus. On the Playback tab, Crossfade sits above AutoMix on the right, and the Equalizer
      card's rows are no longer spread out.
- [x] **Per-device mode.** Set DX5 to Exclusive and the LG TV to Shared. Switching between them
      plays each in its own mode (player bar: BP on DX5, "Shared output" on the TV), with one
      restart at the position per switch and no second hiccup.
- [x] **Nickname.** Nickname DX5 "Desk DAC": the row, the quality menu and the System default
      line ("now Desk DAC" when it is the default) all use it. Nickname a device that isn't
      selected and press Enter: the name saves and the output does **not** switch. Clear the
      field and save: "DX5" comes back. Escape cancels the edit.
- [x] **Pin and quick switch.** Pin DX5 and the LG TV. The quality plate's menu shows an
      **Output** section with both, in pin order, with their modes. Picking one switches
      output. With nothing pinned the section is gone. **Output settings…** opens Playback and
      scrolls to and rings the device list, also when Settings is already open.
- [x] **Hide.** Hide the HyperX: it drops out, and "1 hidden · Show" brings it back. The
      selected device's Hide entry is disabled with "Switch to another device first".
- [x] **Icons.** Icons are bare (no plate). Give DX5 "DAC / Amp" and the LG TV "TV", set one
      to "No icon", and turn Device icons off (the row and quality-menu icons go) and back on.
- [x] **Pinned order.** Pinned devices sit right under the selected row, in pin order, with no
      extra styling. Pinning a row moves it up; unpinning moves it back.
- [x] **Unplug a pinned DAC while it plays.** The row stays, dimmed, as "Not connected" with
      its nickname (also in the quality menu, disabled). Playback falls back to the system
      default with the usual notice. Plug it back in: the list updates by itself, and playback
      returns to it.
- [x] **New USB port.** Move the DAC to another USB port. If Windows gives it a new id, its
      nickname, pin and mode follow it.
- [x] **ASIO.** Select the Topping ASIO driver: no mode switch ("Always exclusive"), and the
      ASIO note and "If exclusive fails" show. Play, then go back to the list while it plays:
      nothing breaks, and the list doesn't freeze.
- [x] **Keyboard and reader.** Tab reaches the list once; the arrows move between rows without
      switching devices (scrolling the group as needed), and Enter selects. A screen reader
      announces each row's name and connection.
- [x] **Looks.** Atmosphere dark and light, Studio, and Rack (the card inside the Rack display):
      readable, with the accent only on the selection.
- [x] **Diagnostics.** Copy diagnostics still reports "a selected device" and never a name or
      nickname.

## Linux (batched run)

- [x] The gates pass in WSL or on the laptop: `cargo test --workspace` and
      `cargo clippy --workspace -- -D warnings`. The build machine had no Linux toolchain;
      only the `alsa` calls were type-checked.
- [x] One "MOONDROP Dawn Pro" row (USB), not three. A saved `sysdefault:` / `plughw:` /
      `hw:` id becomes that row on first start.
- [x] Exclusive on the Dawn Pro gives ✓ BP with no hand-edited config. Shared plays through
      `plughw:`, or falls back with the notice while PipeWire holds the card.
- [x] The Dawn Pro's "Up to …" line shows even while PipeWire holds it (read from `/proc`).
- [x] The internal card shows "Speakers". Connected HDMI shows the monitor's name; empty HDMI
      ports sit under "nothing connected · Show anyway".
- [x] Plugging the DAC in or out, then focusing the window, updates the list.
