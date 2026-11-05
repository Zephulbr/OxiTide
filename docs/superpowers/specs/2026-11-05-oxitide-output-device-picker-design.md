# OxiTide — Output device picker redesign

- **Status:** Implemented 2026-11-05 (Windows gates green); owner smoke and the Linux run
  pending. The design was agreed with the owner on 2026-11-05. Deviations are in §12.
- **Date:** 2026-11-05
- **Owner:** V
- **References:** `docs/agents/audio.md` (device fallback, ASIO, ALSA) ·
  `2026-10-03-oxitide-linux-support-design.md` · `plans/notes/2026-10-23-linux-bare-metal-run.md`
  (issue #11, the direct-ALSA picker) · `ui/design/DESIGN.md` (Settings page, menus)
- **Process note:** the design was worked out with the owner over three mockup rounds on
  2026-11-05. The approved mockup is described in §8. Its HTML was a scratch file and is not
  part of the repo.

---

## 1. Purpose & Outcome

The output picker is a plain `Select` fed by `{id, name, is_default}`. That causes these
problems:

1. **Raw names.** Windows shows `Speakers (2- MOONDROP Dawn Pro)`. ALSA shows its
   two-line `DESC`, which `hint_label` only trims, so the newline collapses into one long
   line. The 300 px trigger ends in an ellipsis with no tooltip.
2. **One card, several rows.** On Linux one card is listed as `sysdefault:CARD=X`, as
   `hw:CARD=X,DEV=n` "(direct hardware)" and possibly as `plughw:`, and every HDMI output
   gets a row. On Windows an ASIO driver sits beside the WASAPI endpoint of the same
   hardware.
3. **Two "defaults".** "System default" (follows the OS) and "X (default)" (locked to X)
   look the same. On Linux the PipeWire `default` alias adds a third.
4. **No way to tell what a device can do.** Exclusive support, rates and hardware volume
   show up only after playback starts.
5. **Exclusive is one global switch.** Moving between speakers and a DAC means flipping it
   every time. With an ASIO device selected it can read *off* while output is exclusive.
6. **No personalisation:** no nickname, pin, hide or icon.
7. **A stale list.** It loads once when Settings mounts.
8. **No quick switch.** The only way in is Settings → Playback.
9. **Bug:** `is_default` is matched by friendly name (`output.rs:523`), so two endpoints
   with the same name are both marked default.
10. **Disconnected devices show raw ids.** A stored device that is not connected shows
    `{0.0.0.00000000}.{…} (not connected)`.

**Success:**
- Settings → Playback → Output lists **one row per output**, grouped by type. Each row
  shows a clean name, one muted line saying how it connects, and a small **BP** tag when the
  device can play bit-perfect.
- The selected row expands to show its **own Shared/Exclusive mode**, the live format and
  what the device supports.
- Devices can be given a **nickname**, **pinned**, **hidden** and given an **icon**. Device
  icons can be turned off.
- The player bar's quality menu lists **pinned devices only**. **Output settings…** opens
  Playback and scrolls to Output.
- The list stays current: Windows pushes device events, and Linux refreshes when the
  picker opens.
- Nothing new runs while the app is idle.

## 2. Scope

**In scope:**
- **`oxitide-audio`:** a richer `OutputDevice`, a capability check per backend, Linux row
  grouping, a Windows device watcher, and the `is_default` fix.
- **`oxitide-app`:**
  - per-device settings in `config.toml` and their migration;
  - commands and DTOs;
  - applying the mode when the device changes;
  - the `output_devices_changed` event.
- **UI:**
  - `OutputDevicePicker.svelte`, which replaces the `Select` in Settings;
  - the pinned section in the `PlayerBar` quality menu;
  - a `setting` parameter on the `settings` route;
  - `api.ts` mirrors and `settingsIndex` entries.
- `ui/design/DESIGN.md`, tests, and a smoke checklist.

**Out of scope (§13):**
- a device switch on the phone remote (the output commands are `NOT_EXPOSED` today and
  stay so);
- a quick switch in the Hi-Fi Rack shell, which has no quality plate;
- routing Linux shared output to a specific PipeWire sink;
- drag-to-reorder for pins;
- an "identify" test tone.

## 3. Current state (read 2026-11-05)

| Area | Today |
|---|---|
| Enumeration | `output::enumerate_output_devices()`: WASAPI active render endpoints plus `asio_output::devices()` (registry), or ALSA `device_list(hints, direct_pcms)`. |
| DTO | `OutputDeviceDto {id, name, is_default}` (`player.rs:190`); `api.ts` `OutputDevice`. |
| Picker | `Settings.svelte:1243` `Select` with `outputDeviceOptions()` (`settingsView.ts:25`). |
| Exclusive | `config.audio.exclusive` (global), `set_exclusive` → `Engine::set_output_mode`; ASIO is forced exclusive by `output::effective_mode`. |
| Fallback | `config.audio.exclusive_fallback`, `set_exclusive_fallback`. |
| Device change | `set_output_device` → `Engine::set_output_device` (forces a reopen at the track position). |
| Remote | `get_output_devices`, `set_output_device`, `set_exclusive`, `set_exclusive_fallback` are all `NOT_EXPOSED`. |
| Diagnostics | Reports "system default", "an ASIO driver" or "a selected device", never a name (`commands.rs:2973`). |
| Quick access | The quality menu's **Output settings…** goes to `{name: "settings"}`, which opens the first tab. |

## 4. Decisions

| # | Decision | Rationale |
|---|---|---|
| DP-D1 | **One row per output, grouped by type:** *Windows audio* (WASAPI endpoints) and *ASIO* on Windows; *Sound cards* on Linux. ASIO drivers keep their own group and are never merged into a WASAPI row. | Owner choice. Matching ASIO drivers to endpoints by name is a guess; separate groups are always right. |
| DP-D2 | **On Linux, a row is a card + PCM device (`hw:CARD=X,DEV=n`).** Shared opens its `plughw:` twin (already `shared_pcm_name`) and Exclusive opens `hw:`. `sysdefault:*`, `plughw:*` and the `default` alias stop being rows. `default` becomes **System default**. Stored ids migrate (§6.3). | Fixes the "shows up several times" problem: these are routes, not devices. |
| DP-D3 | **Exclusive is a per-device mode** (`shared` / `exclusive`), stored per device id (including `""`, System default). The current global `exclusive` value moves to the selected device once. Every other device starts Shared. ASIO rows have no mode switch: they are always exclusive. | Owner choice. Each device keeps the mode that suits it. |
| DP-D4 | **"If exclusive fails" stays one global setting**, shown under the list while any device uses Exclusive or an ASIO device is selected. | Owner choice. It's a preference about behaviour, not about a device, and repeating it on each row was clutter. |
| DP-D5 | **Capabilities come from an active check, cached per session** (§7). Rows show a **BP tag** (check icon + "BP", tooltip "Bit-perfect capable device") when exclusive output was found. The selected row also shows the top rate and bit depth and whether there is hardware volume. If a device can't be checked, the tag shows the last capability recorded from a real playback, or no tag. | Owner choice: active checks rather than learned only. A tag is clearer than the word, and the tooltip explains it. |
| DP-D6 | **Nickname, display only.** Edited inline. The field's placeholder is the device's own name, and saving it empty brings that name back. The full system name is the row's tooltip. No "Real name:" line. Nicknames never reach diagnostics or logs. | Owner choice. It's a cosmetic label, and clearing it is the only reset needed. |
| DP-D7 | **Pins** add the device to the quality menu, in pin order (most recently pinned last). A pinned device that's disconnected stays listed, disabled. Unpinned rows show the pin button on hover or focus. | Owner choice: quick switch for pinned devices only. |
| DP-D8 | **Hide** removes a row from the list. "N hidden · Show" at the bottom reveals hidden rows. Hiding the selected device is refused ("Switch to another device first"). | Less clutter from virtual devices and wrappers. |
| DP-D9 | **Icons are optional.** `ui.device_icons` (default on) shows a 32 px icon tile on each row and an icon in the quality menu. Each device's icon is `auto` (from its type), a named Lucide icon, or `none`. | Owner choice: icons are up to the user. |
| DP-D10 | **Clean names** (§6.1), deduplicated. When two rows would share a label, both use the long form. | Fixes the raw-name problem. |
| DP-D11 | **Devices that disappear stay listed** while they are selected or pinned, marked "Not connected", with the last name seen. Others drop off the list. | The choice survives an unplug (as `outputDeviceOptions` does today), without raw ids. |
| DP-D12 | **Keeping the list current costs nothing while idle.** Windows registers an `IMMNotificationClient` at startup and emits `output_devices_changed` (debounced 300 ms) on add, remove, state or default changes. Linux has no watcher: it re-enumerates when the Settings Output card mounts, when the window regains focus while it's shown, and when the quality menu opens. | Efficiency rules: a paused, idle app does no work. |
| DP-D13 | **The selection and the mode switch together.** `set_output_device` reads the device's stored mode, calls `Engine::set_output_mode` and then `Engine::set_output_device`, so there is one reopen. `set_device_mode` on the selected device reopens the same way. | One restart at the position, never two. |
| DP-D14 | **Commands:** `get_output_devices` stays async and returns the new DTO. New: `set_device_prefs(id, patch)` (sync, config writer) and `set_device_mode(id, mode)` (sync, transport + config). `set_exclusive` is removed (§6.4). All of them are `NOT_EXPOSED`. | AGENTS.md: config writers and transport commands stay sync. The phone has no UI for these yet. |
| DP-D15 | **`settings` route gains `setting?: string`.** Settings opens that entry's tab and runs the search jump (`jumpTo`). **Output settings…** passes `"device"`. | Reuses the search's reveal (scroll, ring, focus). |

## 5. Device model

### 5.1 `oxitide-audio`

```rust
pub enum DeviceGroup { Wasapi, Asio, Alsa }
pub enum DeviceKind { Speakers, Headphones, Headset, Hdmi, Spdif, Line, Bluetooth, Usb, Virtual, Unknown }
pub enum Transport { Usb, Bluetooth, Hdmi, Internal, Virtual, Unknown }

pub struct OutputDevice {
    pub id: String,              // unchanged ids: WASAPI endpoint id, "asio:<key>", "hw:CARD=X,DEV=n"
    pub name: String,            // clean display name (§6.1)
    pub system_name: String,     // the OS's full name, for the tooltip and nickname placeholder
    pub group: DeviceGroup,
    pub kind: DeviceKind,
    pub transport: Transport,
    pub is_default: bool,        // compared by id, not name (fixes #9)
    pub wrapper: bool,           // ASIO4ALL, FlexASIO, Realtek ASIO, … (known keys)
}

pub struct DeviceCaps {
    pub exclusive: bool,
    pub max_rate: Option<u32>,
    pub max_bits: Option<u16>,
    pub hw_volume: Option<bool>,
    pub shared_format: Option<(u32, u16)>,   // WASAPI mix format
    pub source: CapsSource,                  // Probed | Played | Unknown
}
```

`enumerate_output_devices()` stays fast (no driver loads). `probe_caps(&OutputDevice) ->
DeviceCaps` is separate and may block (§7).

### 5.2 DTO (`player.rs`) and `api.ts`

```ts
export interface OutputDevice {
  id: string; name: string; system_name: string;
  group: "wasapi" | "asio" | "alsa";
  kind: "speakers" | "headphones" | "headset" | "hdmi" | "spdif" | "line" | "bluetooth" | "usb" | "virtual" | "unknown";
  transport: "usb" | "bluetooth" | "hdmi" | "internal" | "virtual" | "unknown";
  is_default: boolean; connected: boolean; wrapper: boolean;
  caps: DeviceCaps | null;
  prefs: DevicePrefs;            // merged from config, so the UI needs one call
}
export interface DeviceCaps { exclusive: boolean; max_rate: number | null; max_bits: number | null; hw_volume: boolean | null; source: "probed" | "played" | "unknown" }
export interface DevicePrefs { nickname: string; icon: string; pinned: boolean; hidden: boolean; mode: "shared" | "exclusive" }
export interface OutputDeviceList { system_default: { now: string | null; mode: "shared" | "exclusive" }; devices: OutputDevice[]; pinned: string[] }
```

`get_output_devices` returns `OutputDeviceList`. Disconnected devices that are selected or
pinned are added from config with `connected: false` (DP-D11).

## 6. Backends

### 6.1 Names and types

- **WASAPI.** Read from the endpoint property store:
  - `PKEY_Device_DeviceDesc` (endpoint: "Speakers", "LG TV SSCR2");
  - `PKEY_DeviceInterface_FriendlyName` (adapter: "MOONDROP Dawn Pro", "Realtek(R) Audio");
  - `PKEY_AudioEndpoint_FormFactor`, which gives `kind`.

  `transport` comes from the device's instance id prefix: `USB\`, `BTHENUM\`/`BTHHFENUM\`,
  `HDAUDIO\` with an HDMI/DisplayPort form factor (HDMI), other `HDAUDIO\` (Internal), and
  `SWD\MMDEVAPI` or root-enumerated devices (Virtual). Plan Task 1 verifies which property
  carries it.

  Name rule:
  - HDMI/DisplayPort and Internal → the endpoint name ("LG TV SSCR2", "Speakers");
  - USB and Bluetooth → the adapter name ("MOONDROP Dawn Pro", "WH-1000XM5");
  - strip a leading `\d+- `;
  - if two rows collide, both use `"{endpoint} ({adapter})"`.
- **ASIO.** The registry description, without the "(ASIO)" suffix (the group says it).
  `kind = Usb` if the description contains "USB", else `Unknown`. `wrapper` comes from a
  small list of known driver keys or CLSIDs (ASIO4ALL, FlexASIO, Realtek ASIO, ASIO2WASAPI).
- **ALSA.** `name` is the card name (`MOONDROP Dawn Pro`). When a card has more than one
  playback device, the PCM name is added ("HDA Intel PCH — ALC3246 Analog" becomes
  "Speakers" for the analog device of an internal codec). HDMI devices take the monitor
  name from ELD (`/proc/asound/cardN/eld#*`: `monitor_name`), matched by HDMI index; if no
  match is certain, the name is "HDMI n". HDMI devices whose ELD says `monitor_present 0` are
  grouped under "HDMI 0, 2, 3: nothing connected · Show anyway". `transport` is USB when
  `/proc/asound/cardN/usbid` exists, HDMI by PCM name, otherwise Internal.

### 6.2 Windows `is_default` and watcher

- `is_default` compares `get_default_device_for_role(Render, Console)`'s **id**.
- `DeviceWatcher` (in `wasapi_output.rs` or a new `device_watch.rs`) registers one
  `IMMNotificationClient` for the app's lifetime. Callbacks set a flag and wake one debounce
  thread, which emits `output_devices_changed` and drops any cached caps for that id. It is
  separate from the fallback session's own watch (`needs_reopen`), which stays as it is.

### 6.3 Config and migration

```toml
[audio]
output_device = "hw:CARD=Pro,DEV=0"
exclusive_fallback = "shared"
pinned = ["hw:CARD=Pro,DEV=0", "hw:CARD=PCH,DEV=0"]
# `exclusive` is read once for migration, then no longer written

[audio.devices."hw:CARD=Pro,DEV=0"]
name = "MOONDROP Dawn Pro"    # last clean name seen (for "Not connected" rows)
nickname = "Desk DAC"
icon = "auto"                 # auto | none | <icon name>
hidden = false
mode = "exclusive"

[ui]
device_icons = true
```

- `AudioConfig.devices: BTreeMap<String, DevicePrefsConfig>` with `#[serde(default)]`.
  Entries that hold only defaults are not written.
- **Exclusive migration:** if the file has `exclusive = true` and no `devices` entry for
  `output_device`, that entry gets `mode = "exclusive"` (System default included). The
  field is then dropped on the next save.
- **ALSA id migration:**
  - `sysdefault:CARD=X` and `plughw:CARD=X,DEV=n` become `hw:CARD=X,DEV=n` (n = 0 for
    sysdefault);
  - `default` becomes `""`;
  - `hw:0,0`-style numeric ids become `hw:CARD=<id>,DEV=0` when the card is present.
  - The same mapping runs on `pinned` and the `devices` keys.
- **Id drift (Windows USB port changes):** when a stored id is missing but a connected
  device in the same group has the same `system_name` and transport and no prefs of its own,
  the prefs move to the new id. The selection follows only if it was on the old id.

### 6.4 Commands

| Command | Kind | Notes |
|---|---|---|
| `get_output_devices() -> OutputDeviceList` | async | Enumerates, merges prefs and cached caps, and starts checks for devices without caps (§7). |
| `set_output_device(device_id)` | sync | Unchanged signature; applies the stored mode first (DP-D13). |
| `set_device_mode(device_id, mode)` | sync | Saves the mode. If the device is selected, reopens. ASIO ids are refused. |
| `set_device_prefs(device_id, patch: {nickname?, icon?, pinned?, hidden?})` | sync | Pin appends to or removes from `pinned`. Hiding the selected device is refused. |
| `set_device_icons(enabled)` | sync | `ui.device_icons`. |
| `set_exclusive` | removed | Its only caller was the Settings toggle. Removed from `generate_handler!`, `SYNC_COMMANDS` and `NOT_EXPOSED`. |

Event: `output_devices_changed` (no payload; the UI calls `get_output_devices` again). The
same event is emitted when a capability check finishes.

## 7. Capability checks

| Backend | Check | Cost and limits |
|---|---|---|
| WASAPI | `IAudioClient::IsFormatSupported(EXCLUSIVE, …)` over 44.1–384 kHz × 16/24/32-bit integer, highest first; `GetMixFormat` for `shared_format`; `IAudioEndpointVolume::QueryHardwareSupport` for `hw_volume`. No stream is opened. | A few ms per device. Runs on a worker thread. A device held exclusively by another app reports what it can, or Unknown. |
| ASIO | Load the driver on the `oxitide-asio` owner thread: `canSampleRate` over the same rates, the output channel sample type for `max_bits`. `hw_volume = false`. | Slow and loads a driver. **Skipped while an ASIO stream is open** (only one driver can be loaded at a time). One driver at a time, and only when the list is requested. |
| ALSA USB | Parse `/proc/asound/cardN/stream0` (`Format:` and `Rates:` of the playback interface). `hw_volume` from the mixer (`Master`/`PCM` playback volume). | File read only. Works while PipeWire holds the card. |
| ALSA other | Open `hw:` with `SND_PCM_NONBLOCK` and refine `hw_params` for the max rate and format, then close. If the open fails (EBUSY under PipeWire), Unknown. | Short open. Never while OxiTide's own stream is open on that card. |
| Learned | Every exclusive open that succeeds records `{rate, bits, hw_volume}` for that id as `source: Played`. A probe result replaces it. | Free. Covers ASIO and busy ALSA cards. |

Results are cached in memory per session and dropped on that device's watcher event. The
learned record is saved under `audio.devices.<id>` (`played_rate`, `played_bits`), so a busy
card shows its tag after a restart.

## 8. UI

### 8.1 Settings → Playback → Output (`OutputDevicePicker.svelte`)

- **Layout.** The Output card holds:
  - a **System default** row (icon `monitor-speaker`, sub line "Follows Windows · now
    Speakers" / "Follows the system · now PipeWire");
  - then the group labels and rows;
  - then "N hidden · Show";
  - then two plain `SettingRow`s: **If exclusive fails** (Segmented, DP-D4) and **Device
    icons** (Toggle).
- **The list** is a `role="radiogroup"` with roving focus. Each row is a `role="radio"`,
  with the BP tag and the connection line in its accessible description.
- **Row anatomy** (min height 52 px, radius 10 px):
  - optional 32 px icon tile;
  - the name (14 px, 600, `--text`, ellipsis, tooltip = `system_name`), followed by the
    **BP tag**: an 11 px check + "BP", `--text-muted` on a `--surface-3` pill. It is muted
    because the player bar's accent BP means *playing* bit-perfect, while this one means
    *can*;
  - the sub line (12.5 px, 500, `--text-muted`): the transport ("USB", "HDMI", "Internal",
    "Bluetooth"), "Always exclusive" for ASIO, "Wrapper over Windows audio" for wrappers,
    "Not connected" when disconnected;
  - the pin button (shown when pinned, in `--text`; otherwise on hover/focus);
  - the ⋯ button.
- **The selected row:**
  - an accent ring (45 %) and a 7 % accent wash;
  - the icon tile in accent;
  - below a hairline, the **Shared | Exclusive** `Segmented` on the left (not for ASIO);
  - on the right, two lines of 12.5 px muted text, with the values in `--text` and
    tabular numerals: "Now **96 kHz · 24-bit**, bit-perfect" and "Up to 384 kHz · 32-bit,
    hardware volume". Shared-only devices show the shared format instead.
- **The ⋯ menu** (shared `ContextMenu`): **Nickname…**, **Pin** / **Unpin**, **Hide** /
  **Show**, then an **Icon** section of seven choices (auto-detected first, then speaker,
  headphones, monitor, usb, bluetooth, audio-lines, and **None**).
- **Editing a nickname.** The name turns into an input (placeholder = the clean
  `name`) with **Save** (accent) and **Cancel**. Enter saves, Escape cancels, and saving it
  empty clears the nickname.
- **Disconnected rows** dim their tile, name and sub line to 50 %. They can't be selected
  unless they already are.
- **Accent** is used only for the selected row, the active segment and the Device icons
  toggle.
- **Motion.** The selected row's detail opens with `--dur-state` (opacity and transform
  only). Under reduced motion it appears at once.

### 8.2 Player bar quality menu

- **The section.** After the "Now playing" note and before **Output settings…**, an
  **Output** section lists `pinned` devices in order: icon (if icons are on), the display
  name, and the mode on the right ("Exclusive" / "Shared"; ASIO "ASIO"). The current
  device is `selected`. A disconnected pin is disabled, with "Not connected".
- **Nothing pinned.** No section.
- **Picking a device** calls `set_output_device`.
- **Output settings…** goes to `{name: "settings", setting: "device"}`.

### 8.3 Settings search

- Keep `device`.
- Replace `exclusive` with `device-mode` ("Exclusive mode (bit-perfect)", keywords
  `wasapi asio exclusive bit-perfect mixer`). It jumps to the selected row's segment.
- `exclusive-fallback` keeps its id. Its `needs: "exclusive"` now means "any device uses
  Exclusive, or an ASIO device is selected".
- Add `device-icons`.

## 9. Visual design notes

- Semantic tokens only, and no mode branches. Rack picks up the card through `pane` and the
  shared `Segmented` / `ContextMenu` skins.
- Type follows the Settings scale: names 14/600, sub lines and facts 12.5/500, group
  labels 11/700 uppercase with 0.06em tracking in `--text-muted`. Every value is tabular.
- Contrast: `--text-muted` on the card is about 6:1 in Atmosphere dark, so no token
  changes. The BP tag (muted on `--surface-3`) stays above 4.5:1. The light theme and
  Studio are checked in the smoke run.
- Record the picker, the BP tag and the quality-menu section in `ui/design/DESIGN.md`
  (Settings page and Atmosphere §3 player bar).

## 10. Testing

- **Rust:**
  - `alsa_output`: `device_list` collapses sysdefault, plughw and hw into one row per
    card + device; `default` is dropped; multi-device card names; ELD parse and "not
    connected" grouping; `/proc/.../stream0` parse (fixtures from the Dawn Pro and an HDA
    card).
  - WASAPI name rule and dedupe as pure functions over property tuples; `is_default` by
    id.
  - Config: exclusive migration (selected device, System default), ALSA id migration,
    id-drift move, default entries not written.
  - Commands: `set_output_device` calls `set_output_mode` before `set_output_device`
    (recording engine); `set_device_mode` reopens only the selected device; ASIO mode is
    refused; hiding the selected device is refused; `SYNC_COMMANDS` / `NOT_EXPOSED`
    classification; the `set_exclusive` removal.
