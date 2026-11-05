# Output Device Picker Implementation Plan

> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** replace the output `Select` with a device list.
- **Rows:** one row per output, grouped by type, with a clean name and the connection. (The
  BP tag was built, then removed after the owner's review; see spec §12.)
- **Per device:** a Shared/Exclusive mode, nickname, pin, hide and icon.
- **Quick switch:** pinned devices in the player bar's quality menu.

**Architecture:**
- **`oxitide-audio`:** describes devices (group, kind, transport, clean name), checks what
  they can do, and on Windows watches for device changes.
- **`oxitide-app`:**
  - per-device prefs in `config.toml` (with migration);
  - the mode applied together with the selection;
  - `output_devices_changed`.
- **UI:**
  - `OutputDevicePicker.svelte` in Settings;
  - an Output section in the quality menu;
  - `settings` route deep link.

**Spec:** `docs/superpowers/specs/2026-11-05-oxitide-output-device-picker-design.md`

**Status (2026-11-05):**
- Tasks 1–11 are implemented. The Windows gates are green: `cargo test --workspace`,
  `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, and
  `npm --prefix ui test` (1275 tests).
- The listing and capability checks were run against the owner's real devices.
- The build machine had no WSL, so the Linux code was first only type-checked for its
  `alsa` calls; the owner then ran the Linux gates and rows.
- Task 12 done: the owner smoke (`notes/2026-11-05-device-picker-smoke.md`) passed on
  Windows and Linux, after the review changes in spec §12.
- Deviations from the spec are in spec §12.

## Global constraints

- **Do not commit.** The owner commits.
- **No new dependencies.** Windows property keys come from the existing `windows` crate,
  and ALSA uses the existing `alsa` crate and `/proc` reads.
- **Read before touching:** `docs/agents/audio.md`, `docs/agents/app-shell.md`,
  `docs/agents/ui.md`.
- **Styling:** semantic tokens only, no mode branches, inline Lucide icons, no glyphs.
- **Commands:** each new command goes in `generate_handler![]`, in `SYNC_COMMANDS` with its
  reason when sync, and in remote `NOT_EXPOSED`.
- **Idle:** nothing new runs while the app is idle (spec DP-D12).
- **Gates:**
  - `cargo test --workspace` and `cargo clippy --workspace -- -D warnings`, on Windows
    and in WSL (Linux code);
  - `npm --prefix ui run check` and `npm --prefix ui test`.

## Tasks

- [x] **Task 1: Windows property spike (owner machine).**
  - Dump `PKEY_Device_DeviceDesc`, `PKEY_DeviceInterface_FriendlyName`,
    `PKEY_AudioEndpoint_FormFactor`, `PKEY_Device_InstanceId` and the enumerator name for
    each render endpoint (Realtek, the USB DAC, HDMI, Bluetooth if present).
  - Time `IsFormatSupported` (exclusive) over the rate × depth grid for one device.
  - Record the results in `notes/2026-11-xx-device-properties-spike.md`. This settles
    spec §6.1 transport detection (risk §11).
- [x] **Task 2: device model in `oxitide-audio`.**
  - `DeviceGroup`, `DeviceKind`, `Transport`, and the new `OutputDevice` fields (spec
    §5.1).
  - Fix `is_default` to compare by id.
  - Name rule and dedupe as pure functions (`device_names.rs`) with unit tests.
  - Update `asio_output::devices()`: no "(ASIO)" suffix, plus the `wrapper` list.
- [x] **Task 3: ALSA rows.**
  - Rewrite `device_list` so each card + device is one `hw:CARD=X,DEV=n` row: drop
    `sysdefault`, `plughw` and `default` rows, name multi-device cards, set transport
    from `usbid`.
  - ELD parse (`monitor_name`, `monitor_present`) and HDMI matching.
  - Fixture tests (Dawn Pro, HDA with 4 HDMI). Replace today's `device_list_*` tests.
- [x] **Task 4: capability checks.**
  - `DeviceCaps` and `probe_caps`: WASAPI `IsFormatSupported`, mix format and hardware
    volume; ASIO on the owner thread with a 3 s guard, skipped while ASIO is open; ALSA
    `/proc/.../stream0` parse and a non-blocking `hw_params` refine.
  - The learned record from successful exclusive opens (engine → status hook).
  - Unit tests for the parsers and the rate-grid selection.
- [x] **Task 5: Windows device watcher.**
  - App-lifetime `IMMNotificationClient` with a 300 ms debounce thread. It emits through a
    callback that `oxitide-app` turns into `output_devices_changed` and uses to drop cached
    caps.
  - Leave the fallback session's own watch alone.
- [x] **Task 6: config and migration.**
  - `AudioConfig.devices` (`BTreeMap`), `pinned`, `ui.device_icons`. Entries that hold only
    defaults are skipped when saving.
  - Migrations:
    - `exclusive = true` → the selected device's mode;
    - ALSA ids (`sysdefault`, `plughw`, `default`, numeric `hw:`) across
      `output_device`, `pinned` and `devices`;
    - id drift by `system_name` + transport.
  - Startup (`commands.rs` ~437) reads the selected device's mode instead of
    `audio.exclusive`.
  - Tests for each migration.
- [x] **Task 7: commands.**
  - `get_output_devices` returns `OutputDeviceList` (prefs merged, disconnected
    selected/pinned devices added, checks started).
  - `set_output_device` applies the mode first (one reopen).
  - New: `set_device_mode`, `set_device_prefs`, `set_device_icons`.
  - Remove `set_exclusive` (commands, `main.rs`, `SYNC_COMMANDS`, `NOT_EXPOSED`).
  - Tests: mode-before-device ordering, ASIO mode refused, hiding the selected device
    refused, classification lists.
- [x] **Task 8: UI API and list logic.**
  - `api.ts` types (spec §5.2) and calls; remove `setExclusive`.
  - `lib/outputDevices.ts`:
    - pure shaping (groups, hidden count, display name = nickname || name, label dedupe);
    - quality-menu items from `pinned`.
  - Node tests. Delete `outputDeviceOptions` and its tests.
- [x] **Task 9: `OutputDevicePicker.svelte`.**
  - Radiogroup rows with roving focus, the BP tag (tooltip "Bit-perfect capable device"
    plus accessible description), and the selected-row detail (mode `Segmented`, facts).
  - The ⋯ `ContextMenu` (Nickname…, Pin, Hide, Icon) and inline nickname editing.
  - Hidden reveal, disconnected rows, icons toggle.
  - Listens for `output_devices_changed`; re-enumerates on mount and on focus (Linux).
  - Replace the Output section's `Select` and exclusive toggle in `Settings.svelte`. Keep
    **If exclusive fails** under the list (spec DP-D4).
  - jsdom tests (spec §10).
- [x] **Task 10: quality menu and deep link.**
  - `PlayerBar.qualityItems`: an Output section from `pinned`, re-enumerated when the menu
    opens.
  - `Route` `settings` gains `setting?: string`. `Settings.svelte` runs `jumpTo` for it on
    mount, and **Output settings…** passes `"device"`. Update the nav restore parsing.
  - `settingsIndex`: `device-mode` replaces `exclusive`; update `exclusive-fallback`'s
    `needs`; add `device-icons`. Update the `settingsIndex` tests.
  - Update the PlayerBar test.
- [x] **Task 11: design docs.**
  - `ui/design/DESIGN.md`: the Settings page device list, the BP tag (muted = capable vs
    accent = playing), and the quality-menu Output section.
  - Atmosphere §3 if the plate menu is described there.
  - Update `docs/agents/audio.md` (ALSA rows, per-device mode, watcher) and
    `docs/agents/ui.md` if it mentions the picker.
- [x] **Task 12: smoke checklist (owner).**
  - `notes/2026-11-xx-device-picker-smoke.md` per spec §10, Windows then Linux.
