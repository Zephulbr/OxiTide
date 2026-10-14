# Track 12 — distribution: implementation notes and owner checklist

Spec: `../../specs/2026-10-13-oxitide-track12-distribution-design.md`. Runbook: `docs/release.md`.

## Verified locally (2026-10-13)

- `cargo test -p oxitide-app` 417 passed (incl. the RPC allowlist test classifying the four new
  commands as `NOT_EXPOSED`); `cargo clippy --workspace -- -D warnings` clean.
- `npm --prefix ui run check` 0 errors; `npm --prefix ui test` 774 passed (5 new UpdatesSection tests).
- `node --test scripts/release/release.test.mjs` 7 passed; `check-version.mjs v0.2.0` ok.
- Signed release-overlay build on Windows (throwaway-free: the real key, empty password) produced
  NSIS + MSI + `.sig`; `make-manifest.mjs` turned them into a valid `latest.json` and
  `SHA256SUMS.txt`.

## Not verified (cannot be, locally)

- Neither workflow has run on GitHub. Expect first-run fixes (hosted-runner audio/keyring in
  `cargo test`, apt packages, AppImage build on 22.04).
- The Linux AppImage updater path and the in-app install/restart flow.
- An end-to-end update (needs two published releases).

## Run on GitHub (2026-10-14, `Zephulbr/OxiTide`)

- CI green on Windows, Ubuntu 22.04 and the release-scripts job. Findings: Node 20 fails the UI
  tests (jsdom 30 needs Node >=22.22 / 24.15; fixed, CI uses 24). One Windows run failed
  `oxitide-audio engine::tests::retry_resumes_at_the_recorded_position` and passed on re-run
  (flaky: after `retry()` it asserts `playing` right after the first new samples; unfixed).
- Tag `v0.2.0` built NSIS + MSI + deb + AppImage, signed the updater artifacts, and created a
  **draft** release with `latest.json` (5 platform keys) and `SHA256SUMS.txt`. The first attempt
  failed because the secret was set via PowerShell 5.1 piping, which added a BOM (see
  `docs/release.md`).
- Merged the repo's GitHub "Initial commit" into history with `-s ours` (no force push).

## Owner checklist

- [x] Back up `~/.tauri/oxitide.key` (losing it strands installed copies).
- [x] Create the GitHub repo; confirm the slug matches `plugins.updater.endpoints`.
- [x] Add secret `TAURI_SIGNING_PRIVATE_KEY` (contents of the key file; no password secret).
- [x] Push; fix whatever the first CI run shows.
- [x] Push a tag; inspect the draft release.
- [ ] Install the draft's NSIS build on a clean profile; publish the draft.
- [ ] Release a second version; confirm Settings → System → Updates finds and installs it
      (Windows), and the AppImage path later with the batched Linux run.
- [ ] Discuss: Windows code signing (SignPath Foundation etc.) and the pre-public review.
