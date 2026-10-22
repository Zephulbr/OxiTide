# Instrument — design file

Shared foundation: [../DESIGN.md](../DESIGN.md). Decision record:
`docs/superpowers/specs/2026-09-25-oxitide-2b-visual-mode-instrument-design.md`.

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
| `--accent` | brand `#2ee2c4` (Dynamic colour `Auto` = Off; user may enable) | brand `#0e7a67` |
| flags | backdrop art, glass blur, grain, disc spin, wave seek, ambient motion: **off** | same |

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
