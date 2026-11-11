# Smoke helpers

Small tools for the manual smoke checklists in `docs/superpowers/plans/notes/`. None of them
run in CI. They used to live only in the gitignored `.superpowers/`, where nothing would have
kept them.

| Script | What it does | Used by |
|---|---|---|
| `net-toggle.ps1` | Run **elevated** and leave it open. It watches `%TEMP%\oxitide-net.cmd` and applies one action per line: `down`/`up` (adapter), `block`/`unblock` (an outbound firewall rule for the app exe only), `status`. A non-elevated shell or agent drives it with `Set-Content $env:TEMP\oxitide-net.cmd down`. | Network resilience rows (`2026-10-14-network-resilience-smoke.md`, run record `2026-10-24-engine-smokes.md`) |
| `watch-cache.ps1 -Mode new\|spill` | Prints cache files as they appear: new `.fmp4` + `.meta` sidecars (download ahead), or `spill-*.part` scratch files appearing and going (seek). | Same |
| `watch-log.ps1` | Tails the newest app log for network/underrun/error lines, with UTC stamps shown in local time. | Same |
| `glide-probe.mjs <port> <selector> [maxEls]` | Over the WebView2 CDP endpoint: records each matched element's rect every frame while the queue dock opens and closes, and reports how far each frame strays from the glide curve. | Queue dock glide checks (`docs/agents/ui.md`) |
| `glide-steps.mjs <port>` | Lists elements whose height jumps by more than 8 px between two frames of the glide. | Same |

The CDP probes need a build with remote debugging on, as the perf harness runs it
(`scripts/perf/run-scenario.ps1` sets `WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS`).
