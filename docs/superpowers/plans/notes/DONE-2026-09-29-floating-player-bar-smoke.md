# Floating player bar — manual smoke checklist

GUI smoke checks for the full-bleed floating player bar (2026-09-29). The bar
is now out of the grid flow and overlays the columns, so lists scroll behind
its glass. Requires a human, a real account, and a browser login. Run with
`npm --prefix ui run app:dev`.

## Content behind the glass

- [ ] Track list (`Album` / `Playlist` / `Tracks`): rows scroll **under** the
      bar and visibly blur through the glass — no hard band where content
      stops above the bar.
- [ ] The last track row can scroll fully clear of the bar (bottom clearance in
      `.table-scroll.fill`); nothing is permanently hidden under it.
- [ ] Search / Explore / Home (`.main` scroller): the last section clears the
      bar when scrolled to the bottom.
- [ ] Queue rail: queue items scroll behind the bar and the last item clears it
      (bottom clearance in `VirtualList`).
- [ ] Sidebar: nav + playlist tree scroll behind the bar; the last row clears
      it (bottom clearance in `.scroll`).
- [ ] Glass recipe unchanged: `--surface-2` tint + blur, no top border, no
      rounded corners, no drop shadow, no change to bar height (80px).
- [ ] Hovering the seek wave still reads correctly over content scrolling
      behind it (wave cap covers raw content up to the bar edge).

## Seek wave cap

- [ ] Hover/drag the seek over a **bright** cover: the strip between the wave
      and the bar matches the bar (same tint + blur) — no darker band.
- [ ] No rectangular artifact and no visible seam where the cap meets the bar
      glass (Chromium `clip-path` + `backdrop-filter` blends cleanly).
- [ ] Cap tracks scrolled content: rows sliding behind the bar blur the same
      through the cap as through the bar.
- [ ] Instrument mode: cap is an opaque strip matching the opaque bar; no
      blur/transparency artifacts.
- [ ] At rest the cap is collapsed (straight progress line only).
- [ ] Hover reveal feels snappy: the flat line fades out and the wave + cap
      grow in together (`--dur-reveal`, 80ms) with no lingering tail. If it
      still trails, swap the cap/wave easing to `cubic-bezier(0.2, 0, 0, 1)`.

## Sidebar header popover

- [ ] Expanded sidebar: gear icon sits in the header next to the collapse
      toggle; clicking it opens the glass menu with **Settings** and **Log out**.
- [ ] Settings navigates to the Settings view; Log out signs out.
- [ ] Log out renders in the danger color.
- [ ] Menu closes on outside click, on Escape, and on scroll.
- [ ] Rail (collapsed) mode: the gear appears as a centered icon below the
      header; the wordmark<->collapse hover swap still works; the popover
      opens in the same place.
- [ ] No pinned Settings/Log out footer remains at the sidebar bottom.

## Layering / overlays

- [ ] Docked queue resize handle is still grabbable along the queue's left edge
      (including near the bar).
- [ ] Floating queue overlay (`< ~900px` window) sits **above** the bar; its
      bottom stops at the bar's top edge.
- [ ] Playlist picker / dialogs / context menus / toasts all render above the
      bar.
- [ ] Now Playing: content sits above the bar (does not run under it); closing
      it dissolves in place without a layout jump.

## Modes / reduced motion

- [ ] Atmosphere: content clearly blurs through the bar glass.
- [ ] Instrument: no blur — the bar is an opaque veil over the content behind
      (still full-bleed, still no visible content through it).
- [ ] Reduced motion on: no new animation added by this change (layout only).