- **Node:** list shaping (groups, hidden count, disconnected pinned rows, display-name and
  nickname fallback, dedupe of labels), quality-menu items from `pinned`.
- **jsdom (`OutputDevicePicker`):**
  - arrow keys move the selection;
  - Nickname: save, Escape, save empty → system name;
  - pin and hide;
  - the BP tag has its tooltip and accessible description;
  - icons off hides the tiles;
  - ASIO rows have no mode switch.
- **Smoke (owner):** `plans/notes/2026-11-xx-device-picker-smoke.md`.
  - Windows: Realtek + USB DAC + HDMI + ASIO; nickname, pin, quick switch, per-device mode
    across a switch, unplug and replug while pinned, live list refresh.
  - Linux: Dawn Pro one row, exclusive BP, shared through `plughw:`, HDMI names.
  - Light theme, Studio and Rack.

## 11. Risks

- **The WASAPI transport property** (§6.1) is unverified. Plan Task 1 checks it on the
  owner's devices. If it's missing, transport is Unknown and the sub line falls back to the
  form factor ("Speakers", "Headphones").
- **ASIO check cost:** some drivers take a second to load or open a control panel. Checks
  run one driver at a time on demand and are skipped while ASIO plays. A driver that hangs
  past 3 s is marked Unknown for the session.
