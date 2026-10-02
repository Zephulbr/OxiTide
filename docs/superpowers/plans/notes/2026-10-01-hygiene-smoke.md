# Hygiene batch smoke checklist (2026-10-01)

Human checks for the LICENSE/notices/CSP/docs batch. The CSP is new and can
break resource loading that unit tests cannot see.

## CSP (dev)

- [ ] `npm --prefix ui run app:dev` — window loads without a blank screen.
- [ ] DevTools console has no CSP violations on startup.
- [ ] Vite HMR works (edit a Svelte file, see the update apply).
- [ ] A remote cover loads (album/playlist art from `resources.tidal.com`).
- [ ] Vendored fonts render (Atmosphere typeface; no fallback flash).
- [ ] Search + lyrics + queue interactions work (IPC unaffected).

## CSP (packaged)

- [ ] `npm --prefix ui run app:build`, install/run the NSIS build.
- [ ] Previous checks repeat with no console CSP errors.
- [ ] Login round-trip: browser opens, paste-back completes, deep link still
      handled.

## Legal/docs

- [ ] `LICENSE` is the AGPL-3.0 text; `THIRD_PARTY_NOTICES.md` includes the font
      OFL text, Oneko pointer, and dependency inventory.
- [ ] README and AGENTS.md describe the current state (Windows today, Linux
      roadmapped; Tracks 8–10 in the roadmap).
