# OxiTide — Track 2b Visual Identity: Instrument Mode (Delta)

- **Status:** Draft for review
- **Date:** 2026-09-25
- **Owner:** V
- **Parent:** `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` (shared foundation — this document only defines deltas from it)
- **Sibling:** `…-2b-visual-mode-hifi-rack-design.md` — **deferred**; Hi-Fi Rack gets its own dedicated design session, sequenced last (owner decision, 2026-09-25: it needs custom elements and a different approach, not a delta).

---

## 1. Purpose & personality

Instrument is the same app with the **atmosphere dial at zero**: static, minimal-motion,
performance-first. It exists for four audiences at once:

1. Users who don't want background movement or visual ceremony.
2. Performance-conscious users / weaker hardware — nothing here taxes the GPU.
3. Reduced-motion users (the mode reduced-motion behavior approximates).
4. **Track 3:** Instrument is the performance *baseline*; the harness measures it as the
   floor and Atmosphere's budget against it.

Personality: quiet confidence, density, precision — the mastering deck, not the light
show. "Nothing moves unless you touched it."

**Non-negotiable:** Instrument is *not* a lesser build. Same layout, same information
architecture, same components, same identity thread — effects off, nothing removed.

## 2. Mechanism

Per foundation §4: Instrument is a `data-mode="instrument"` token + flag override.
Components never branch on mode. If a component needs an `if instrument` during
implementation, the foundation's token/flag surface is incomplete — fix the foundation,
don't branch.

## 3. Token deltas (Warm Graphite, flattened)

| Token | Atmosphere | Instrument |
|---|---|---|
| `--bg` | `#17120e` | `#17120e` (unchanged) |
| Backdrop | cover art, blurred, crossfading | none — flat room (an ultra-subtle static warm gradient is permitted, fixed, never artwork-derived) |
| `--surface-1/2/3` | translucent white-warm alphas over backdrop | **solid:** `#211b15` / `#262019` / `#2b241c` |
| `--edge-lit` | top-left-lit gradient border | flat `1px` `--line`; no gradient, no grain |
| `--accent` | dynamic (default) | static brand `#2ee2c4` (dynamic color defaults Off; user may re-enable — accent recolor works fine on flat surfaces) |
| text / muted / semantic hues | — | unchanged |

## 4. Feature-flag deltas (foundation §4.3)

| Flag | Instrument value | Visible effect |
|---|---|---|
| `--flag-backdrop-art` | off | flat room |
| `--flag-glass-blur` | off | solid panels, zero `backdrop-filter` in the app |
| `--flag-grain` | off | — |
| `--flag-disc-spin` | off | now-playing shows the **Sleeve** artwork |
| `--flag-dynamic-color` | default `off` (user-adjustable) | static brand accent |
| `--flag-ambient-motion` | off | `--dur-mood` → 0 (instant recolors); `--dur-view` → 120ms fades; hover/press states stay (120ms, functional) |
| `--flag-wave-seek` | off | flat hairline seek — the waveform reads as decoration when nothing else moves |

Kept deliberately:

- **Fraunces display face** — the identity thread belongs to the *system*, not the mode
  (owner-confirmed 2026-09-25).
- **Disc spinner** for loading — functional feedback, zero ambient cost.
- Radii, icons, focus rings, badges, toasts, menus, dialogs — unchanged.

## 5. Typography delta

UI font default is **Inter** (bundled) — the disciplined, dense face suits the mode.
The user's Typeface setting always wins (foundation §6); switching modes with no
explicit user pick swaps Manrope ↔ Inter. Fraunces is not affected (§4).

## 6. Explicit non-differences

Layout & IA (shell, sidebar rail, resizable/hideable queue, expanded now-playing layout,
density setting, breakpoints, mini player), Settings surface, cover thumbnails in track
rows, all accessibility floors (contrast, focus ring, targets), and all motion *tokens*
(they're simply re-valued). `prefers-reduced-motion` hard floor (foundation §12)
applies in every mode — in Instrument it's already satisfied by construction.

## 7. Performance role

- Zero `backdrop-filter`, zero animated blur, zero crossfade tweening, static surfaces:
  Instrument is expected to hold frame budget on any hardware the app runs on.
- The Track 3 harness uses Instrument as the baseline profile; Atmosphere's accepted
  overhead is defined *relative to Instrument* (values fixed when the harness lands).

## 8. Settings interplay

- Mode switch live-applies; appearance settings are stored per user, not per mode —
  except Typeface and Dynamic color, which keep per-mode *defaults* (Inter / Off here)
  that an explicit user pick overrides.
- Documented in Settings: "Instrument minimizes animation and visual effects."

## 9. Open items for the implementation plan

None beyond foundation §15. Instrument requires no new systems (it's the foundation
with flags off) and is the natural first build target: land tokens + components in
Instrument, then light up Atmosphere's dynamic systems behind flags.
