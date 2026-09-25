# OxiTide — Track 2b Visual Identity: Convergence (What Ships)

- **Status:** Draft for review
- **Date:** 2026-09-25
- **Owner:** V
- **Parents:** `2026-09-25-oxitide-2b-visual-foundation-atmosphere-design.md` ·
  `2026-09-25-oxitide-2b-visual-mode-instrument-design.md` ·
  `2026-09-24-oxitide-post-mvp-roadmap.md` (Track 2b)

---

## 1. What ships in Track 2b

**One design system, two modes at launch:**

| Mode | Role | Spec |
|---|---|---|
| **Atmosphere** | Default | Foundation doc (tokens, color, type, layout, components, motion) + mode definition |
| **Instrument** | Static / performance / reduced-motion mode | Delta doc (foundation with effects off) |

Mode is a `data-mode` attribute swapping semantic tokens + feature flags; components
never branch on mode (Instrument doc §2). Switching lives in Settings → Appearance,
live-applies, persisted. The `prefers-reduced-motion` hard floor applies in every mode
(foundation §12).

## 2. Hi-Fi Rack — deferred, sequenced last

Owner decision (2026-09-25): Rack is **not a delta** — it needs custom elements and its
own approach, so it gets a **dedicated design session and its own spec**, after the two
shipping modes land. The Settings mode switch ships showing Atmosphere/Instrument; Rack
arrives as a later addition without redesigning anything (flag/token architecture was
built for it).

Assets already parked for that session:

- **LED-readout badge style** (its star feature, deferred from the Atmosphere polish pass)
- Warm Graphite base was chosen partly as the base Rack wants (fewer overrides later)
- The disc/groove/sheen CSS groundwork in Atmosphere's now-playing

## 3. Build order (guidance for the implementation plan)

1. **Land the foundation in Instrument first** — tokens, flags, components, layout/IA.
   Instrument needs zero new systems; it de-risks everything after.
2. **Light up Atmosphere behind flags** — backdrop pipeline, cover-color extraction +
   §5.3 contrast chain, glass/grain/lit edges, disc spin, crossfades.
3. Mini player: approved in principle; sequencing (in-2b vs immediate follow-up slice)
   is a plan-time decision (foundation §15).

## 4. Explicitly not in 2b

Hi-Fi Rack (own session) · light mode (token-ready only, foundation §4.4) · true
waveform peak data (stylized wave ships) · synced lyrics (Track 4; slot reserved in
now-playing) · user-uploaded font files.

## 5. Doc ownership map

| Question | Answer lives in |
|---|---|
| Tokens, palette, dynamic color, contrast chain | Foundation §4–§5 |
| Typography, fonts, Settings surface | Foundation §6, §12 |
| Layout, shell, now-playing, density, mini player | Foundation §7 |
| Components, motion, signature treatments, a11y | Foundation §8–§11 |
| What Instrument changes/keeps; perf baseline role | Instrument doc (whole) |
| Rack's future | This doc §2 → its own future spec |
