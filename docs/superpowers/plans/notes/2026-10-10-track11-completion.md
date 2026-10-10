# Track 11 (Phone Remote) — completion status (2026-10-10)

Spec: `specs/2026-10-09-oxitide-track11-phone-remote-design.md`. Plan record:
`plans/2026-10-09-track11-phone-remote.md`. Branch: `track11-phone-remote` (not merged).

**Status: implementation complete; closure waits on the real-phone smoke** (§7 checklist,
`notes/2026-10-09-track11-phone-remote-smoke.md`), which needs the owner, a phone, and a
real account. Everything scriptable is verified below.

## Verified

| Gate | Result |
|---|---|
| `cargo test --workspace` (Windows) | pass — 58 remote tests incl. HTTP integration on an ephemeral port |
| `cargo clippy --workspace -- -D warnings` (Windows) | clean |
| `cargo test --workspace` + clippy (WSL2 Ubuntu 22.04, `/root/oxitide-8.3` at `cfa1233`) | pass / clean |
| `npm --prefix ui run check` / `npm --prefix ui test` | 0 errors / 92 files, 750 tests |
| `npm --prefix ui run app:build` | NSIS + MSI built; remote chunks contain no Tauri code |
| Release exe, LAN probe (`192.168.1.189:47820`, test device seeded in `remote.json`) | listening on `[::]`; `/remote` 200 with the page CSP; hashed JS/CSS 200; `/index.html`, `/assets/nope.js`, `/whatever` 404 (resolver fallback blocked); RPC without cookie 401; foreign `Host` 403; RPC with cookie returned the live session; SSE delivered `snapshot` |
| Phone page UI | checked at 390×844, dark + light, against a mock backend (Now Playing, windowed Queue, Home shelves, Search + row sheet) |

## PM-D1 (perf corpus, 2026-10-10)

Debug exe at `cfa1233`, Vite dev server, S1/S2/S3-miss/S3-hit/S5-instrument/S5-atmosphere/S6/S7
× 5. **Enabled** = `[remote] enabled = true` plus a simulated phone (an SSE client with a
seeded device cookie) that connected to every launch (40/40: `snapshot`, `session`, `state`
deltas). **Control** = the same build and machine immediately after, remote disabled.
Corpora: `.superpowers/perf/runs-track11/` and `runs-track11-off/` (gitignored).

| Scenario | Metric | Budget | Enabled + phone | Control (off) | Verdict |
|---|---|---|---|---|---|
| S1 | first_paint_ms | ≤ 1500 | 3000 (run 1) / 1106 (runs 2–5 max) | 1136 | no regression — run 1 included Vite's cold start (`starting Vite on 1420`); runs 2–5 = 1079–1106 vs control 1063–1136 |
| S1 | ui_ready_ms | ≤ 2500 | 3483 (run 1) / 1566 (runs 2–5 max) | 1722 | same as above |
| S1 | memory_max_mb | ≤ 570 | 620 | 623 | **pre-existing breach** |
| S2 | first_paint_ms | ≤ 1000 | 1130 | 1105 | **pre-existing breach** |
| S2 | memory_max_mb | ≤ 560 | 616 | 623 | **pre-existing breach** |
| S3-miss | ttfs_ms | ≤ 2500 | 349 | — (one control run timed out waiting for login) | pass |
| S3-hit | ttfs_ms | ≤ 300 | 255 | 252 | pass |
| S3-hit | cache_hits | ≥ 1 | 0 | 0 | **pre-existing breach** |
| S5-instrument | memory_max_mb | ≤ 690 | 693 | 697 | **pre-existing breach** |
| S5-atmosphere | memory_max_mb | ≤ 675 | 699 | 690 | **pre-existing breach** |
| S6 | frame_p99_ms | ≤ 33 | 55.6 | 48.4 | **pre-existing breach** |
| S6 | memory_max_mb | ≤ 740 | 727 | 725 | pass |
| S7 | queue_build_ms | ≤ 250 | 121.9 | 155.3 | pass |
| S7 | memory_max_mb | ≤ 670 | 653 | 654 | pass |

All other rows pass in both corpora. **Conclusion:** the remote — enabled, with a phone
connected — shows no measurable cost against the control. Seven budgets breach **with the
remote disabled too**: memory ceilings (S1/S2/S5), S2 first paint, S3-hit cache hits, and
S6 instrument frame p99. These predate Track 11 (budgets were set 2026-10-02; Tracks 7, 9.x
and 9.5 landed since) and need their own triage; the S3-hit `cache_hits = 0` looks like a
harness/cache-state issue rather than a regression and should be checked first.

The disabled case adds one engine subscriber (a mutex read + `None` check per event) and
nothing on the startup path; the multi-page build's shared chunk does not apply to these
dev-server runs and should be re-checked once on a release build (S1/S2 by hand).

## Open (owner)

- [ ] Real-phone smoke (`notes/2026-10-09-track11-phone-remote-smoke.md`), including the
      Windows firewall prompt, approval flow, revoke, sleep/wake, and cellular isolation.
- [ ] Decide on the pre-existing PM-D1 breaches above (triage separately or renegotiate).
- [ ] Review the spec refinements in the plan record, then merge `track11-phone-remote`.
