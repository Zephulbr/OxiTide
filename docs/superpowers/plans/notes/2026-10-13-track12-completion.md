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

## Owner checklist

- [ ] Back up `~/.tauri/oxitide.key` (losing it strands installed copies).
- [ ] Create the GitHub repo; confirm the slug matches `plugins.updater.endpoints`.
- [ ] Add secret `TAURI_SIGNING_PRIVATE_KEY` (contents of the key file; no password secret).
- [ ] Push; fix whatever the first CI run shows.
- [ ] Push a tag; inspect the draft release; install the NSIS build; publish.
- [ ] Release a second version; confirm Settings → System → Updates finds and installs it
      (Windows), and the AppImage path later with the batched Linux run.
- [ ] Discuss: Windows code signing (SignPath Foundation etc.) and the pre-public review.
