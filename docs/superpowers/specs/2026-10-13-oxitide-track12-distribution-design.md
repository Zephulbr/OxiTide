# Track 12 — Distribution: CI, release pipeline, auto-update (design)

- **Date:** 2026-10-13
- **Owner:** V
- **Parents:** `2026-09-24-oxitide-post-mvp-roadmap.md` (§6 "Packaging: code signing, auto-update
  (deferred; required only if shared beyond friends); CI (none today)")
- **Process note:** the owner called this on 2026-10-13 after deciding the project may be shared as
  a public GitHub project. Windows code signing and the "before going public" review are
  deliberately **not** part of this spec; they are discussed separately (T12-D8).

## 1. Goal

Make the repository safe to share and cheap to ship: every push is gated by the same checks the
owner runs by hand, a version tag produces a verifiable release, and installed copies can update
themselves without a paid code-signing certificate.

## 2. Scope

**In:**

1. **CI** — GitHub Actions workflow running the existing Rust and UI gates on Windows and Linux.
2. **Release pipeline** — a tag-triggered workflow that builds the Windows (NSIS, MSI) and Linux
   (deb, AppImage) bundles, signs the updater artifacts, writes `latest.json` and `SHA256SUMS.txt`,
   and creates a **draft** GitHub Release.
3. **Auto-update** — `tauri-plugin-updater`, driven from Rust commands: a startup check
   (opt-out), a manual "Check for updates", and a consented download-and-install.

**Out:** Windows code signing (T12-D8), macOS, Forgejo/Codeberg CI, delta updates, release
channels (beta/nightly), silent installs without user action, the AUR package (stays manual),
perf-harness and smoke checks in CI (they need a real account, audio hardware, or a CDP-attached
WebView2), and any change to playback or UI beyond the Updates section.

## 3. Decisions

- **T12-D1 — host and slug.** Workflows live in `.github/workflows/`. `origin` is currently on
  Codeberg; the GitHub repository is assumed to be `Zephulbr/OxiTide`. That slug appears in exactly
  one place in code (`plugins.updater.endpoints` in `tauri.conf.json`) plus the README badge text;
  change it there if the repository lands elsewhere. Nothing in this track pushes to any remote.
- **T12-D2 — CI gates mirror `AGENTS.md`.** Per OS (`windows-latest`, `ubuntu-22.04`): UI build,
  `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`,
  `npm --prefix ui test`. The UI build runs **first** because `tauri::generate_context!` needs
  `ui/dist` at compile time. `rustfmt` is not gated (the repo has never enforced it). Ubuntu 22.04
  matches the pinned Linux base in `docs/linux-build.md`. Jobs cancel superseded runs and use
  `Swatinem/rust-cache`. Tests never reach the live TIDAL API or real credentials (existing rule),
  so CI needs no secrets.
- **T12-D3 — release flow.** Pushing a tag `vX.Y.Z` runs `release.yml`:
  1. `check-version` — tag must equal `crates/oxitide-app/tauri.conf.json` `version` and the
     workspace `Cargo.toml` `version` (`scripts/release/check-version.mjs`).
  2. `build` matrix (Windows, Ubuntu 22.04) — `npm --prefix ui run app:build -- --config
     crates/oxitide-app/tauri.release.conf.json`, then uploads the bundles and their `.sig` files
     as workflow artifacts.
  3. `publish` — `scripts/release/make-manifest.mjs` writes `latest.json` and `SHA256SUMS.txt`
     from the artifacts; `gh release create --draft` attaches everything. The owner reviews and
     publishes the draft by hand. Publishing is what makes
     `releases/latest/download/latest.json` resolve, so a bad build never reaches installed copies.
- **T12-D4 — updater artifacts only in a release overlay.** `bundle.createUpdaterArtifacts` lives in
  `tauri.release.conf.json`, not the base config, so a local `npm --prefix ui run app:build` keeps
  working without the signing key.
- **T12-D5 — update signing key (free, not a code-signing certificate).** `tauri signer generate`
  makes a minisign keypair. The public key is committed in `tauri.conf.json`; the private key and
  its password are the GitHub secrets `TAURI_SIGNING_PRIVATE_KEY` and
  `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`. It lets installed copies verify that an update came from
  the owner regardless of Windows Authenticode. **Losing the private key strands existing
  installs** (they could only be replaced by a manual reinstall), so the completion note makes
  backing it up an explicit owner step. Key rotation is out of scope.
- **T12-D6 — updater lives in Rust.** `crates/oxitide-app/src/updater.rs` owns the plugin calls;
  the webview gets four ordinary commands (`check_for_update`, `install_update`,
  `get_update_support`, `set_update_check_on_start`) and one event (`update_available`). The JS
  updater plugin and its capability permissions are not added, so the CSP and the capability file
  are unchanged and the webview cannot be pointed at another endpoint. The four commands are
  classified `NOT_EXPOSED` for the phone remote (an update must be approved at the desktop).
- **T12-D7 — update behaviour.**
  - **Config:** `[updates] check_on_start = true` (opt-out, shown in Settings → System).
  - **Startup check:** ~10 s after launch, in the background, only when enabled, supported, and not a
    debug build. Failures are logged and silent. A found update emits `update_available`; the UI
    shows an info toast with a "View" action to Settings. Nothing downloads without a click.
  - **Install:** user presses "Download and install"; progress events drive a progress bar. Before
    the installer takes over, the same shutdown work as `RunEvent::Exit` runs (session flush,
    history finalize, remote stop, scheme restore), because the Windows updater exits the process
    directly and `Exit` would not fire. NSIS runs `passive`; Linux restarts the app.
  - **Support matrix:** Windows NSIS and MSI install; Linux only when running as an AppImage
    (`APPIMAGE` env present). deb/AUR installs report "managed by your package manager" and the
    buttons are disabled. A mismatch (Windows MSI installed per-machine without admin) surfaces the
    installer's error; it is not special-cased.
  - **Platform keys in `latest.json`:** `windows-x86_64` (NSIS), `windows-x86_64-nsis`,
    `windows-x86_64-msi`, `linux-x86_64`, `linux-x86_64-appimage`.
- **T12-D8 — Windows code signing is deferred.** Installers stay unsigned (SmartScreen warns on
  first run). The README says so, and releases carry `SHA256SUMS.txt`. The options (SignPath
  Foundation for OSS, Azure Trusted Signing, a paid cert) and the public-repo review (legal/ToS
  exposure of an unofficial TIDAL client) are an owner discussion after this track lands. The
  pipeline is written so a signing step can slot between `build` and `publish` without reshaping it.
- **T12-D9 — supply-chain hygiene.** Workflow permissions default to `contents: read`; only the
  `publish` job gets `contents: write`. Third-party actions are pinned to major versions from
  well-known publishers (`actions/*`, `dtolnay`, `Swatinem`). Secrets are only exposed to the
  tag-triggered release workflow, never to pull-request runs.

## 4. Components

| Piece | File(s) |
|---|---|
| CI workflow | `.github/workflows/ci.yml` |
| Release workflow | `.github/workflows/release.yml` |
| Release overlay | `crates/oxitide-app/tauri.release.conf.json` |
| Version check / manifest | `scripts/release/check-version.mjs`, `make-manifest.mjs`, `*.test.mjs` (`node --test`) |
| Updater service | `crates/oxitide-app/src/updater.rs`, `config.rs` (`[updates]`), `state.rs` (pending update), `main.rs` (plugin, handler list, startup check, shared exit work) |
| UI | `ui/src/lib/components/UpdatesSection.svelte` in Settings → System, `api.ts` types, `App.svelte` toast |
| Docs | `docs/release.md` (how to cut a release, key handling), README + `AGENTS.md` updates |

## 5. Testing

- **Automated (local):** Rust unit tests for the support matrix, `[updates]` config round trip, and
  the exit-work extraction; the RPC allowlist test covers command classification; `node --test` for
  the manifest and version scripts (fixture artifacts, sig contents, tag mismatch); vitest for the
  Updates section (supported / unsupported / available / installing / error states).
- **Local build proof:** a signed release-overlay build on Windows with a throwaway key confirms the
  `.sig` files and bundle names the manifest script expects.
- **Not provable locally (owner checklist, `plans/notes/`):** the workflows have never run on GitHub;
  first-run failures (runner audio devices, apt packages, cache paths) are expected and get fixed
  from real logs. The end-to-end update (install an older tag, publish a newer draft, update) needs
  two real releases and is an owner smoke.

## 6. Risks

- **Tests that assume a desktop** (audio devices, keyring, tray) may fail on hosted runners; they are
  fixed or gated with `cfg`/skips when the first CI run shows which.
- **Windows MSI + updater** on a per-machine install needs elevation; the NSIS per-user installer is
  the recommended download and is the manifest's default Windows entry.
- **First release must be installed manually** — an installed build only updates to releases made
  after the updater existed.
