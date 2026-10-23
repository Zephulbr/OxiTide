# Studio (formerly Instrument) — design file

Shared foundation: [../DESIGN.md](../DESIGN.md). Decision record:
`docs/superpowers/specs/2026-09-25-oxitide-2b-visual-mode-instrument-design.md`.
Renamed Studio in the UI (2026-10-23); the mode id stays `instrument` in config,
`data-mode` and code.

## 1. Personality

Quiet confidence, density, precision — the mastering desk, not the light show.
"Nothing moves unless you touched it." Instrument is **the same app with the atmosphere
dial at zero**: same layout, IA and components, effects off, nothing removed. It is the
Track 3 performance baseline and the natural home for reduced-motion users.

## 2. Tokens (deltas over Atmosphere)

| Token | Dark | Light |
|---|---|---|
| `--bg` | `#17120e` (unchanged) | `#f6f1ea` |
| Backdrop | flat room (no artwork) | flat paper |
| `--surface-1/2/3` | solid `#211b15` / `#262019` / `#2b241c` | `#fbf8f3` / `#f2ece3` / `#eae2d7` |
| `--surface-float` | `#2b241c` | `#fffdf9` |
| `--edge-lit` | flat 1px `--line` | same |
| `--glare-alpha` | .10 | .08 |
| `--player-veil` | `--surface-2` (opaque; no gradient needed) | same |
| `--glass-blur`, `--overlay-blur` | 0 | 0 |
| `--accent` | brand `#f7f3ee` warm white (Dynamic colour defaults to Full; shown when it is Off) | brand `#2a211a` warm ink |
| flags | backdrop art, glass blur, grain, disc spin, wave seek, ambient motion: **off** | same |

The Dark column is the **Warm** tone (the default).

### 2.1 Tones (dark theme only)

`ui.instrument_tone` (Settings → Tone, shown in Studio) sets `data-tone` on `<html>`.
`modes.css` swaps the dark recipe for Neutral and Black; the light theme keeps its warm
paper whatever the tone. Layout, flags and everything else stay identical.

| Token | Warm | Neutral | Black (OLED) |
|---|---|---|---|
| `--bg` | `#17120e` | `#121212` | `#000000` |
| `--surface-1/2/3` | `#211b15` / `#262019` / `#2b241c` | `#1b1b1b` / `#202020` / `#262626` | `#0a0a0a` / `#121212` / `#1a1a1a` |
| `--surface-float` | `#2b241c` | `#262626` | `#161616` |
| `--player-veil-top` | `--surface-2` | `--surface-2` | `--surface-1` (the chrome sits a hair above black) |
| `--text` / `--text-muted` | `#f2ece6` / `#a99f94` | `#ededed` / `#a3a3a3` | `#ededed` / `#9e9e9e` |
| `--line` | `rgba(255,240,225,.10)` | `rgba(255,255,255,.10)` | `rgba(255,255,255,.11)` |
| brand accent / on-accent | `#f7f3ee` / `#17120e` | `#f2f2f2` / `#121212` | `#f2f2f2` / `#000000` |

## 3. Typography

UI font defaults to **Inter**; the user's Typeface pick always wins. Display text is the
same face at the shared display weight/tracking (see the root DESIGN.md §2.2).

## 4. Kept deliberately

Display type, disc spinner for loading, radii, icons, focus rings, badges, toasts,
menus, dialogs, layout and breakpoints (including the docked/floating panel styles for the
player bar and queue; the floating cards are opaque here), density, mini player, and the
player bar as a whole (64px cover, "From …" line, quality plate, floor light on hover — it answers the
pointer, so it fits "nothing moves unless you touched it").

## 5. Performance role

Zero `backdrop-filter`, zero animated blur, zero colour tweening. Atmosphere's and Rack's
overheads are judged relative to Instrument.
