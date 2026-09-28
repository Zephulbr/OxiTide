# Seamless Login — Smoke Checklist

Spec: `docs/superpowers/specs/2026-09-28-oxitide-seamless-login-design.md`
Run: `npm --prefix ui run app:dev` (real account, browser login)

- [x] Email/password: Log in → browser → sign in → OxiTide returns logged in.
- [x] Google SSO: same, via Continue with Google.
- [x] Apple SSO: same, via Continue with Apple.
- [x] Already running: app open, login completes in place (single-instance forwarding).
- [x] Cold start: quit OxiTide, click Log in, complete in browser; the deep link launches and logs in OxiTide.
- [x] Browser handoff prompt appears once and Open launches OxiTide.
- [x] Fallback: open "Browser didn't open the app?", use "Open browser (paste-back)", paste the Oops URL, Complete login succeeds.
- [x] Cancel: while waiting, Cancel returns to the idle login screen with no error.
- [x] Scheme restored: after quitting OxiTide, `Start-Process "tidal://login/auth"` reaches the official TIDAL app (or `reg query HKCU\Software\Classes\tidal` no longer points at OxiTide).
- [ ] No token values appear in `%APPDATA%\OxiTide\logs\`.

Notes:
