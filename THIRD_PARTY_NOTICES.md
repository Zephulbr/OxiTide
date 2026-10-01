# Third-party notices

OxiTide bundles or links third-party software. This file records the attributions
and license terms. It is maintained by hand; the exhaustive per-crate license
texts ship inside each crate and can be assembled with `cargo about generate`
before distributing binaries.

## OxiTide

Copyright (C) 2026 OxiTide contributors.

OxiTide is free software: you can redistribute it and/or modify it under the
terms of the GNU Affero General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version. See `LICENSE` for the full text.

## Fonts (SIL Open Font License 1.1)

The UI vendors the following fonts under the SIL Open Font License 1.1:

- Fraunces — Copyright 2018 The Fraunces Project Authors
  (<https://github.com/undercasetype/Fraunces>)
- Manrope — Copyright 2018 The Manrope Project Authors
  (<https://github.com/googlefonts/manrope>)
- Inter — Copyright 2020 The Inter Project Authors
  (<https://github.com/rsms/inter>)

Font files: `ui/src/assets/fonts/`.

### SIL OPEN FONT LICENSE Version 1.1 - 26 February 2007

PREAMBLE
The goals of the Open Font License (OFL) are to stimulate worldwide
development of collaborative font projects, to support the font creation
efforts of academic and linguistic communities, and to provide a free and
open framework in which fonts may be shared and improved in partnership
with others.

The OFL allows the licensed fonts to be used, studied, modified and
redistributed freely as long as they are not sold by themselves. The
fonts, including any derivative works, can be bundled, embedded,
redistributed and/or sold with any software provided that any reserved
names are not used by derivative works. The fonts and derivatives,
however, cannot be released under any other type of license. The
requirement for fonts to remain under this license does not apply
to any document created using the fonts or their derivatives.

DEFINITIONS
"Font Software" refers to the set of files released by the Copyright
Holder(s) under this license and clearly marked as such. This may
include source files, build scripts and documentation.

"Reserved Font Name" refers to any names specified as such after the
copyright statement(s).

"Original Version" refers to the collection of Font Software components as
distributed by the Copyright Holder(s).

"Modified Version" refers to any derivative made by adding to, deleting,
or substituting -- in part or in whole -- any of the components of the
Original Version, by changing formats or by porting the Font Software to a
new environment.

"Author" refers to any designer, engineer, programmer, technical
writer or other person who contributed to the Font Software.

PERMISSION & CONDITIONS
Permission is hereby granted, free of charge, to any person obtaining
a copy of the Font Software, to use, study, copy, merge, embed, modify,
redistribute, and sell modified and unmodified copies of the Font
Software, subject to the following conditions:

1) Neither the Font Software nor any of its individual components,
in Original or Modified Versions, may be sold by itself.

2) Original or Modified Versions of the Font Software may be bundled,
redistributed and/or sold with any software, provided that each copy
contains the above copyright notice and this license. These can be
included either as stand-alone text files, human-readable headers or
in the appropriate machine-readable metadata fields within text or
binary files as long as those fields can be easily viewed by the user.

3) No Modified Version of the Font Software may use the Reserved Font
Name(s) unless explicit written permission is granted by the corresponding
Copyright Holder. This restriction only applies to the primary font name as
presented to the users.

4) The name(s) of the Copyright Holder(s) or the Author(s) of the Font
Software shall not be used to promote, endorse or advertise any
Modified Version, except to acknowledge the contribution(s) of the
Copyright Holder(s) and the Author(s) or with their explicit written
permission.

5) The Font Software, modified or unmodified, in part or in whole,
must be distributed entirely under this license, and must not be
distributed under any other license. The requirement for fonts to
remain under this license does not apply to any document created
using the Font Software.

TERMINATION
This license becomes null and void if any of the above conditions are
not met.

DISCLAIMER
THE FONT SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO ANY WARRANTIES OF
MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT
OF COPYRIGHT, PATENT, TRADEMARK, OR OTHER RIGHT. IN NO EVENT SHALL THE
COPYRIGHT HOLDER BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY,
INCLUDING ANY GENERAL, SPECIAL, INDIRECT, INCIDENTAL, OR CONSEQUENTIAL
DAMAGES, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING
FROM, OUT OF THE USE OR INABILITY TO USE THE FONT SOFTWARE OR FROM
OTHER DEALINGS IN THE FONT SOFTWARE.

