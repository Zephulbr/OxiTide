# Plan 5 Release Checklist — OxiTide 0.2.0

> Human checklist for the first shareable installer. Record results inline:
> `[x]` pass, `[ ]` fail/not run, with a one-line note after a failure.

## Version bump (both files in one commit)
- [x] `Cargo.toml` → `[workspace.package] version = "0.2.0"`
- [x] `crates/oxitide-app/tauri.conf.json` → `"version": "0.2.0"`
- [x] `cargo test --workspace` regenerates `Cargo.lock`; commit it with both files.

## Build
```powershell
npm --prefix ui run app:build
```
Artifacts (unsigned):
- NSIS: `target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe` (per-user, no admin prompt)
- MSI: `target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi`

The WebView2 bootstrapper is downloaded at install time, so the installer needs
internet on a machine without WebView2 (Windows 10/11 normally ships it).

## Install / launch / upgrade / uninstall
- [x] Run the NSIS setup as a normal user: no UAC/admin prompt; installs under `%LOCALAPPDATA%\OxiTide`.
- [x] Launch from the Start Menu: the window opens at the saved size/position and the app works.
- [x] SmartScreen warns "Windows protected your PC" (unsigned): More info → Run anyway is expected. actually didnt get flagged by smart screen for me (could be my settings) either way not a problem.
- [x] Sign in and play a track; note the queue and position.
- [x] Run the same setup again over the installed version: upgrade in place succeeds.
- [x] `%APPDATA%\OxiTide\config.toml`, the cache directory, and the keyring entry survive the upgrade.
- [x] Uninstall from Settings → Apps: the install directory is removed; `%APPDATA%\OxiTide` (config, cache, logs) is kept.
- [x] The MSI target still builds (`target/release/bundle/msi/...`) even though NSIS is primary.

## Records
- Build commit: dd6622e
- Build date: 2026-09-24
- Notes:
