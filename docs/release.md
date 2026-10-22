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
   - Set it with `gh secret set TAURI_SIGNING_PRIVATE_KEY --body "$((Get-Content ~/.tauri/oxitide.key -Raw).Trim())"`
     (PowerShell). **Do not pipe the file into `gh secret set` from Windows PowerShell 5.1**: it
     prepends a UTF-8 byte-order mark and the build fails with "failed to decode base64 secret key:
     Invalid symbol 239".
2. **Repository slug.** The updater endpoint in `tauri.conf.json` assumes
   `github.com/Zephulbr/OxiTide`. Change it if the repository lives elsewhere.
3. **Actions permissions.** Settings → Actions → General: workflows may create releases
   (the `publish` job asks for `contents: write` itself).

## Release steps

1. Bump `version` in the workspace `Cargo.toml` (`[workspace.package]`) — the only place the
   version lives — and refresh `Cargo.lock` (any `cargo build`/`cargo check` does it). Commit, push
   to `main`, and wait for CI to pass.
   - `tauri.conf.json` has no `version` on purpose: Tauri then uses `Cargo.toml`'s, and the Rust
     code (updater, lyrics user agent) reads the same value through `CARGO_PKG_VERSION`.
     `ui/package.json` has none either. The release check fails if either file gains one.
2. `git tag vX.Y.Z && git push origin vX.Y.Z`.
3. The workflow checks the tag against `Cargo.toml`, builds Windows (NSIS, MSI) and Linux
   (deb, AppImage) bundles, signs the updater artifacts, and creates a **draft** release containing
   the bundles, `latest.json`, `SHA256SUMS.txt`, and the AUR `PKGBUILD` (rendered from
   `packaging/aur/PKGBUILD.in` by `make-pkgbuild.mjs`, building the tagged source).
4. Smoke the draft's installer on a clean machine, edit the generated notes, then **publish**.
   Publishing is what makes `…/releases/latest/download/latest.json` resolve — installed copies
   only see the update after this step.

## Notes

- Windows installers are not Authenticode-signed (SmartScreen warns: *More info → Run anyway*);
  verify downloads against `SHA256SUMS.txt`. Updates are still verified against the minisign key.
- The updater runs for Windows installs and for the Linux AppImage. deb/AUR installs update through
  the package manager.
- The AUR recipe is a template (`pkgver=@PKGVER@`, `_tag=@TAG@`); never fill it in by hand. The
  rendered `PKGBUILD` fetches `#tag=vX.Y.Z` from the Codeberg `url`, so the tag must be on Codeberg
  (step 2 pushes it there). A prerelease tag drops its hyphens (`v1.0.0-rc.1` → `pkgver=1.0.0rc.1`)
  so it sorts before the release. The package is not on the AUR yet; to publish, copy the release's
  `PKGBUILD` into the AUR repo and run `makepkg --printsrcinfo > .SRCINFO`.
- A copy only updates to releases published after it contained the updater, so the first
  updater-enabled version must be installed by hand.
- Local unsigned build (no key needed): `npm --prefix ui run app:build`. Local signed build:
  set `TAURI_SIGNING_PRIVATE_KEY` to the key path and add
  `-- --config crates/oxitide-app/tauri.release.conf.json`.
- To test the pipeline without a real release, push a tag like `v0.0.0-test` on a throwaway branch
  after temporarily setting the `Cargo.toml` version to match, or run the pieces locally:
  `node --test scripts/release/release.test.mjs` and
  `node scripts/release/make-manifest.mjs <bundleDir> <outDir> vX.Y.Z owner/repo` and
  `node scripts/release/make-pkgbuild.mjs vX.Y.Z <outDir>`.
