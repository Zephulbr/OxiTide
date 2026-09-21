# Plan 1 Smoke Checklist

> Note: The items below require a human to launch the GUI and log into a real
> TIDAL account in a browser. They are PENDING a human run and are intentionally
> left unchecked. Automated checks (cargo build/test/clippy, UI build/test) were
> run separately and passed.
>
> Run the app with `npm --prefix ui run app:dev` (starts the Vite dev server via
> the Tauri CLI, then launches the app). A bare `cargo run -p oxitide-app` does
> NOT work: a debug build loads `devUrl` (http://localhost:1420) and fails with
> "localhost refused to connect" unless the dev server is already running.
> To run the packaged assets instead, use `npm --prefix ui run app:build`.

- [x] `npm --prefix ui run app:dev` launches a window showing the app (not an Edge "can't reach this page" error).
- [x] Login screen: "Open login page" opens the TIDAL PKCE URL in the browser.
- [ ] Complete login; app shows Home.
- [ ] Home renders at least one section (or shows an empty state without crashing).
- [x] Search "Beck" returns tracks and albums; clicking an album opens the album view with a populated track table.
- [ ] Library lists your playlists; clicking one opens the playlist view.
- [x] Clicking Queue toggles the right rail.
- [x] Logout clears the status; restarting the app requires login again (token cleared).
