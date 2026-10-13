# Cutting a release

Track 12 (spec `superpowers/specs/2026-10-13-oxitide-track12-distribution-design.md`). Releases are
built by `.github/workflows/release.yml` from a version tag; nothing is uploaded from a dev machine.

## One-time setup

1. **Update-signing key.** Already generated for this repo with
   `npm --prefix ui exec -- tauri signer generate -w ~/.tauri/oxitide.key -p ""`.
   The public half is committed in `crates/oxitide-app/tauri.conf.json` (`plugins.updater.pubkey`).
   - **Back up `~/.tauri/oxitide.key` somewhere safe (password manager).** If it is lost, installed
     copies can never update again and users must reinstall by hand.
   - The key has an empty password. Add only the secret `TAURI_SIGNING_PRIVATE_KEY` (the file's
     contents); GitHub does not allow empty secrets, so leave `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
     unset — the workflow passes it through as empty.
2. **Repository slug.** The updater endpoint in `tauri.conf.json` assumes
   `github.com/Zephulbr/OxiTide`. Change it if the repository lives elsewhere.
3. **Actions permissions.** Settings → Actions → General: workflows may create releases
   (the `publish` job asks for `contents: write` itself).

## Release steps

1. Bump the version in **both** `crates/oxitide-app/tauri.conf.json` and the workspace `Cargo.toml`
   (`[workspace.package]`), commit, and push to `main`; wait for CI to pass.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The workflow checks the tag against both versions, builds Windows (NSIS, MSI) and Linux
   (deb, AppImage) bundles, signs the updater artifacts, and creates a **draft** release containing
   the bundles, `latest.json`, and `SHA256SUMS.txt`.
4. Smoke the draft's installer on a clean machine, edit the generated notes, then **publish**.
   Publishing is what makes `…/releases/latest/download/latest.json` resolve — installed copies
   only see the update after this step.

## Notes

- Windows installers are not Authenticode-signed (SmartScreen warns: *More info → Run anyway*);
  verify downloads against `SHA256SUMS.txt`. Updates are still verified against the minisign key.
- The updater runs for Windows installs and for the Linux AppImage. deb/AUR installs update through
  the package manager.
- A copy only updates to releases published after it contained the updater, so the first
  updater-enabled version must be installed by hand.
- Local unsigned build (no key needed): `npm --prefix ui run app:build`. Local signed build:
  set `TAURI_SIGNING_PRIVATE_KEY` to the key path and add
  `-- --config crates/oxitide-app/tauri.release.conf.json`.
- To test the pipeline without a real release, push a tag like `v0.0.0-test` on a throwaway branch
  after temporarily matching the versions, or run the pieces locally:
  `node --test scripts/release/release.test.mjs` and
  `node scripts/release/make-manifest.mjs <bundleDir> <outDir> vX.Y.Z owner/repo`.
