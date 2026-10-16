# Offline audio test fixtures

The six `he_aac_*_96k_*.mp4` files are self-generated HE-AAC acceptance fixtures,
not recordings or downloaded music. They contain the same encoded access units
and ASC forms as the original 2026-10-16 decoder spike, repackaged as **streamable
fragmented MP4**. The existing `tone*` fixtures were not changed.

Tests consume the checked-in files directly: **no internet, encoder, generator,
FFmpeg, NumPy, or ignored development files are needed at test runtime**.

## Standalone regeneration

From the repository/worktree root, run Python 3.9+ with the path to an existing
`fdkaac` executable (replace the example absolute path with your local path):

```sh
python -B crates/oxitide-audio/tests/fixtures/generate_he_aac.py --encoder C:/tools/fdkaac.exe
```

`--encoder` is required. The generator uses only Python's standard library and
that executable; it does not download anything, run FFmpeg/Rust, or import any
ignored spike files. Intermediate WAV, ADTS, and staged output files are created
inside a `tempfile.TemporaryDirectory` and removed on exit. After both profiles
reproduce the golden access units, it replaces only the six named MP4s and
`he_aac_96k_provenance.json` alongside the script.

The exact reference encoder is RareWares **fdkaac 1.0.5 x64**, linked with
**FDK AAC 2.0.2 according to its publisher**:

- Publisher: <https://www.rarewares.org/aac-encoders.php>
- Reference archive: <https://www.rarewares.org/files/aac/fdkaac-1.0.5-x64.zip>
- Archive SHA-256:
  `2717cf55c641326cd7551f337ca5b2220764ce21aea609b24dd3d52755f32d4a`
- Extracted `fdkaac.exe` SHA-256:
  `f8831e22a8e42e71f24353397d6ed3fd9311dc975d66a696832be9463eacfb22`

These URLs document provenance; neither regeneration nor tests contact them.
Other encoder builds are accepted only if they reproduce the original access
units exactly. The generator checks a pinned hash over every little-endian
u32 packet length followed by its unchanged AAC payload, so both packet bytes
and packet boundaries are protected. It fails rather than silently regenerating
different audio. Exact regeneration was repeated successfully on Windows with
Python 3.13 and the reference encoder.

Encoder arguments (input/output paths are temporary):

```text
fdkaac -p 5  -b 96000 -s 2 -f 2 --no-timestamp -o he-aac-v1-96k.aac synthetic-stereo-48k.wav
fdkaac -p 29 -b 96000 -s 2 -f 2 --no-timestamp -o he-aac-v2-96k.aac synthetic-stereo-48k.wav
```

`-b 96000` requests CBR 96 kb/s. After stripping seven ADTS header bytes per
2048 output frames, raw AAC payload averages about **94.69 kb/s**; this is not a
profile downgrade. ADTS is used only as the encoder's intermediate transport.
The fixture decoder consumes **raw ASC plus raw AAC access units**, not ADTS.

## Source and rights provenance

The generator creates an original 3-second, 48000 Hz stereo signed-16-bit WAV:

- Left: 440, 6000, and 14000 Hz tones; right: 660, 7500, and 16000 Hz tones.
- A chirp with phase `2*pi*(1100*t + 650*t*t)` and opposite channel polarity.
- Four-per-second decaying noise bursts from xorshift32 seed `0x12345678`.
- 10 ms start/end fades. Exact amplitudes/formulas are in `generate_he_aac.py`.

Asymmetric channels test PS; substantial content above the core's 12 kHz
Nyquist tests actual SBR reconstruction rather than plausible output metadata.
Source WAV SHA-256:
`54f2858ae60124f30fd37c77058582450d9c35d15ceaa808202df87e87f65726`.

No third-party recording, music sample, speech, live TIDAL media, credentials,
or API response was used. This is original procedural project test data under
`AGPL-3.0-or-later`. No encoder binary, downloaded source, build cache, or Python
package is part of this fixture set. Content provenance does not establish AAC
patent clearance or grant redistribution rights to an external encoder.

`he_aac_96k_provenance.json` records the tracked generator/source hashes, encoder
hash and arguments, full fixture hashes, access-unit/payload hashes, raw ASC,
counts, and exact init/fragment boundaries. It has no dependency on ignored files.

## Fragment layout and engine-test handoff

Every file has exactly these top-level boxes, in order:

```text
ftyp + moov | moof + mdat | moof + mdat
   init    | fragment 1  | fragment 2
```

The init contains one `mp4a`/`esds` description, empty `stts/stsc/stsz/stco`
tables, and `mvex/trex`. Track id and description index are both 1. `trex`
default duration is **1024 ticks at timebase 1/24000**. Init durations are zero
(unknown); there is no edit list, gapless trim, index, or seek-back requirement.
Each `tfhd` uses `default-base-is-moof`; each `trun` gives sample sizes and a
moof-relative data offset. Version-1 `tfdt` uses the **24 kHz core timeline**.
DTS equals PTS, with no composition offsets.

Byte ranges below are **zero-based, half-open**. Each fragment range includes
its complete `moof` and `mdat`; no test-side MP4 repackaging is needed.

| Filename | Init | Fragment 1 | Fragment 2 |
|---|---|---|---|
| `he_aac_v1_96k_hierarchical.mp4` | `[0, 603)` | `[603, 12916)` | `[12916, 37953)` |
| `he_aac_v1_96k_sync.mp4` | `[0, 604)` | `[604, 12917)` | `[12917, 37954)` |
| `he_aac_v1_96k_implicit.mp4` | `[0, 601)` | `[601, 12914)` | `[12914, 37951)` |
| `he_aac_v2_96k_hierarchical.mp4` | `[0, 603)` | `[603, 12915)` | `[12915, 38461)` |
| `he_aac_v2_96k_sync.mp4` | `[0, 606)` | `[606, 12918)` | `[12918, 38464)` |
| `he_aac_v2_96k_implicit.mp4` | `[0, 601)` | `[601, 12913)` | `[12913, 38459)` |

Fragment 1 always contains packets **0..24**, `tfdt=0`, duration **24576 core
ticks**, and **49152 decoded PCM frames/channel**. Fragment 2 starts at packet
24, `tfdt=24576` (1.024 s):

| Profile | Remaining packets | Fragment 2 core ticks | Fragment 2 PCM frames | Total PCM frames |
|---|---:|---:|---:|---:|
| HE-AAC v1 | 49 | 50176 | 100352 | 149504 |
| HE-AAC v2 | 50 | 51200 | 102400 | 151552 |

Output is always **48000 Hz stereo**, **2048 frames/access unit**. Counts include
encoder priming/padding, not just the 144000 source frames. Full durations are
3.1146667 s (v1) and 3.1573333 s (v2). All six MP4s total **229242 bytes**.

All three signaling forms per version contain identical encoded packets:

| Version | Hierarchical ASC | Sync-extension ASC | Implicit ASC |
|---|---|---|---|
| v1 | `2b118800` (AOT 5) | `131056e598` | `1310` |
| v2 | `eb098800` (AOT 29) | `130856e59d4880` | `1308` |

The AAC core is 24000 Hz stereo for v1 and 24000 Hz mono for v2; SBR/PS produces
48000 Hz stereo. Implicit ASC declares only AAC-LC, so extensions must be learned
in-band. Demux hints are not the final output format.

## Independent acceptance findings

Revalidated after fragmenting with **Symphonia 0.6.1 demuxing through a truly
nonseekable `ReadOnlySource`**, raw ASC, and **rusty_aac 1.1.0** packet decoding.
Independent reference: FFmpeg 9.0.2 native AAC decoder. Every fixture also
passed FFmpeg stdin decoding and decoded all 24 first-fragment packets with
only the init plus fragment 1 available. Thus no final fragment or tail metadata
is needed to start emitting audio. The ordinary-file and stdin FFmpeg PCM are
identical; candidate PCM is unchanged from the original nonfragmented fixtures.

Useful optional maintainer commands with an already-installed FFmpeg (these are
not runtime test prerequisites):

```sh
ffprobe -v error -show_streams -show_data -of json crates/oxitide-audio/tests/fixtures/he_aac_v2_96k_implicit.mp4
ffmpeg -hide_banner -nostdin -v error -i crates/oxitide-audio/tests/fixtures/he_aac_v2_96k_implicit.mp4 -c:a pcm_f32le -f f32le reference.f32le
```

| Clean family | Output | Candidate/reference frames | PCM SNR | Max absolute float error |
|---|---|---:|---:|---:|
| HE-AAC v1, all three ASC forms | 48 kHz stereo | 149504 | 103.225 dB | 1.34241e-5 |
| HE-AAC v2, all three ASC forms | 48 kHz stereo | 151552 | 124.286 dB | 6.63102e-7 |
| Existing `tone_aac.mp4` | 44.1 kHz stereo | 90112 | 136.174 dB | 5.21541e-8 |

All seven files pass in SIMD-enabled release, scalar release, and debug builds
(**21 comparisons**). Unchanged gates: exact rate/channels/frame count, no clean
decode errors or nonfinite samples, PCM SNR >=60 dB, max absolute error <=0.001,
active-band and stereo side-energy error <=0.25 dB. No alignment, resampling,
gain fitting, or channel remapping is used. Maximum rounded 16-bit error is one
LSB on this set; worst active-band difference is 1.172e-5 dB.

**PASS is qualified by fail-fast behavior**: any packet decode error ends the
current track, no later packet is submitted, and the decoder is discarded.
A fresh next-track decoder is created from that track's raw ASC. The six-byte
malformed AU `a0 00 00 00 00 00` still poisons in-place decoder recovery; that
upstream defect is not fixed or hidden by these fixtures. All six profile/build
negative controls stop after the first error, and all six fresh-next-track
controls produce sample-identical full PCM. Continuing after an error is **not
accepted**. No production integration or seek accounting was changed here.

No panics, hangs, or nonfinite output were observed in the bounded malformed
corpus (90 packet cases plus seven ASC cases per fixture/build). This is not an
exhaustive fuzz, allocation-bound, long-run performance, or licensing audit.
Implicit `output_sample_rate()` initially reports 24 kHz before first decode,
then 48 kHz; `sbr_support()` stays misleadingly `NotPresent`. No later format
changes were observed in these valid fixtures. Upstream's broader conformance
claims remain unverified beyond this independently generated set.
