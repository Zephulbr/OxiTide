# OxiTide

Beautiful rust client for Windows and Linux

## Development

Prerequisites: Rust (stable), Node 20+, and the workspace tooling.

```bash
# Install the UI dependencies (includes the Tauri CLI)
npm --prefix ui install

# Run the app in development (starts the Vite dev server, then the Tauri app)
npm --prefix ui run app:dev

# Build a packaged installer (MSI / NSIS)
npm --prefix ui run app:build

# Tests and checks
cargo test --workspace
cargo clippy --workspace -- -D warnings
npm --prefix ui run check
npm --prefix ui test
```

> Running the app requires the Tauri CLI. A bare `cargo run -p oxitide-app`
> (debug build) points the webview at `devUrl` (http://localhost:1420) and will
> fail with "localhost refused to connect" unless the Vite dev server is already
> running. Use `npm --prefix ui run app:dev` instead.
