# Track 8.2 — ALSA Output Backend Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give OxiTide a real Linux `AudioOutput` implementation — direct `hw:` exclusive bit-perfect output, `default` shared output, `ExclusiveFallback`, device enumeration, and hardware/software volume — with Windows behavior untouched.

**Architecture:** `crates/oxitide-audio/src/alsa_output.rs` is compiled on every platform: it holds platform-neutral pure functions (format candidates, sample packing, device-hint parsing, volume conversion, fallback decisions) with unit tests that run on Windows. The ALSA I/O (`PCM`, `HwParams`, `Mixer`) lives behind `#[cfg(target_os = "linux")]` and is verified in WSL2 Ubuntu 22.04, using ALSA's built-in `null` PCM for hardware-free lifecycle tests. The crate gains `alsa` only as a Linux target dependency; `real_output()` in `oxitide-app` and `enumerate_output_devices()` in `oxitide-audio` gain Linux arms.

**Tech Stack:** Rust (alsa 0.9, existing `AudioOutput` trait), WSL2 Ubuntu 22.04 for Linux compilation/tests.

**Spec:** `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` (§4.1, §4.8, §5.1–5.2, Plan 8.2)

## Global Constraints

- Windows-first: all Linux-only code is `#[cfg(target_os = "linux")]`; Windows behavior and outputs must not change.
- No new Windows dependencies; `alsa` is added only under `[target.'cfg(target_os = "linux")'.dependencies]`.
- No CI: gates are manual. Windows: `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`. Linux cfg code is compiled and tested in WSL2 Ubuntu 22.04 (the environment from Plan 8.1), repo mounted at its Windows path.
- Pure logic must be testable on Windows; Linux-only tests may use the `null` ALSA PCM and must not require audio hardware.
- The existing `AudioOutput` contract (`open`/`start`/`write`/`set_volume`/`flush`/`stop`/`status`) is fixed; `OutputMode`, `ExclusiveFallback`, `OutputStatus`, `exclusive_formats()` semantics match `crates/oxitide-audio/src/output.rs`.
- The engine supplies `PcmBuffer::I32` (left-aligned, 32-bit container) for bit-perfect exclusive playback and `PcmBuffer::F32` otherwise (see `crates/oxitide-audio/src/engine.rs:880`); the backend must accept both.
- Volume semantics mirror WASAPI (`crates/oxitide-audio/src/wasapi_output.rs:217-243`): exclusive tries hardware volume first; if unavailable, software scaling sets `bit_perfect = false` and the exact notice `volume applied in software; bit-perfect compromised`. Shared always scales in software and keeps its existing status.
- No UI/command changes in this plan; settings already flow through `set_output_device` / `set_output_mode` / `set_exclusive_fallback`.

## Review Focus

- **Exclusive device busy** (PipeWire/Pulse or another app holds the DAC): open must not fail the app — with `ExclusiveFallback::Shared` it reopens `default`, sets `notice`, and playback continues; with `WarnOnly` it returns an error so the engine pauses with a notice. Pinned by `exclusive_fallback_plan_*` tests and the WSLg smoke item.
- **DAC rejects 32-bit containers**: negotiation must step down S32LE → S24_3LE → S16LE with correct sample packing, never emit silence or wrong words. Pinned by `alsa_formats_step_down_for_narrow_sources` and `pack_*` tests.
- **Underrun/xrun after a stall or seek**: `write` must recover via `try_recover` and continue; it must never spin forever or surface a permanent error. Pinned by the `null`-PCM lifecycle test; xrun behavior smoke-checked in WSLg.
- **Stale/missing selected device** (unplugged USB, bogus `hw:` id): opening must fail cleanly into the configured fallback/error path, not panic or hang. Pinned by `exclusive_warn_only_errors_on_a_missing_device`.
- **Software volume in exclusive mode**: when no mixer element is writable, volume must be applied in software with the exact notice and `bit_perfect = false`; when a mixer works, the notice clears and bit-perfect is restored. Pinned by `software_volume_notice_constant_matches_wasapi` and `volume_math_*` tests.

---

## File Structure

- Modify `crates/oxitide-audio/Cargo.toml` — add `[target.'cfg(target_os = "linux")'.dependencies] alsa = "0.9"`.
- Modify `crates/oxitide-audio/src/lib.rs` — `pub mod alsa_output;`.
- Create `crates/oxitide-audio/src/alsa_output.rs` — pure mapping + Linux `AlsaOutput` + `enumerate()`, with both test sets.
- Modify `crates/oxitide-audio/src/output.rs` — Linux arm for `enumerate_output_devices()`; keep the empty stub for other non-Windows targets.
- Modify `crates/oxitide-app/src/player.rs` — `real_output()` Linux arm.
- Create `docs/superpowers/plans/notes/2026-10-03-track8-2-alsa-smoke.md` — WSLg smoke checklist for real shared/exclusive playback (the track-wide lists live in Plans 8.5's notes).

---

### Task 1: Pure format candidates and sample packing

**Files:**
- Create: `crates/oxitide-audio/src/alsa_output.rs`
- Modify: `crates/oxitide-audio/src/lib.rs:16` (after `pub mod wasapi_output;`)

**Interfaces:**
- Consumes: `crate::decode::{PcmFormat, StreamInfo}`, `crate::output::exclusive_formats`.
- Produces: `AlsaSampleFormat` (`S32Le`, `S24_3Le`, `S16Le`) with `store_bits(self) -> u16`; `alsa_exclusive_formats(info: &StreamInfo) -> Vec<AlsaSampleFormat>`; `pack_i32(format: AlsaSampleFormat, samples: &[i32], out: &mut Vec<u8>)`.

- [ ] **Step 1: Write the failing tests**

Create `crates/oxitide-audio/src/alsa_output.rs` with only the test module, so it fails to compile against missing items:

```rust
use crate::decode::{PcmFormat, StreamInfo};
use crate::output::exclusive_formats;

#[cfg(test)]
mod tests {
    use super::*;

    fn info(bits: u16, format: PcmFormat) -> StreamInfo {
        StreamInfo { sample_rate: 44_100, channels: 2, bits_per_sample: bits, format }
    }

    #[test]
    fn alsa_formats_step_down_for_narrow_sources() {
        assert_eq!(
            alsa_exclusive_formats(&info(16, PcmFormat::I32 { bits: 16 })),
            vec![AlsaSampleFormat::S32Le, AlsaSampleFormat::S24_3Le, AlsaSampleFormat::S16Le]
        );
        assert_eq!(
            alsa_exclusive_formats(&info(24, PcmFormat::I32 { bits: 24 })),
            vec![AlsaSampleFormat::S32Le, AlsaSampleFormat::S24_3Le]
        );
        assert_eq!(
            alsa_exclusive_formats(&info(32, PcmFormat::I32 { bits: 32 })),
            vec![AlsaSampleFormat::S32Le]
        );
    }

    #[test]
    fn alsa_formats_are_empty_for_non_candidates() {
        assert!(alsa_exclusive_formats(&info(16, PcmFormat::F32)).is_empty());
    }

    #[test]
    fn store_bits_matches_the_container_or_packed_width() {
        assert_eq!(AlsaSampleFormat::S32Le.store_bits(), 32);
        assert_eq!(AlsaSampleFormat::S24_3Le.store_bits(), 24);
        assert_eq!(AlsaSampleFormat::S16Le.store_bits(), 16);
    }

    #[test]
    fn pack_s32le_preserves_left_aligned_words() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S32Le, &[0x7fff_ff00u32 as i32, -256], &mut out);
        assert_eq!(out, vec![0x00, 0xff, 0xff, 0x7f, 0x00, 0xff, 0xff, 0xff]);
    }

    #[test]
    fn pack_s24_3le_keeps_the_top_three_bytes() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S24_3Le, &[0x7fff_ff00u32 as i32], &mut out);
        assert_eq!(out, vec![0xff, 0xff, 0x7f]);
    }

    #[test]
    fn pack_s16le_keeps_the_top_two_bytes() {
        let mut out = Vec::new();
        pack_i32(AlsaSampleFormat::S16Le, &[0x7fff_0000u32 as i32, 0], &mut out);
        assert_eq!(out, vec![0xff, 0x7f, 0x00, 0x00]);
    }

    #[test]
    fn pack_clears_previous_contents() {
        let mut out = vec![0xaa, 0xbb];
        pack_i32(AlsaSampleFormat::S32Le, &[], &mut out);
        assert!(out.is_empty());
    }
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-audio alsa`
Expected: compile error — `cannot find type AlsaSampleFormat`, `cannot find function alsa_exclusive_formats`, `cannot find function pack_i32` — the failing state.

- [ ] **Step 3: Implement**

Replace the top of `crates/oxitide-audio/src/alsa_output.rs` (keep the tests below it):

```rust
use crate::decode::{PcmFormat, StreamInfo};
use crate::output::exclusive_formats;

/// The ALSA sample formats OxiTide writes, best quality first. All are
/// little-endian and use a 32-, 24-, or 16-bit container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsaSampleFormat {
    S32Le,
    S24_3Le,
    S16Le,
}

impl AlsaSampleFormat {
    pub fn store_bits(self) -> u16 {
        match self {
            AlsaSampleFormat::S32Le => 32,
            AlsaSampleFormat::S24_3Le => 24,
            AlsaSampleFormat::S16Le => 16,
        }
    }
}

/// Formats to try, in order, for an exclusive open. Empty when the source is
/// not an integer 16/24/32-bit bit-perfect candidate. The 32-bit container is
/// preferred (symphonia left-aligns integer samples to 32 bits); packed 24-bit
/// and 16-bit containers are the step-down options for DACs without S32.
pub fn alsa_exclusive_formats(info: &StreamInfo) -> Vec<AlsaSampleFormat> {
    let PcmFormat::I32 { bits } = info.format else {
        return Vec::new();
    };
    if exclusive_formats(info).is_empty() {
        return Vec::new();
    }
    let mut out = vec![AlsaSampleFormat::S32Le];
    if bits <= 24 {
        out.push(AlsaSampleFormat::S24_3Le);
    }
    if bits <= 16 {
        out.push(AlsaSampleFormat::S16Le);
    }
    out
}

/// Pack left-aligned i32 samples into interleaved little-endian bytes for
/// `format`. `out` is cleared first. S24_3Le keeps bytes 1..4 of the LE word
/// and S16Le bytes 2..4, which drops the low padding bits symphonia leaves in
/// the 32-bit container.
pub fn pack_i32(format: AlsaSampleFormat, samples: &[i32], out: &mut Vec<u8>) {
    out.clear();
    match format {
        AlsaSampleFormat::S32Le => {
            for &sample in samples {
                out.extend_from_slice(&sample.to_le_bytes());
            }
        }
        AlsaSampleFormat::S24_3Le => {
            for &sample in samples {
                let bytes = sample.to_le_bytes();
                out.extend_from_slice(&bytes[1..4]);
            }
        }
        AlsaSampleFormat::S16Le => {
            for &sample in samples {
                let bytes = sample.to_le_bytes();
                out.extend_from_slice(&bytes[2..4]);
            }
        }
    }
}
```

In `crates/oxitide-audio/src/lib.rs`, add `pub mod alsa_output;` after `pub mod cache;` (keeping the unconditional modules roughly alphabetical).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-audio alsa`
Expected: PASS (7 tests).
Run: `cargo clippy -p oxitide-audio -- -D warnings`
Expected: no warnings (on Windows the module contains only public pure functions, so nothing is dead code).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/alsa_output.rs crates/oxitide-audio/src/lib.rs
git commit -m "feat(audio): add pure ALSA format mapping and packing"
```

---

### Task 2: Pure device hints, conversion, volume, and fallback decision

**Files:**
- Modify: `crates/oxitide-audio/src/alsa_output.rs` (append before `#[cfg(test)]`)

**Interfaces:**
- Consumes: `crate::output::{ExclusiveFallback, OutputDevice}`.
- Produces: `Hint { name: String, desc: Option<String>, ioid: Option<String> }`; `device_list(hints: &[Hint]) -> Vec<OutputDevice>`; `f32_to_i32_one(sample: f32) -> i32`; `raw_to_volume(raw: i64, min: i64, max: i64) -> f32`; `volume_to_raw(volume: f32, min: i64, max: i64) -> i64`; `exclusive_fallback_plan(fallback: ExclusiveFallback) -> AlsaOpen`; `AlsaOpen { Exclusive, SharedWithNotice, Fail }`.

- [ ] **Step 1: Write the failing tests**

Append inside the existing `mod tests` in `crates/oxitide-audio/src/alsa_output.rs`:

```rust
    use crate::output::{ExclusiveFallback, OutputDevice};

    fn hint(name: &str, desc: Option<&str>, ioid: Option<&str>) -> Hint {
        Hint { name: name.into(), desc: desc.map(str::to_string), ioid: ioid.map(str::to_string) }
    }

    #[test]
    fn device_list_filters_inputs_and_unpickable_names() {
        let list = device_list(&[
            hint("default", Some("Default output"), None),
            hint("sysdefault:CARD=X", Some("X"), Some("Output")),
            hint("hw:CARD=X,DEV=0", Some("X direct"), Some("Output")),
            hint("mic", Some("Microphone"), Some("Input")),
            hint("surround21:CARD=X,DEV=0", Some("Surround"), Some("Output")),
            hint("null", Some("Null"), Some("Output")),
        ]);
        let ids: Vec<&str> = list.iter().map(|d| d.id.as_str()).collect();
        assert_eq!(ids, vec!["default", "hw:CARD=X,DEV=0", "sysdefault:CARD=X"]);
    }

    #[test]
    fn device_list_marks_default_and_dedupes() {
        let list = device_list(&[
            hint("hw:CARD=X,DEV=0", Some("Direct"), Some("Output")),
            hint("default", Some("Default"), None),
            hint("default", Some("Default again"), None),
        ]);
        assert!(list[0].is_default);
        assert_eq!(list[0].name, "Default");
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn device_list_falls_back_to_the_device_name_as_label() {
        let list = device_list(&[hint("sysdefault:CARD=Y", None, None)]);
        assert_eq!(list[0].name, "sysdefault:CARD=Y");
        assert_eq!(list[0], OutputDevice {
            id: "sysdefault:CARD=Y".into(),
            name: "sysdefault:CARD=Y".into(),
            is_default: false,
        });
    }

    #[test]
    fn f32_conversion_saturates_at_the_rails() {
        assert_eq!(f32_to_i32_one(0.0), 0);
        assert_eq!(f32_to_i32_one(1.0), i32::MAX);
        assert_eq!(f32_to_i32_one(-1.0), i32::MIN);
        assert_eq!(f32_to_i32_one(2.0), i32::MAX);
        assert_eq!(f32_to_i32_one(f32::NAN), 0);
    }

    #[test]
    fn volume_math_maps_both_directions() {
        assert_eq!(raw_to_volume(0, 0, 100), 0.0);
        assert_eq!(raw_to_volume(50, 0, 100), 0.5);
        assert_eq!(raw_to_volume(100, 0, 100), 1.0);
        assert_eq!(volume_to_raw(0.5, 0, 100), 50);
        assert_eq!(volume_to_raw(1.0, 0, 100), 100);
        assert_eq!(raw_to_volume(10, 10, 10), 1.0);
    }

    #[test]
    fn exclusive_fallback_plan_matches_wasapi_semantics() {
        assert_eq!(
            exclusive_fallback_plan(ExclusiveFallback::Shared),
            AlsaOpen::SharedWithNotice
        );
        assert_eq!(exclusive_fallback_plan(ExclusiveFallback::WarnOnly), AlsaOpen::Fail);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-audio alsa`
Expected: compile error — `cannot find type Hint`, `device_list`, `f32_to_i32_one`, `AlsaOpen` — the failing state.

- [ ] **Step 3: Implement**

In `crates/oxitide-audio/src/alsa_output.rs`, extend the imports and append before `#[cfg(test)]`:

```rust
use crate::output::{ExclusiveFallback, OutputDevice};
```

```rust
/// A parsed ALSA PCM hint (from `snd_device_name_hint`), before filtering.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Hint {
    pub name: String,
    pub desc: Option<String>,
    pub ioid: Option<String>,
}

/// True when the hint is playback-capable. An absent IOID means both directions.
pub fn hint_is_output(hint: &Hint) -> bool {
    !matches!(hint.ioid.as_deref(), Some(ioid) if ioid.eq_ignore_ascii_case("Input"))
}

/// The picker surfaces the system default, sysdefault, and direct hardware
/// names; plugin aliases (surround*, null, dmix, …) are hidden to keep the
/// list meaningful.
pub fn hint_is_pickable(name: &str) -> bool {
    name == "default"
        || name == "sysdefault"
        || name.starts_with("hw:")
        || name.starts_with("plughw:")
}

fn hint_label(name: &str, desc: Option<&str>) -> String {
    desc.map(str::trim)
        .filter(|desc| !desc.is_empty())
        .unwrap_or(name)
        .to_string()
}

/// Collapse hints into the picker list: one entry per ALSA device name, the
/// default first, then the rest by id.
pub fn device_list(hints: &[Hint]) -> Vec<OutputDevice> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out: Vec<OutputDevice> = hints
        .iter()
        .filter(|hint| hint_is_output(hint) && hint_is_pickable(&hint.name))
        .filter(|hint| seen.insert(hint.name.clone()))
        .map(|hint| OutputDevice {
            id: hint.name.clone(),
            name: hint_label(&hint.name, hint.desc.as_deref()),
            is_default: hint.name == "default",
        })
        .collect();
    out.sort_by(|a, b| (a.id != "default", &a.id).cmp(&(b.id != "default", &b.id)));
    out
}

/// Left-aligned i32 conversion for one f32 sample; the float is clamped to
/// -1.0..=1.0 and rounded, because `as i32` truncation would bias downward.
pub fn f32_to_i32_one(sample: f32) -> i32 {
    if sample.is_nan() {
        return 0;
    }
    let scaled = (sample as f64).clamp(-1.0, 1.0) * 2_147_483_648.0;
    let rounded = scaled.round();
    if rounded >= 2_147_483_647.0 {
        i32::MAX
    } else if rounded <= -2_147_483_648.0 {
        i32::MIN
    } else {
        rounded as i32
    }
}

/// Map a raw ALSA mixer volume into 0.0..=1.0.
pub fn raw_to_volume(raw: i64, min: i64, max: i64) -> f32 {
    if max <= min {
        return 1.0;
    }
    ((raw - min) as f32 / (max - min) as f32).clamp(0.0, 1.0)
}

/// Map 0.0..=1.0 onto a raw ALSA mixer volume.
pub fn volume_to_raw(volume: f32, min: i64, max: i64) -> i64 {
    if max <= min {
        return max;
    }
    (min as f64 + (volume.clamp(0.0, 1.0) as f64) * (max - min) as f64).round() as i64
}

/// What a failed exclusive attempt does, mirroring WASAPI (`wasapi_output.rs:444`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlsaOpen {
    Exclusive,
    SharedWithNotice,
    Fail,
}

pub fn exclusive_fallback_plan(fallback: ExclusiveFallback) -> AlsaOpen {
    match fallback {
        ExclusiveFallback::Shared => AlsaOpen::SharedWithNotice,
        ExclusiveFallback::WarnOnly => AlsaOpen::Fail,
    }
}

/// Exact notice text used when software volume compromises bit-perfect output;
/// it matches WASAPI so the engine/UI behavior is identical on both platforms.
pub const SOFTWARE_VOLUME_NOTICE: &str = "volume applied in software; bit-perfect compromised";
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-audio alsa`
Expected: PASS (13 tests).
Run: `cargo clippy -p oxitide-audio -- -D warnings`
Expected: no warnings.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/alsa_output.rs
git commit -m "feat(audio): add pure ALSA device, volume, and fallback helpers"
```

---

### Task 3: Linux `AlsaOutput` — open, write, flush, stop

**Files:**
- Modify: `crates/oxitide-audio/Cargo.toml` (after the `[target.'cfg(windows)'.dependencies]` block)
- Modify: `crates/oxitide-audio/src/alsa_output.rs` (Linux section before `#[cfg(test)]`)

**Interfaces:**
- Consumes: `AlsaSampleFormat`, `alsa_exclusive_formats`, `pack_i32`, `f32_to_i32_one`, `exclusive_fallback_plan`, `SOFTWARE_VOLUME_NOTICE` (Tasks 1–2); `crate::decode::{PcmBuffer, StreamInfo}`; `crate::output::{AudioOutput, AudioOutputSpec, AudioOutputSpec fields, OutputMode, OutputStatus}`.
- Produces: `pub struct AlsaOutput` with `new()`; `pub fn enumerate() -> Result<Vec<OutputDevice>>`; Linux `impl AudioOutput for AlsaOutput`.

- [ ] **Step 1: Add the Linux dependency**

In `crates/oxitide-audio/Cargo.toml`, after the Windows target block:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
alsa = "0.9"
```

- [ ] **Step 2: Write the failing Linux tests**

Append inside the existing `mod tests` in `crates/oxitide-audio/src/alsa_output.rs`:

```rust
    #[cfg(target_os = "linux")]
    #[test]
    fn shared_output_lifecycle_on_the_null_device() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::F32(vec![0.5, -0.5, 0.25, -0.25])).unwrap();
        out.flush().unwrap();
        out.stop().unwrap();
        let status = out.status();
        assert_eq!(status.mode, OutputMode::Shared);
        assert!(!status.bit_perfect);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_output_on_the_null_device_opens_exclusive() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Exclusive,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.start().unwrap();
        out.write(&PcmBuffer::I32(vec![0x7fff_0000u32 as i32; 32])).unwrap();
        out.stop().unwrap();
        // The `null` device has no mixer, so Task 4's volume semantics will
        // clear bit_perfect here; this test pins the negotiated mode only.
        assert_eq!(out.status().mode, OutputMode::Exclusive);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_warn_only_errors_on_a_missing_device() {
        let mut out = AlsaOutput::new();
        let err = out
            .open(&AudioOutputSpec {
                device_id: Some("hw:99,99".into()),
                info: info(16, PcmFormat::I32 { bits: 16 }),
                mode: OutputMode::Exclusive,
                exclusive_fallback: ExclusiveFallback::WarnOnly,
            })
            .unwrap_err();
        assert!(matches!(err, crate::error::Error::Output(_)), "got {err:?}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn enumerate_reports_at_least_one_device() {
        let devices = enumerate().unwrap();
        assert!(!devices.is_empty());
    }
```

- [ ] **Step 3: Run tests to verify they fail (WSL2)**

In the WSL2 Ubuntu shell at the repo root:

```bash
cargo test -p oxitide-audio alsa
```

Expected: compile error — `cannot find type AlsaOutput`, `cannot find function enumerate` — the failing state. (On Windows this step is skipped: the tests and items are Linux-only. Do not run the Windows gate expecting these to pass there.)

- [ ] **Step 4: Implement**

Append to `crates/oxitide-audio/src/alsa_output.rs` before `#[cfg(test)]`:

```rust
#[cfg(target_os = "linux")]
mod linux {
    use alsa::device_name::HintIter;
    use alsa::pcm::{Access, Format, HwParams, PCM, ValueOr};
    use alsa::{Direction, Error as AlsaError};

    use super::*;
    use crate::decode::PcmBuffer;
    use crate::error::{Error, Result};
    use crate::output::{AudioOutput, AudioOutputSpec, OutputDevice, OutputMode, OutputStatus};

    pub struct AlsaOutput {
        pcm: Option<PCM>,
        format: AlsaSampleFormat,
        device: String,
        channels: u16,
        volume: f32,
        software_volume: bool,
        status: OutputStatus,
    }

    impl AlsaOutput {
        pub fn new() -> Self {
            Self {
                pcm: None,
                format: AlsaSampleFormat::S32Le,
                device: String::new(),
                channels: 2,
                volume: 1.0,
                software_volume: true,
                status: OutputStatus {
                    mode: OutputMode::Shared,
                    bit_perfect: false,
                    notice: None,
                },
            }
        }

        /// Open and fully configure a PCM; on any failure the handle is
        /// dropped and the error returned so the caller can try the next
        /// candidate format.
        fn open_pcm(&mut self, name: &str, format: AlsaSampleFormat, spec: &AudioOutputSpec) -> Result<()> {
            let pcm = PCM::new(name, Direction::Playback, false).map_err(to_error)?;
            {
                let hwp = HwParams::any(&pcm).map_err(to_error)?;
                hwp.set_channels(spec.info.channels as u32).map_err(to_error)?;
                hwp.set_rate(spec.info.sample_rate, ValueOr::Nearest).map_err(to_error)?;
                hwp.set_format(alsa_format(format)).map_err(to_error)?;
                hwp.set_access(Access::RWInterleaved).map_err(to_error)?;
                hwp.set_buffer_size_near((spec.info.sample_rate as i64 / 2).max(1024))
                    .map_err(to_error)?;
                hwp.set_periods(4, ValueOr::Nearest).map_err(to_error)?;
                pcm.hw_params(&hwp).map_err(to_error)?;
            }
            pcm.prepare().map_err(to_error)?;
            self.pcm = Some(pcm);
            self.format = format;
            self.device = name.to_string();
            self.channels = spec.info.channels;
            Ok(())
        }

        fn close(&mut self) {
            if let Some(pcm) = self.pcm.take() {
                let _ = pcm.drop();
            }
        }
    }

    fn alsa_format(format: AlsaSampleFormat) -> Format {
        match format {
            AlsaSampleFormat::S32Le => Format::S32LE,
            AlsaSampleFormat::S24_3Le => Format::S243LE,
            AlsaSampleFormat::S16Le => Format::S16LE,
        }
    }

    fn to_error(error: AlsaError) -> Error {
        Error::Output(error.to_string())
    }

    /// Enumerate output PCMs from ALSA's device hints.
    pub fn enumerate() -> Result<Vec<OutputDevice>> {
        let hints: Vec<Hint> = HintIter::new_str(None, "pcm")
            .map_err(to_error)?
            .map(|hint| Hint { name: hint.name.unwrap_or_default(), desc: hint.desc, ioid: hint.ioid })
            .collect();
        Ok(device_list(&hints))
    }

    impl AudioOutput for AlsaOutput {
        fn open(&mut self, spec: &AudioOutputSpec) -> Result<()> {
            self.close();
            self.software_volume = true;
            let requested = spec.mode;
            let device = match spec.device_id.as_deref() {
                Some(id) if !id.is_empty() => id.to_string(),
                _ if requested == OutputMode::Exclusive => "hw:0,0".to_string(),
                _ => "default".to_string(),
            };
            let mut status = OutputStatus {
                mode: OutputMode::Shared,
                bit_perfect: false,
                notice: None,
            };

            if requested == OutputMode::Exclusive {
                let opened = alsa_exclusive_formats(&spec.info)
                    .into_iter()
                    .find(|format| self.open_pcm(&device, *format, spec).is_ok());
                match opened {
                    Some(format) => {
                        self.format = format;
                        status.mode = OutputMode::Exclusive;
                        status.bit_perfect = true;
                    }
                    None => match exclusive_fallback_plan(spec.exclusive_fallback) {
                        AlsaOpen::SharedWithNotice => {
                            self.open_pcm("default", AlsaSampleFormat::S32Le, spec)?;
                            status.notice =
                                Some("exclusive mode unavailable; using shared output".into());
                        }
                        AlsaOpen::Fail | AlsaOpen::Exclusive => {
                            return Err(Error::Output(
                                "exclusive mode is not supported by this output device".into(),
                            ));
                        }
                    },
                }
            } else {
                self.open_pcm(&device, AlsaSampleFormat::S32Le, spec)?;
            }

            self.status = status;
            self.set_volume(self.volume)?;
            Ok(())
        }

        fn start(&mut self) -> Result<()> {
            match self.pcm.as_ref() {
                Some(pcm) => pcm.start().map_err(to_error),
                None => Ok(()),
            }
        }

        fn write(&mut self, buf: &PcmBuffer) -> Result<()> {
            let Some(pcm) = self.pcm.as_ref() else {
                return Err(Error::Output("output not open".into()));
            };
            let software = self.software_volume && (self.volume - 1.0).abs() >= f32::EPSILON;
            let samples: Vec<i32> = match buf {
                PcmBuffer::I32(v) => {
                    if software {
                        v.iter()
                            .map(|&sample| (sample as f64 * self.volume as f64) as i32)
                            .collect()
                    } else {
                        v.clone()
                    }
                }
                PcmBuffer::F32(v) => v
                    .iter()
                    .map(|&sample| {
                        let scaled = if software { sample * self.volume } else { sample };
                        f32_to_i32_one(scaled)
                    })
                    .collect(),
            };

            let mut bytes = Vec::with_capacity(samples.len() * self.format.store_bits() as usize / 8);
            pack_i32(self.format, &samples, &mut bytes);
            if bytes.is_empty() {
                return Ok(());
            }
            let frame_bytes =
                self.format.store_bits() as usize / 8 * self.channels as usize;
            let io = pcm.io_bytes();
            let mut offset = 0;
            while offset < bytes.len() {
                match io.writei(&bytes[offset..]) {
                    Ok(0) => return Err(Error::Output("alsa write made no progress".into())),
                    Ok(frames) => offset += frames * frame_bytes,
                    Err(error) => {
                        // xrun / suspend: recover and retry; unrecoverable
                        // errors are returned by `try_recover`.
                        pcm.try_recover(error, true).map_err(to_error)?;
                    }
                }
            }
            Ok(())
        }

        fn set_volume(&mut self, volume: f32) -> Result<()> {
            self.volume = volume.clamp(0.0, 1.0);
            Ok(())
        }

        fn flush(&mut self) -> Result<()> {
            // Discard ring and device buffers but keep the negotiated handle,
            // so a seek does not release and re-lock an exclusive DAC.
            if let Some(pcm) = self.pcm.as_ref() {
                pcm.drop().map_err(to_error)?;
                pcm.prepare().map_err(to_error)?;
            }
            Ok(())
        }

        fn stop(&mut self) -> Result<()> {
            self.close();
            Ok(())
        }

        fn status(&self) -> OutputStatus {
            self.status.clone()
        }
    }
}

#[cfg(target_os = "linux")]
pub use linux::{enumerate, AlsaOutput};
```

- [ ] **Step 5: Run tests to verify they pass (WSL2)**

In WSL2:

```bash
cargo test -p oxitide-audio alsa
```

Expected: PASS — pure tests plus the four Linux tests (`shared_output_lifecycle_on_the_null_device`, `exclusive_output_on_the_null_device_opens_exclusive`, `exclusive_warn_only_errors_on_a_missing_device`, `enumerate_reports_at_least_one_device`).

- [ ] **Step 6: Windows gate and commit**

On Windows:

```bash
cargo test -p oxitide-audio
cargo clippy --workspace -- -D warnings
```

Expected: PASS; the Linux items are cfg'd out and do not affect Windows.

```bash
git add crates/oxitide-audio/Cargo.toml crates/oxitide-audio/src/alsa_output.rs
git commit -m "feat(audio): add the Linux ALSA output backend"
```

---

### Task 4: Exclusive volume semantics and fallback notice

**Files:**
- Modify: `crates/oxitide-audio/src/alsa_output.rs`

**Interfaces:**
- Consumes: `raw_to_volume`, `volume_to_raw`, `SOFTWARE_VOLUME_NOTICE`, `exclusive_fallback_plan` (Task 2).
- Produces: `set_hardware_volume(device: &str, volume: f32) -> Result<()>`; the final `set_volume` semantics on `AlsaOutput`.

- [ ] **Step 1: Write the failing tests**

Append inside `mod tests` (Linux-gated where the item is Linux-only, plus one cross-platform constant assertion):

```rust
    #[test]
    fn software_volume_notice_constant_matches_wasapi() {
        assert_eq!(
            SOFTWARE_VOLUME_NOTICE,
            "volume applied in software; bit-perfect compromised"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn exclusive_volume_without_a_mixer_compromises_bit_perfect() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Exclusive,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.set_volume(0.5).unwrap();
        let status = out.status();
        assert!(!status.bit_perfect);
        assert_eq!(status.notice.as_deref(), Some(SOFTWARE_VOLUME_NOTICE));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn shared_volume_keeps_its_status_clean() {
        let mut out = AlsaOutput::new();
        out.open(&AudioOutputSpec {
            device_id: Some("null".into()),
            info: info(16, PcmFormat::I32 { bits: 16 }),
            mode: OutputMode::Shared,
            exclusive_fallback: ExclusiveFallback::Shared,
        })
        .unwrap();
        out.set_volume(0.5).unwrap();
        assert_eq!(out.status().notice, None);
        assert!(!out.status().bit_perfect);
    }
```

- [ ] **Step 2: Run tests to verify they fail (WSL2)**

In WSL2:

```bash
cargo test -p oxitide-audio alsa
```

Expected: FAIL — `exclusive_volume_without_a_mixer_compromises_bit_perfect` sees `bit_perfect = true` / `notice = None` because `set_volume` currently only stores the value. (The `null` device has no mixer, which is the point.)

- [ ] **Step 3: Implement**

Add the hardware-volume helper inside `mod linux`, above `impl AudioOutput for AlsaOutput`:

```rust
    /// Set the card's playback volume through ALSA's mixer, preferring
    /// "Master" then "PCM". Returns an error when no writable playback
    /// element exists, which sends the caller down the software path.
    pub fn set_hardware_volume(device: &str, volume: f32) -> Result<()> {
        use alsa::mixer::{Mixer, SelemId};
        let mixer = Mixer::new(device, false).map_err(to_error)?;
        for name in ["Master", "PCM"] {
            let id = SelemId::new(name, 0);
            let Some(selem) = mixer.find_selem(&id) else {
                continue;
            };
            if !selem.has_playback_volume() {
                continue;
            }
            let (min, max) = selem.get_playback_volume_range();
            selem
                .set_playback_volume_all(volume_to_raw(volume, min, max))
                .map_err(to_error)?;
            return Ok(());
        }
        Err(Error::Output("no writable playback volume control".into()))
    }
```

Replace the body of `set_volume` in `impl AudioOutput for AlsaOutput`:

```rust
        fn set_volume(&mut self, volume: f32) -> Result<()> {
            self.volume = volume.clamp(0.0, 1.0);
            let exclusive = self.status.mode == OutputMode::Exclusive;
            if exclusive {
                if set_hardware_volume(&self.device, self.volume).is_ok() {
                    self.software_volume = false;
                    if self.status.notice.as_deref() == Some(SOFTWARE_VOLUME_NOTICE) {
                        self.status.notice = None;
                        self.status.bit_perfect = true;
                    }
                    return Ok(());
                }
                self.software_volume = true;
                self.status.bit_perfect = false;
                self.status.notice = Some(SOFTWARE_VOLUME_NOTICE.into());
            } else {
                self.software_volume = true;
            }
            Ok(())
        }
```

- [ ] **Step 4: Run tests to verify they pass**

In WSL2:

```bash
cargo test -p oxitide-audio alsa
cargo clippy -p oxitide-audio -- -D warnings
```

Expected: PASS (20 tests on Linux: 13 pure + 1 cross-platform constant + 6 Linux-only) and no warnings.
On Windows:

```bash
cargo test -p oxitide-audio
cargo clippy --workspace -- -D warnings
```

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-audio/src/alsa_output.rs
git commit -m "feat(audio): mirror WASAPI volume semantics on ALSA"
```

---

### Task 5: Wire Linux enumeration and `real_output()`, smoke checklist

**Files:**
- Modify: `crates/oxitide-audio/src/output.rs:211-219`
- Modify: `crates/oxitide-app/src/player.rs:68-79`
- Create: `docs/superpowers/plans/notes/2026-10-03-track8-2-alsa-smoke.md`

**Interfaces:**
- Consumes: `crate::alsa_output::{AlsaOutput, enumerate}` (Task 3).
- Produces: Linux `enumerate_output_devices() -> Result<Vec<OutputDevice>>`; Linux arm of `real_output() -> Box<dyn AudioOutput>`.

- [ ] **Step 1: Write the failing test**

Append to `crates/oxitide-audio/src/output.rs`'s `mod tests` (Linux-gated; this pins that enumeration is really wired for Linux, not still the empty stub):

```rust
    #[cfg(target_os = "linux")]
    #[test]
    fn enumeration_is_wired_on_linux() {
        let devices = enumerate_output_devices().unwrap();
        assert!(!devices.is_empty());
    }
```

- [ ] **Step 2: Run test to verify it fails (WSL2)**

In WSL2:

```bash
cargo test -p oxitide-audio enumeration_is_wired_on_linux
```

Expected: FAIL — `assertion failed: !devices.is_empty()` because the current `#[cfg(not(windows))]` arm returns an empty vector.

- [ ] **Step 3: Implement**

In `crates/oxitide-audio/src/output.rs`, replace the non-Windows enumeration arm:

```rust
#[cfg(windows)]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    platform::enumerate()
}

#[cfg(target_os = "linux")]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    crate::alsa_output::enumerate()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn enumerate_output_devices() -> Result<Vec<OutputDevice>> {
    Ok(Vec::new())
}
```

In `crates/oxitide-app/src/player.rs`, extend `real_output()`:

```rust
/// The real output: a WASAPI device on Windows, an ALSA device on Linux,
/// silent `NullOutput` elsewhere (keeps the crate building on other dev
/// machines).
pub fn real_output() -> Box<dyn AudioOutput> {
    #[cfg(windows)]
    {
        Box::new(oxitide_audio::wasapi_output::WasapiOutput::new())
    }
    #[cfg(target_os = "linux")]
    {
        Box::new(oxitide_audio::alsa_output::AlsaOutput::new())
    }
    #[cfg(not(any(windows, target_os = "linux")))]
    {
        Box::new(oxitide_audio::output::NullOutput::new())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

In WSL2:

```bash
cargo test -p oxitide-audio
cargo test -p oxitide-app
cargo clippy --workspace -- -D warnings
```

Expected: PASS on Linux, including the wiring test and the existing engine tests.
On Windows:

```bash
cargo test --workspace
cargo clippy --workspace -- -D warnings
```

Expected: PASS (all Linux items cfg'd out).

- [ ] **Step 5: Create the smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-03-track8-2-alsa-smoke.md`:

```markdown
# Track 8.2 — ALSA smoke checklist (WSLg)

Human-in-the-loop checks for the Linux output backend. WSL2 with WSLg, repo
built per `docs/linux-build.md`. Check items off with the run date and notes.

## Shared output
- [ ] `default` appears in Settings → Audio → Output and is preselected.
- [ ] Play a 16/44.1 lossless track; audio is audible in WSLg.
- [ ] Play a lossy (AAC) track; audio is audible; status shows shared.
- [ ] Change the volume slider; loudness changes smoothly.

## Exclusive / bit-perfect (bare metal required)
- [ ] On a machine with a real card, select a `hw:CARD=…,DEV=…` device and
  Exclusive mode; status shows bit-perfect and the DAC locks to the track rate.
- [ ] Switch tracks of different sample rates; the DAC re-locks.
- [ ] Set Exclusive + WarnOnly with a busy device; playback pauses with the
  exclusive-unavailable notice.
- [ ] Set Exclusive + Shared; playback continues on `default` with the
  "exclusive mode unavailable" notice.
- [ ] Set volume below 100% with no writable mixer; the software-volume notice
  appears and bit-perfect clears.

## Robustness
- [ ] Unplug the selected USB DAC during playback; playback pauses/fails with a
  notice and recovers to `default` on the next play.
- [ ] Seek repeatedly during exclusive playback; no dropouts or hangs (flush
  path).
- [ ] Re-open Settings after a device list change; Refresh lists the new card.
```

- [ ] **Step 6: Commit**

```bash
git add crates/oxitide-audio/src/output.rs crates/oxitide-app/src/player.rs docs/superpowers/plans/notes/2026-10-03-track8-2-alsa-smoke.md
git commit -m "feat(audio): wire ALSA output and enumeration on Linux"
```

---

## Execution notes

- Tasks 1–2 run on Windows. Task 3 onward requires the WSL2 Ubuntu 22.04 environment created by Plan 8.1; if it is not ready, do Task 1–2 on Windows first, then set up WSL2 (`wsl --install -d Ubuntu-22.04`, apt deps incl. `libasound2-dev`) before Task 3.
- The `null` ALSA PCM is built into alsa-lib; it never reaches hardware and makes lifecycle tests deterministic.
- Do not modify `wasapi_output.rs`, `endpoint_volume.rs`, or any Windows-only module.
