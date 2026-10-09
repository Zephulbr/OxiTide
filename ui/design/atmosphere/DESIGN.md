# Atmosphere — design file

Shared foundation: [../DESIGN.md](../DESIGN.md). Decision record:
`docs/superpowers/specs/2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md`
(+ convergence `2026-09-25-oxitide-2b-visual-convergence.md`, light theme
`2026-10-05-track9-ui-completion-design.md` §6.1).

## 1. Personality

*Immersive, warm-dark, unhurried, premium.* A listening room at night — the record
sleeve is the lamp.

1. **Music is the light source.** Artwork illuminates the UI; chrome never competes with
   it for saturation or brightness.
2. **Glass, not walls.** Translucent layers over the artwork; depth from blur and light.
3. **Motion answers the music.** Ambient motion only while playing; pause stills the room.
4. **Chrome is quiet until needed.**

**Identity budget:** exactly four signature places — the Fraunces display face, lit edges
+ film grain, the tide motif (waveform seek + disc spinner), artwork reactivity. Do not
add a fifth.

## 2. Tokens (deltas over the foundation)

| Token | Dark | Light |
|---|---|---|
| `--bg` | `#17120e` (Warm Graphite) | `#f6f1ea` (warm paper) |
| `--surface-1/2/3` | warm white alphas .05/.07/.09 | warm ink alphas .045/.065/.085 |
| `--surface-float` | `rgba(23,18,14,.82)` | `rgba(250,246,240,.94)` |
| `--glass-blur` / `--overlay-blur` | 18px / 8px | same |
| `--accent` | `--dyn-primary` (cover) | `--dyn-primary` (light-room extraction) |
| flags | backdrop art, glass blur, grain, disc spin, wave seek, dynamic colour, ambient motion: **on** | same |

Dynamic colour: three roles (Primary, Secondary, Tertiary) + backdrop wash, guarded by a
contrast chain (structural scrim minimums + per-cover composite-ceiling clamping).
`Auto` = Full here.

## 3. Layout

- Shell "Refined Classic": sidebar 248px ↔ 64px rail (auto below 1080px), main, queue
  rail 260–420px (overlay below 760px), floating 80px glass player bar with the seek as
  its top edge.
- Now Playing "Editorial Split": artwork left (Disc default / Sleeve), metadata column
  right (eyebrow, Fraunces title, italic byline, badges, tabs Lyrics / Next / Info).
- Track rows 46px (Comfortable) / 38px (Compact); cover thumbs in mixed lists; playing
  row = 18% accent tint + accent title; no left bars, no animated equalizers.

## 4. Components

Radii: controls 8 · cards 12 · sheets 16 · play button round. Hover = surface brighten;
press = scale .98; disabled 40%. Buttons: primary accent fill, ghost glass, danger red
ghost. Badges: bordered chips (MAX Tertiary-outlined). Toasts: glass + severity glyph,
slide-in with slight overshoot (the only micro-interaction). Loading: disc spinner.
Scrollbar: 8px warm translucent thumb.

## 5. Motion

Backdrop + dynamic-colour crossfade 1200 ms; disc spin ~12 s/rev, eases to stop in
~600 ms on pause; everything else per foundation §2.3.
