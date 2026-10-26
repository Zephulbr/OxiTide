# Privacy

OxiTide has no telemetry and no accounts of its own. This page lists everything that leaves your machine and
how to control each one. The short version is in the [README](../README.md#privacy-and-security).

## Network destinations

| Destination | When | Control |
|---|---|---|
| TIDAL (login, catalog, streams, images) | Always, to function | n/a |
| GitHub (update manifest) | Shortly after start and on "Check for updates" | Settings → System → Updates |
| Last.fm / ListenBrainz | Only if you connect an account | Settings → Connections |
| Discord (local IPC) | Only if enabled | Settings → Connections |
| LRCLIB (lyrics) | When you open lyrics | n/a |

Beat detection for CatJam runs on your machine. No track data is sent to any service for it.

## Credentials

- Your session tokens are stored in the OS keyring: Windows Credential Manager, or the Secret Service on Linux.
  Without a keyring, the session lasts only until you quit, and the app tells you.
- Tokens are never logged.
- The login uses OAuth PKCE, so there is no client secret. The TIDAL OAuth *client ID* in the source is a public
  identifier, not a credential.
- Last.fm keys are supplied by you (in `config.toml`, or through `OXITIDE_LASTFM_API_KEY` /
  `OXITIDE_LASTFM_API_SECRET`). They are not committed to the repository.

## Phone remote

Off by default. When on, it listens only for devices on your local network (private and CGNAT ranges), requires QR
pairing, and exposes a fixed allowlist of actions. Pairing codes are single-use and expire after five minutes.

## Cache

Recently played tracks are cached on disk for fast replay and gapless transitions. The cache has a size limit you
choose (Settings → Storage) and evicts old entries automatically. It is a playback buffer, not a library or an
offline mode. Clear it at any time, or set it to clear on exit.

## Data location

Everything lives under `%APPDATA%\OxiTide\`: settings in `config\`, cache in `config\cache\`, logs in `logs\`.
On Linux, it lives under the XDG config directory.