## Vendored assets

- **Oneko** (`ui/src/assets/oneko/`) — see `ui/src/assets/oneko/LICENSE` for
  its license terms.

## Rust dependencies

Dependencies come from crates.io and are linked into the binary. The set is
dominated by permissive licenses (MIT, Apache-2.0, BSD, ISC, Zlib). The groups
in `Cargo.lock` (counts as of 2026-10-01):

| License expression | Packages |
|---|---|
| MIT OR Apache-2.0 | 268 |
| MIT | 128 |
| Apache-2.0 OR MIT | 49 |
| Zlib OR Apache-2.0 OR MIT | 17 |
| MPL-2.0 | 18 |
| Unicode-3.0 | 18 |
| MIT/Apache-2.0 | 16 |
| Unlicense OR MIT | 10 |
| Apache-2.0 WITH LLVM-exception OR Apache-2.0 OR MIT | 5 |
| MIT OR Apache-2.0 OR Zlib | 4 |
| BSD-3-Clause | 3 (alloc-no-stdlib, alloc-stdlib, subtle) |
| ISC | 3 (libloading, rustls-webpki, untrusted) |
| Apache-2.0/MIT | 4 (cesu8, dbus, libdbus-sys, fnv) |
| BSD-3-Clause AND MIT | 1 (brotli) |
| BSD-3-Clause/MIT | 1 (brotli-decompressor) |
| BSD-3-Clause OR Apache-2.0 | 2 (moxcms, pxfm) |
| BSD-3-Clause OR MIT OR Apache-2.0 | 2 (num_enum, num_enum_derive) |
| BSD-2-Clause OR Apache-2.0 OR MIT | 2 (zerocopy, zerocopy-derive) |
| MIT OR Zlib OR Apache-2.0 | 2 (miniz_oxide) |
| MIT OR Apache-2.0 OR LGPL-2.1-or-later | 2 (r-efi — used under MIT OR Apache-2.0) |
| Apache-2.0 OR ISC OR MIT | 2 (hyper-rustls, rustls) |
| Apache-2.0 | 2 (sync_wrapper, tao) |
| Apache-2.0 WITH LLVM-exception | 1 (target-lexicon) |
| Apache-2.0 AND ISC | 1 (ring) |
| Apache-2.0 AND MIT | 1 (dpi) |
| Apache-2.0 OR BSL-1.0 | 1 (ryu) |
| 0BSD OR MIT OR Apache-2.0 | 1 (adler2) |
| CC0-1.0 | 1 (tiny-keccak) |
| CC0-1.0 OR MIT-0 OR Apache-2.0 | 1 (dunce) |
| CDLA-Permissive-2.0 | 1 (webpki-roots) |
| Zlib | 2 (foldhash, zlib-rs) |
| Unlicense/MIT | 2 (same-file, walkdir) |
| (MIT OR Apache-2.0) AND Unicode-3.0 | 1 (unicode-ident) |

Notice-sensitive items worth calling out:

- **MPL-2.0** (file-level copyleft, unmodified): `symphonia` and its
  `symphonia-*` crates (0.6.1), `cssparser`, `selectors`, `dtoa-short`,
  `option-ext`.
- **CDLA-Permissive-2.0**: `webpki-roots` (Mozilla CA root data).
- **Unicode-3.0**: the `icu_*`/`zerovec` family.

To regenerate the full inventory (and, with `cargo-about`, the complete license
texts for every crate), run from the workspace root:

```bash
cargo metadata --format-version 1 --locked
# or, for a full notices bundle:
cargo about generate about.hbs > THIRD_PARTY_NOTICES.full.md
```

## Optional ASIO backend (not currently compiled)

If the optional ASIO output backend is ever enabled, it links code generated
from the Steinberg ASIO SDK. Steinberg dual-licenses the SDK (proprietary or
GPLv3); under the GPLv3 option the combined binary must carry the SDK's license
and notices. Add them here before shipping such a build.
