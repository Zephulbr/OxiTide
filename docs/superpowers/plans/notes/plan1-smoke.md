# Plan 1 Smoke Checklist

> Note: The items below require a human to launch the GUI and log into a real
> TIDAL account in a browser. They are PENDING a human run and are intentionally
> left unchecked. Automated checks (cargo build/test/clippy, UI build/test) were
> run separately and passed.

- [ ] `npm --prefix ui run build` then `cargo run -p oxitide-app` launches a window.
- [ ] Login screen: "Open login page" opens the TIDAL PKCE URL in the browser.
- [ ] Complete login; app shows Home.
- [ ] Home renders at least one section (or shows an empty state without crashing).
- [ ] Search "Beck" returns tracks and albums; clicking an album opens the album view with a populated track table.
- [ ] Library lists your playlists; clicking one opens the playlist view.
- [ ] Clicking Queue toggles the right rail.
- [ ] Logout clears the status; restarting the app requires login again (token cleared).
