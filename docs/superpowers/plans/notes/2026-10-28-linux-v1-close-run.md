# Linux close-out run for v1.0 (2026-10-28)

- **Machine:** CachyOS laptop (`Vivobooo`), KDE Plasma Wayland, PipeWire. Same host as
  `2026-10-23-linux-bare-metal-run.md`.
- **Code:** `main` at `8fdfd39`. It includes the §14 fixes from `01b92b8` (direct ALSA picker,
  MPRIS Stop/Volume), and everything merged after 2026-10-23 (autoplay, undo, Cat settings, etc.).
- **Goal:** close the Linux items in `2026-10-17-v1-release-gate.md` §6, so that the only things
  left before the `v1.0.0` tag are the owner's decisions.
- **Out of scope:** tagging `v1.0.0` (outward-facing, owner's call), the Windows rerun of the
  Linux-touched code, and perf budget calibration (PM-D1, separate gate item).

## Plan

Each step's result is recorded under **Results** below. Hands-on rows are labelled `[HUMAN]`.

1. **Sync and environment.** Confirm `HEAD` contains `01b92b8` and the tree is clean. Use the
   toolchains from `.superpowers/linux-run/` (`RUSTUP_HOME`, `CARGO_HOME`, `node24`).
2. **Automated gates on `8fdfd39`:**
   - `cargo test --workspace`
   - `cargo clippy --workspace -- -D warnings`
   - `npm --prefix ui run check`
   - `npm --prefix ui test`
   - `node --test scripts/release/release.test.mjs`
   - `npm --prefix ui run app:build` (deb + AppImage), then deb metadata and AppImage extract.
3. **Machine-checkable §14.3 rows** (no DAC needed):
   - MPRIS: `playerctl stop` → `Stopped`, position 0, metadata kept; `playerctl play` resumes from 0;
     `playerctl volume 0.35` moves the slider.
   - SNI `Title` is `OxiTide` (D-Bus StatusNotifier item).
   - Logs land in `~/.config/oxitide/logs/`.
   - After a close and reopen, `config.toml` stores `window_width = 900` for a 900-wide window.
   - Launch the release build from `target/release` and confirm it starts and reaches the main window.
4. **Audio rows that need the DAC** `[HUMAN]`: the MOONDROP Dawn Pro is **not connected** at the
   start of this run (`/proc/asound/cards` lists only the internal HDA and ACP cards). When it is
   plugged in, check: picker lists `MOONDROP Dawn Pro, USB Audio (direct hardware)`; exclusive on
   gives `✓ BP` with no hand-edited `config.toml`; exclusive off plays through `plughw:` with no
   notice; HDA `hw:` opens exclusive at 44.1 kHz; unplug and replug (fallback notice, then recovery).
5. **Rows that stay with the owner** `[HUMAN]`: listening check, `tidal://` and single instance (already
   PASS, optional re-check), the AT-SPI empty tree (issue #10, accept as debt or fix), phone pairing,
   and the Linux mix artwork check (issue #8).
6. **AUR (release gate §6):** the `v0.4.0` tag is pushed and its release carries the rendered
   `PKGBUILD`. Run `makepkg` in a scratch directory (no `-si`, so no sudo) against that tag, to
   confirm the source fetches and the package builds. `makepkg -si` stays with the owner.
7. **Record and close.** Write the results below, tick the rows in `2026-10-23-linux-bare-metal-run.md`
   §14.3 that passed, and update the Linux lines of `2026-10-17-v1-release-gate.md` §6. The rows that
   cannot run today are listed as open, with the reason.

## Open choices (owner)

- If the DAC rows cannot run before the tag: label Linux "preview" in the 1.0 notes, or hold the tag.
- Issue #10 (empty AT-SPI tree on WebKitGTK): accept as documented debt, or fix before 1.0.
- Issue #4 (MPRIS `DesktopEntry` per package): verifiable only after installing the deb or AUR package.

## Results

Run on `main` at `8fdfd39`, tree clean before the run. Logs are in the session scratchpad, not
committed.

### Step 2: automated gates (all PASS)

| Gate | Result |
|---|---|
| `cargo test --workspace` | **PASS**: 1148 passed, 0 failed (was 1092 on 2026-10-23) |
| `cargo clippy --workspace -- -D warnings` | **PASS**: no warnings |
| `npm --prefix ui run check` | **PASS**: 700 files, 0 errors, 0 warnings |
| `npm --prefix ui test` | **PASS**: 152 files, 1245 tests (was 151 / 1228) |
| `node --test scripts/release/release.test.mjs` | **PASS**: 12 of 12 |
| `npm --prefix ui run app:build` | **PASS**: `OxiTide_0.4.0_amd64.deb` (11.06 MB, mtime 17:02), `OxiTide_0.4.0_amd64.AppImage` (121 MB) |
| deb control and desktop entry | **PASS**: `Package: oxi-tide`, `Version: 0.4.0`, `amd64`; `Exec=oxitide-app`, `StartupWMClass=oxitide-app`, `MimeType=x-scheme-handler/tidal;` |
| AppImage extract | **PASS**: `--appimage-extract` OK; `OxiTide.desktop` matches the deb entry |

`dpkg-deb` is not installed on this CachyOS box, so the deb was read with `ar` and `tar`.

### Step 3: §14.3 rows that need no DAC

| Row | Result |
|---|---|
| Logs land in `~/.config/oxitide/logs/` | **PASS**: `oxitide.2026-10-28.log` written. Nothing new under the stale `~/.config/OxiTide/`. |
| `playerctl stop` → `Stopped`, pos 0, metadata kept | **PASS** |
| `playerctl play` resumes from 0 | **PASS**: pos 3.25 s after 3 s of play |
| `playerctl volume 0.35` moves the app | **PASS on state**: `playerctl volume` reads 0.35. The slider drawing is visual; see the owner list. |
| SNI `Title` is `OxiTide` | **PASS**: `Title = "OxiTide"` on the watcher-registered `tray_icon_tray_app_main` item |
| `config.toml` `window_width = 900` after close/reopen | **PASS**: `window_width = 900` after the window was closed and saved. A full process restart restores 900×620 (the frame size; the saved content height is 592). |
| MPRIS `DesktopEntry` names an installed `.desktop` | **PARTIAL**: advertises `OxiTide`, and a user-dir `OxiTide.desktop` exists. It is a stale install from 23 Oct (`Exec=/home/v/.local/bin/oxitide`), so the per-package check still needs the deb installed. |

### Step 4: DAC rows: NOT RUN

The MOONDROP Dawn Pro is **not connected** (`/proc/asound/cards` lists only the internal HDA and
ACP cards). Exclusive, picker, `plughw:` fallback, HDA exclusive, and unplug/replug all wait for the
DAC.

### Step 6: AUR: FAIL as published, passes with a GitHub source (needs an owner decision)

- The release's `PKGBUILD` (v0.4.0, tag `6f05014`) sources `git+https://codeberg.org/Zephulbr/OxiTide.git`.
  `makepkg -o` fails at the clone: `fatal: could not read Username for 'https://codeberg.org'`.
- A scratch copy with `url=https://github.com/Zephulbr/OxiTide` clones the tag and runs `prepare()`
  (`npm ci`, `cargo fetch --locked`) with no error. `build()` was not run; it is the same
  `tauri build --no-bundle` path the gates already cover.
- Not changed in the repo. `packaging/aur/PKGBUILD.in` should point at the public host, but which
  host is canonical is the owner's call (the 2026-10-23 note says origin is Codeberg; this clone's
  `origin` is GitHub).

### Observations (not in the 2026-10-23 list)

- **Tray hide and show restores the wrong size.** The window was resized to 900×620 and closed
  (close-to-tray hides it). A second launch brought it back at 1646×1029 (the work area), although
  `window_maximized = false` was saved. A real process restart restored 900×620. Repro: resize,
  close, re-run the binary, read the window geometry. Cause not verified: maybe KWin re-maximizes
  on show, or the hidden-window restore path. Open.
- The stored `window_width = 3292` from before the fix made the window open maximized, and
  `window_maximized = true` stuck. Clearing it needed a manual un-maximize.

### Still open for the owner `[HUMAN]`

- DAC rows (step 4), including the picker and DAC hotplug.
- Visual check of the volume slider and the "dragging the slider updates `playerctl volume`" half.
- Issue #8 (mix art shows on Linux), issue #10 (empty AT-SPI tree, accept as debt or fix), issue #3
  (empty tooltip, accepted caveat).
- The MPRIS desktop-entry check on an installed package (deb or AUR).
- Feed gray-out, phone pairing from a real phone, a listening check.
- AUR decision above, then `makepkg -si` if it goes ahead.
- Owner call on the tray hide-and-show size (above).

### Side effects on this machine

- The release build is running as the second app instance (pid from `pgrep -x oxitide-app`). It was
  started for these checks and is still up.
- `config.toml` now has `window_width = 900`, `window_height = 592`, `window_maximized = false`.
- `playerctl stop` / `play` changed the restored session position.
