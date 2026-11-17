# Contributing to OxiTide

OxiTide is an unofficial TIDAL desktop client maintained by one person, mostly for personal
use. Bug reports, translations and small fixes are welcome.

- **Bugs and ideas:** open an issue on GitHub (github.com/Zephulbr/OxiTide). Include your
  OS, the OxiTide version (Settings → System → Updates) and what you did.
- **Code:** read `AGENTS.md` first. It lists the build commands, the gates CI runs and the
  rules every change keeps. Keep a pull request to one change.
- **Translations:** see below. You don't need to build anything.

## Translating

OxiTide's interface is in English plus 23 languages. Most of them were **drafted by a
machine** and nobody has checked them yet, so expect wrong words, stiff phrasing and the odd
mistranslation. Corrections are the most useful thing you can send.

### Report a wrong translation (no files)

Open a [translation issue](https://github.com/Zephulbr/OxiTide/issues/new?template=translation.yml).
Say which language, where in the app the text appears, what it says now and what it should
say. A screenshot helps.

### Fix it yourself

1. Find the text in `ui/src/locales/<language>/`:
   - `app.json`: the desktop app (almost everything);
   - `remote.json`: the phone remote;
   - `shell.json`: the tray menu and the taskbar buttons.

   Each file is a flat list of `"key": "text"`. Search the file for the wrong text; the
   English for the same key is in `ui/src/locales/en/`, and a note for translators (where
   the text appears, what it means) is in `ui/src/locales/notes/`.
2. Edit the text. GitHub's web editor is enough ("Edit this file", then "Propose changes").
   Change only the text after the colon, never the key.
3. Open a pull request. CI checks every catalog (`node scripts/i18n/check.mjs`) and tells
   you if something broke a placeholder or a plural.

Don't edit `ui/src/locales/_meta/`: it records which messages a person has reviewed, and
the maintainer updates it after merging (`node scripts/i18n/sync.mjs`). Your edits count as
reviewed from then on, and a later machine draft never overwrites them.

### Message format in five lines

Messages use [ICU MessageFormat](https://unicode-org.github.io/icu/userguide/format_parse/messages/):

- `{name}` is filled in by the app: keep every placeholder, spelled exactly the same. You
  may move it within the sentence.
- `{count, plural, one {# track} other {# tracks}}` picks a form by number; `#` is the
  number. Use your language's categories (`one`, `few`, `many`, `other`…); `other` is
  always required. `=0` / `=1` match an exact number.
- `{kind, select, album {…} other {…}}` picks by a word; translate inside the braces, keep
  the words before them.
- A literal `{` or `}` is written `'{'`. An apostrophe right before `{`, `}` or `#` is
  written twice (`''`); anywhere else, write it as usual (or use your language's `’`).
- Never join two messages into one sentence in your head: each message is a whole
  sentence or label on its own.

### Words to keep

- Don't translate names: OxiTide, TIDAL, Hi-Fi Rack, AutoMix, CatJam, ASIO, WASAPI,
  ReplayGain, Last.fm, ListenBrainz, quality names such as Max and High (the full list is
  `doNotTranslate` in `ui/src/locales/glossary.json`).
- Use the glossary's word for recurring terms (Playlist, Queue, My Collection…): each term
  lists the rendering each language uses and a style line (formal or informal "you", and
  so on). If the glossary's choice is wrong for your language, change it there in the same
  pull request.
- Don't decline or inflect names that the app inserts (`{title}`, `{artist}`): build the
  sentence so the name stands on its own, for example after a colon.
- The Hi-Fi Rack's painted labels (nameplates, key captions) stay English on purpose.

### Try your change

With [Node](https://nodejs.org) 24 and [Rust](https://rustup.rs) installed:

```sh
npm --prefix ui install
npm --prefix ui run app:dev
```

Then Settings → Appearance → Language. Check that the text fits: long words are cut off with
"…" where space runs out. `node scripts/i18n/check.mjs` runs the same check as CI, and
`node scripts/i18n/review-sheet.mjs <language>` writes a side-by-side sheet (English, your
language, the note) to read through.

### Add a language

Copy `ui/src/locales/en/` to `ui/src/locales/<code>/` (a short BCP 47 code: `ca`, `vi`,
`pt-PT`), translate it, and add a row to `ui/src/locales/languages.json`
(`"ship": false` until it is complete). The tray text is embedded in the app: add the
language to the `catalogs![…]` list in `crates/oxitide-app/src/shell_text.rs` too. A new
script (Arabic, Hebrew, Thai…) may also need fonts or right-to-left layout, which the app
does not have yet: open an issue first.