- **ELD matching** between `eld#c.p` and the HDMI PCM index is a best guess. If it's
  unsure, the row is named "HDMI n".
- **Linux shared output on a busy card:** a specific card in Shared mode still opens
  `plughw:` directly, which fails under PipeWire and falls back to the default with the
  existing notice. That is the same as today, and §13 tracks routing through PipeWire.
- **Migration** rewrites ids users may have edited by hand. The mapping is lossless for the
  shapes OxiTide has ever written, and unknown ids are kept as-is.

## 12. Implementation notes (2026-11-05)

Where the build differs from the text above:
- **Task 1 confirmed transport detection** (`plans/notes/2026-11-05-device-properties-spike.md`):
  - `PKEY_Device_EnumeratorName` and `FormFactor` sit on the endpoint store;
  - vendor USB drivers use their own enumerator (`TUSBAUDIO_ENUM`), so "contains USB" is the
    rule;
  - HDMI endpoints carry a Windows display number (`4 - LG TV`), which is stripped;
  - the endpoint name wins unless it is generic ("DX5" over "TOPPING USB DAC", but "HyperX
    DuoCast" over "Speakers").
- **No `device-mode` search entry.** "Exclusive", "bit-perfect" and "nickname" find the
  `device` row, which holds the mode switch. `exclusive` was removed from the index, and
  `device-icons` added.
- **The deep link is a one-shot request** (`lib/settingsJump.svelte.ts`), not a
  `setting` field on the route (DP-D15). Going to Settings while it is open is a no-op in
  `nav`, and a route field would be persisted with the history.
- **Id drift is applied by the UI.** `get_output_devices` stays read-only and returns
  `moved` pairs; the picker calls the new sync `move_device_prefs` once per pair per session.
  Config writers must not run from an async command.
- **The list DTO carries `selected` and `icons`**, so the quality menu needs no config read.
- **Icon choices are menu rows**, not a grid: the shared ContextMenu renders rows.
- **Learned capabilities** live in `config\device_caps.json`, not `config.toml`. They are
  recorded only from bit-perfect exclusive states without a notice, since a notice means
  the open fell back to another device.
- **The device playing exclusively is not checked** while it plays (it would look busy).
  It is checked on the next list after playback moves off it.
- **ASIO check limit:** a driver that hangs while loading keeps the one-driver slot until it
  returns. If it never returns, ASIO opens fail until restart. This has not been seen on the
  owner's drivers: Realtek ASIO doesn't initialise for a check (no caps), and the Topping
  driver answers in about 50 ms.
- **Linux was type-checked only for its `alsa` calls.** WSL isn't installed on the build
  machine; the calls were checked against the `alsa` crate for the Linux target, but there
  had been no full Linux build or test run on it. The owner has since run the Linux gates
  and smoke rows, and they pass.

**Owner review of the first build (2026-11-05):**
- **The BP tag is gone.** Nearly every device on the owner's machine got it, so it told
  nothing apart. The capability checks stay: they feed the selected row's "Up to …" line.
  This supersedes the BP tag in §3, §8.1 and §9.
- **The selected device stays at the top of its group**, outside the scroll, and the group's
  other rows scroll under it (216px under a selected row, 280px otherwise; the cut row hints
  there is more). Selecting a row moves it to the top, and focus follows. Crossfade moved to the right column
  above AutoMix. Together these stop the Playback columns from stretching the Equalizer card.
- **Pinned devices come first** under the selected row, in pin order, with no extra styling.
- **Icons are bare glyphs** (no plate), so they don't read as card headers. Three icon
  choices were added: DAC / Amp (`radio-receiver`), TV and Laptop.
- **Picker errors name the action.** A backend error reads "Couldn't switch to DX5: …". A
  page bug (a JS exception) shows only "Couldn't read the output devices. Reopen Settings to
  try again." and goes to the console. A dev hot reload mid-edit had surfaced a raw
  `ReferenceError` as a toast.
- **No "Now 44.1 kHz · 16-bit, bit-perfect" line.** The player bar already shows it; the
  detail keeps only the "Up to …" line.
- **Enter in the nickname field only saves.** Saving returns focus to the row's radio before
  the key finished, so Enter also pressed it and switched devices; the field now consumes
  Enter and Escape.
- **The detail's numbers are in the accent** ("Up to **192** kHz · **24**-bit"); the units stay
  muted.
- **Microphone headphone jacks are not filtered.** The HyperX DuoCast shows because the mic
  has a real headphone output for monitoring, which Windows lists as a render endpoint
  ("Speakers"). Nothing in its properties tells it apart from a USB headset or an audio
  interface: the same form factor (Speakers), the same USB enumerator, and a container shared
  with a capture endpoint. A filter on those would also hide a Focusrite-style interface.
  **Hide** in the ⋯ menu is the answer.

## 13. Follow-ups

- Device switch on the phone remote (classify the commands, add a sheet).
- A quick switch in the Rack shell (Amp unit).
- Linux shared output routed to the chosen PipeWire sink.
- Drag-to-reorder pins; an "Identify" chime per device.
