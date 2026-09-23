# OxiTide

Beautiful rust client for Windows and Linux

## Development

Prerequisites: Rust (stable), Node 20+, and the workspace tooling.

```bash
# Install the UI dependencies (includes the Tauri CLI)
npm --prefix ui install

# Run the app in development (starts the Vite dev server, then the Tauri app)
npm --prefix ui run app:dev

# Build a packaged installer (NSIS per-user / MSI)
npm --prefix ui run app:build
# NSIS: target/release/bundle/nsis/OxiTide_0.2.0_x64-setup.exe
# MSI:  target/release/bundle/msi/OxiTide_0.2.0_x64_en-US.msi

# Tests and checks
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

> The NSIS installer is unsigned (SmartScreen warns) and installs per user
> without admin rights; the MSI installs per machine and needs admin. The
> installer downloads the WebView2 bootstrapper when WebView2 is missing.

> Running the app requires the Tauri CLI. A bare `cargo run -p oxitide-app`
> (debug build) points the webview at `devUrl` (http://localhost:1420) and will
> fail with "localhost refused to connect" unless the Vite dev server is already
> running. Use `npm --prefix ui run app:dev` instead.
