# Linux Perf Hardening (Track 8.5) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement Linux `/proc` process metrics in `perf.rs` behind `#[cfg(target_os = "linux")]` (pure parsers unit-tested on Windows), document the Windows-only scripted perf-harness exemption in a note plus `AGENTS.md`, and assemble Track 8's WSLg and deferred bare-metal smoke checklists.

**Architecture:** Add module-private pure parsers (`&str` in, value out) compiled under `cfg(any(target_os = "linux", test))`, then a thin Linux I/O layer behind `cfg(target_os = "linux")` that reads `/proc/self/stat`, `/proc/<pid>/task/*/children`, `smaps_rollup`, `statm`, and `comm` (anchoring `starttime` with `CLOCK_BOOTTIME`), reusing the existing 2 s memory-sample thread, `PerfSession` JSONL sink, `check.mjs`, and `budgets.json` unchanged. Windows implementations are untouched; a docs task records the T8-D3 exemption and updates `AGENTS.md`; a final task writes the two smoke checklists and runs every gate.

**Tech Stack:** Rust (Tauri 2 binary crate, `cfg`-gated `libc`), Node (`scripts/perf/check.mjs`), PowerShell runner (`scripts/perf/run-scenario.ps1`), Markdown notes.

**Spec:** `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md`

## Global Constraints

- Windows-first: every new runtime path is `#[cfg(target_os = "linux")]`; the existing Windows functions and their behavior stay byte-for-byte unchanged; Windows gates must stay green after every task.
- Windows gates (manual; no CI): `cargo test --workspace`, `cargo clippy --workspace -- -D warnings`, `npm --prefix ui run check`, `npm --prefix ui test`.
- WSL2/Linux gates (spec §5.1): the same cargo gates plus `cargo build -p oxitide-app`, run inside the Plan 8.1 environment documented in `docs/linux-build.md` — they are what compiles and lints the `cfg(target_os = "linux")` code.
- Parsing is pure (`&str` in) and unit-tested on Windows with fixture strings; the Linux `cfg` code only reads files and calls the parsers (spec §4.8).
- `libc` is added only under `[target.'cfg(target_os = "linux")'.dependencies]`; no new Windows dependency, no UI changes, no new background work (performance is a standing PM-D1 consideration; sampling stays on the existing 2 s thread).
- The scripted S1–S10 WebView2 CDP scenarios stay Windows-only (T8-D3) and are documented, never weakened or silently dropped.
- No token logging; keyring/`MemoryStore` rules are unaffected by this slice.
- Plan 8.5 executes on the `track8-linux` branch/worktree (spec §6 execution hygiene). The WSL2 steps assume the Plan 8.1 environment already exists.

## Review Focus

- **Malformed `/proc` lines.** A truncated `/proc/<pid>/stat`, a `smaps_rollup` without `Rss:`, or a junk `statm` field must yield `None` (process skipped) — never a panic or a bogus measurement. Pinned by `stat_starttime_rejects_malformed_lines`, `smaps_rollup_rss_is_parsed_from_its_line`, and `statm_resident_pages_are_parsed_and_zero_marks_a_zombie` in Task 1.
- **`comm` containing spaces/parens.** `/proc/<pid>/stat` must still yield field 22 (`starttime`), not a field shifted by a `comm` like `my (weird) app`. Pinned by `stat_starttime_survives_comm_with_spaces_and_parens` in Task 1.
- **`/proc` permission limits or older kernels.** When `smaps_rollup` is unreadable/missing (ptrace limits, kernel < 4.14), the walker must fall back to `statm` resident pages × page size; when both are unreadable the process is skipped and the tree is still positive for the app itself. Pinned by `memory_kb_prefers_smaps_and_falls_back_to_statm` in Task 1 and `process_tree_memory_is_positive` on Linux in Task 2.
- **Zombie/exited PIDs.** A zombie's zero resident pages must produce no row, and a PID that exits mid-walk must not break or poison the sample. Pinned by `statm_resident_pages_are_parsed_and_zero_marks_a_zombie` and `zero_resident_kb_never_becomes_a_row` in Task 1.
- **Memory double-count across task dirs.** A PID listed by more than one `/proc/<pid>/task/*/children` file (or already sampled) must be queued/sampled exactly once, so the aggregate never double-counts. Pinned by `task_children_parsing_and_dedup_across_tasks` in Task 1 and `memory_samples_name_each_process_and_flag_the_app` on Linux in Task 2.

---

## File Structure

**Rust**

- Modify `crates/oxitide-app/src/perf.rs` — Task 1 adds the pure parsers + tests after `now_unix_ms`; Task 2 adds the Linux I/O layer, narrows the `not(windows)` stubs to `not(any(windows, target_os = "linux"))`, gates the Toolhelp-only helpers, and extends the Linux coverage of the existing tests.
- Modify `crates/oxitide-app/Cargo.toml` — Task 2 adds `[target.'cfg(target_os = "linux")'.dependencies] libc = "0.2"`.
- Modify `Cargo.lock` — Task 2; the lock already contains `libc 0.2.189`, only `oxitide-app`'s dependency list changes.

**Docs**

- Create `docs/superpowers/plans/notes/2026-10-03-track8-perf-exemption.md` — Task 3 (no `scripts/perf/README*` exists; checked 2026-10-03).
- Modify `AGENTS.md` — Task 3 (exactly two insertions: Commands, Performance harness).
- Create `docs/superpowers/plans/notes/2026-10-03-track8-wslg-smoke.md` — Task 4.
- Create `docs/superpowers/plans/notes/2026-10-03-track8-bare-metal-checklist.md` — Task 4.

No UI files are touched.

---

### Task 1: Pure `/proc` parsers and their unit tests

**Files:**
- Modify: `crates/oxitide-app/src/perf.rs` (parsers after `now_unix_ms`, currently lines 320-325; tests inside the existing `mod tests`, after `marks_are_process_relative_and_share_the_unix_base`)

**Interfaces:**
- Consumes: nothing.
- Produces (module-private, each compiled under `#[cfg(any(target_os = "linux", test))]` so the tests run on Windows and the helpers cannot trip Windows `dead_code`):
  - `fn parse_stat_starttime(stat: &str) -> Option<u64>`
  - `fn creation_unix_ms(now_unix_ms: u128, boot_seconds: f64, starttime_ticks: u64, ticks_per_sec: u64) -> Option<u128>`
  - `fn parse_smaps_rollup_rss_kb(rollup: &str) -> Option<u64>`
  - `fn parse_statm_resident_pages(statm: &str) -> Option<u64>`
  - `fn memory_kb_from_proc(smaps_rollup: Option<&str>, statm: Option<&str>, page_size: u64) -> Option<u64>`
  - `fn resident_kb_to_mb(kb: u64) -> Option<u64>`
  - `fn parse_children(children: &str) -> Vec<u32>`
  - `fn new_child_pids(sampled: &std::collections::BTreeSet<u32>, task_children: &[Vec<u32>]) -> Vec<u32>`
  - `fn parse_comm(comm: &str) -> Option<String>`

- [ ] **Step 1: Write the failing tests**

In `crates/oxitide-app/src/perf.rs`, inside `mod tests`, insert immediately after the `marks_are_process_relative_and_share_the_unix_base` test (just before the `#[cfg(windows)]` `process_tree_memory_is_positive` test):

```rust
    #[test]
    fn stat_starttime_survives_comm_with_spaces_and_parens() {
        let stat = "123 (my (weird) app) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 987654";
        assert_eq!(parse_stat_starttime(stat), Some(987654));
    }

    #[test]
    fn stat_starttime_rejects_malformed_lines() {
        assert_eq!(parse_stat_starttime(""), None);
        assert_eq!(parse_stat_starttime("123 (no closing paren S 1 2"), None);
        assert_eq!(parse_stat_starttime("123 (proc) S 1 2 3"), None);
        assert_eq!(
            parse_stat_starttime("123 (proc) S 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16 17 18 x"),
            None
        );
    }

    #[test]
    fn creation_time_anchors_starttime_to_the_wall_clock() {
        let now = 1_700_000_000_000u128;
        let created = creation_unix_ms(now, 10_000.0, 12_345, 100).unwrap();
        assert_eq!(created, now - 10_000_000 + 123_450);
        assert_eq!(creation_unix_ms(now, 10.0, 1, 0), None);
    }

    #[test]
    fn smaps_rollup_rss_is_parsed_from_its_line() {
        let rollup = "Rss:   12345 kB\nPss:  2345 kB\nShared_Clean: 100 kB\n";
        assert_eq!(parse_smaps_rollup_rss_kb(rollup), Some(12345));
        assert_eq!(parse_smaps_rollup_rss_kb("Pss: 1 kB\n"), None);
        assert_eq!(parse_smaps_rollup_rss_kb("Rss: not-a-number kB\n"), None);
    }

    #[test]
    fn statm_resident_pages_are_parsed_and_zero_marks_a_zombie() {
        assert_eq!(parse_statm_resident_pages("12345 678 90 0 0 0 0"), Some(678));
        assert_eq!(parse_statm_resident_pages("0 0 0 0 0 0 0"), Some(0));
        assert_eq!(parse_statm_resident_pages("12345"), None);
        assert_eq!(parse_statm_resident_pages("12345 pages"), None);
    }

    #[test]
    fn memory_kb_prefers_smaps_and_falls_back_to_statm() {
        let smaps = "Rss: 100 kB\n";
        let statm = "1000 200 0 0 0 0 0";
        assert_eq!(memory_kb_from_proc(Some(smaps), Some(statm), 4096), Some(100));
        assert_eq!(memory_kb_from_proc(None, Some(statm), 4096), Some(800));
        assert_eq!(memory_kb_from_proc(Some("Pss: 1 kB\n"), Some(statm), 4096), Some(800));
        assert_eq!(memory_kb_from_proc(None, None, 4096), None);
        assert_eq!(memory_kb_from_proc(None, Some(statm), 0), None);
    }

    #[test]
    fn zero_resident_kb_never_becomes_a_row() {
        assert_eq!(resident_kb_to_mb(0), None);
        assert_eq!(resident_kb_to_mb(1023), None);
        assert_eq!(resident_kb_to_mb(1024), Some(1));
    }

    #[test]
    fn task_children_parsing_and_dedup_across_tasks() {
        assert_eq!(parse_children("101 202\n"), vec![101, 202]);
        assert_eq!(parse_children("101 junk 202"), vec![101, 202]);
        assert_eq!(parse_children("   \n"), Vec::<u32>::new());
        let sampled = std::collections::BTreeSet::from([10u32]);
        let fresh = new_child_pids(&sampled, &[vec![20, 30], vec![30, 10, 40]]);
        assert_eq!(fresh, vec![20, 30, 40], "a pid must be queued at most once");
    }

    #[test]
    fn comm_is_trimmed_and_empty_names_are_dropped() {
        assert_eq!(parse_comm("oxitide-app\n"), Some("oxitide-app".to_string()));
        assert_eq!(parse_comm("  my app  \n"), Some("my app".to_string()));
        assert_eq!(parse_comm(" \n"), None);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p oxitide-app perf::`
Expected: compile error — `cannot find function parse_stat_starttime`, `cannot find function memory_kb_from_proc`, etc. That is the failing state (Task 1 deliberately lands before the helpers).

- [ ] **Step 3: Implement the pure parsers**

In `crates/oxitide-app/src/perf.rs`, insert immediately after `now_unix_ms()` (currently ends at line 325) and before the `/// Windows: ...` comment for `process_creation_unix_ms`:

```rust
/// Pure `/proc` parsing for the Linux process metrics. Compiled on every
/// platform so the unit tests run under the Windows gates; the
/// `#[cfg(target_os = "linux")]` code below only reads files and calls them.
#[cfg(any(target_os = "linux", test))]
fn parse_stat_starttime(stat: &str) -> Option<u64> {
    // `comm` (field 2) may contain spaces and parentheses, so field splitting
    // starts after the final `)`.
    let after_comm = stat.get(stat.rfind(')')? + 1..)?;
    after_comm.split_whitespace().nth(19)?.parse().ok()
}

/// Anchor a tick-based starttime to the wall clock:
/// `creation = now - boot_seconds + starttime / ticks_per_sec`.
#[cfg(any(target_os = "linux", test))]
fn creation_unix_ms(
    now_unix_ms: u128,
    boot_seconds: f64,
    starttime_ticks: u64,
    ticks_per_sec: u64,
) -> Option<u128> {
    if ticks_per_sec == 0 {
        return None;
    }
    let boot_ms = (boot_seconds * 1000.0) as u128;
    let start_ms = (starttime_ticks as u128) * 1000 / (ticks_per_sec as u128);
    Some(now_unix_ms.saturating_sub(boot_ms).saturating_add(start_ms))
}

/// `Rss:` (kB) from a `/proc/<pid>/smaps_rollup` body.
#[cfg(any(target_os = "linux", test))]
fn parse_smaps_rollup_rss_kb(rollup: &str) -> Option<u64> {
    rollup.lines().find_map(|line| {
        let value = line.strip_prefix("Rss:")?;
        value.split_whitespace().next()?.parse().ok()
    })
}

/// Second field (resident pages) of a `/proc/<pid>/statm` body.
#[cfg(any(target_os = "linux", test))]
fn parse_statm_resident_pages(statm: &str) -> Option<u64> {
    statm.split_whitespace().nth(1)?.parse().ok()
}

/// Resident kB from the two `/proc` sources: `smaps_rollup`'s `Rss:` when
/// readable, else `statm` pages scaled by `page_size`.
#[cfg(any(target_os = "linux", test))]
fn memory_kb_from_proc(
    smaps_rollup: Option<&str>,
    statm: Option<&str>,
    page_size: u64,
) -> Option<u64> {
    if let Some(kb) = smaps_rollup.and_then(parse_smaps_rollup_rss_kb) {
        return Some(kb);
    }
    let pages = statm.and_then(parse_statm_resident_pages)?;
    if page_size == 0 {
        return None;
    }
    Some(pages.saturating_mul(page_size) / 1024)
}

/// kB -> MiB for one process; `None` for zero resident memory (a zombie or a
/// process that exited mid-sample), so no empty row is emitted.
#[cfg(any(target_os = "linux", test))]
fn resident_kb_to_mb(kb: u64) -> Option<u64> {
    let mb = kb / 1024;
    (mb > 0).then_some(mb)
}

/// Space-separated child PIDs from `/proc/<pid>/task/<tid>/children`.
#[cfg(any(target_os = "linux", test))]
fn parse_children(children: &str) -> Vec<u32> {
    children
        .split_whitespace()
        .filter_map(|pid| pid.parse().ok())
        .collect()
}

/// Child PIDs not yet sampled, in first-seen order; a PID appears once even
/// when several task `children` files list it.
#[cfg(any(target_os = "linux", test))]
fn new_child_pids(
    sampled: &std::collections::BTreeSet<u32>,
    task_children: &[Vec<u32>],
) -> Vec<u32> {
    let mut seen = sampled.clone();
    let mut fresh = Vec::new();
    for list in task_children {
        for &pid in list {
            if seen.insert(pid) {
                fresh.push(pid);
            }
        }
    }
    fresh
}

/// `/proc/<pid>/comm` content, trimmed; `None` when empty.
#[cfg(any(target_os = "linux", test))]
fn parse_comm(comm: &str) -> Option<String> {
    let trimmed = comm.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p oxitide-app perf::`
Expected: PASS — all existing perf tests plus the 9 new parser tests.

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings (the helpers are absent from the Windows non-test build, so no `dead_code`).

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/perf.rs
git commit -m "feat(perf): add pure /proc parsers for Linux process metrics"
```

---

### Task 2: Linux creation-time and process-tree memory behind `cfg(target_os = "linux")`

**Files:**
- Modify: `crates/oxitide-app/Cargo.toml` (add the Linux target section)
- Modify: `crates/oxitide-app/src/perf.rs` (Linux I/O layer; narrow the two `not(windows)` stubs; delete the unused `creation_ms_for_pid` stub; gate the Toolhelp-only helpers; extend the two Windows tests to Linux; add `linux_process_creation_is_recent`)
- Modify: `Cargo.lock` (libc moves into `oxitide-app`'s dependency list)

**Interfaces:**
- Consumes: Task 1 parsers; `now_unix_ms`; the existing call sites — `PerfSession::create_in` already calls `process_creation_unix_ms()`, and `write_memory_sample`/`snapshot` already call `process_tree_memory_mb()` → `process_memory_rows()`.
- Produces: `#[cfg(target_os = "linux")] fn process_creation_unix_ms() -> Option<u128>`; `#[cfg(target_os = "linux")] fn process_memory_rows() -> Vec<(String, u64, bool)>` plus private helpers `clock_ticks_per_sec`, `boottime_seconds`, `page_size_bytes`, `read_task_children`, `read_process_name`, `read_process_memory_mb`; no public API change, no JSONL schema change.

- [ ] **Step 1: Extend the Linux test coverage (failing on Linux, green on Windows)**

In `crates/oxitide-app/src/perf.rs`, in `mod tests`, replace:

```rust
    #[cfg(windows)]
    #[test]
    fn process_tree_memory_is_positive() {
        let mb = process_tree_memory_mb().expect("measurement available on Windows");
        assert!(mb > 0, "expected a positive working set, got {mb}");
    }
```

with:

```rust
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn process_tree_memory_is_positive() {
        let mb = process_tree_memory_mb().expect("measurement available on Linux and Windows");
        assert!(mb > 0, "expected a positive working set, got {mb}");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_process_creation_is_recent() {
        let created = process_creation_unix_ms().expect("creation time from /proc");
        let now = now_unix_ms();
        assert!(created <= now, "creation {created} must not be in the future");
        assert!(
            now - created < 24 * 60 * 60 * 1000,
            "creation {created} must be less than a day old"
        );
    }
```

and change the `memory_samples_name_each_process_and_flag_the_app` attribute from:

```rust
    #[cfg(windows)]
    #[test]
    fn memory_samples_name_each_process_and_flag_the_app() {
```

to:

```rust
    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn memory_samples_name_each_process_and_flag_the_app() {
```

- [ ] **Step 2: Run the tests on both platforms to verify the Linux failure**

Run (Windows): `cargo test -p oxitide-app perf::`
Expected: PASS — the widened tests use the untouched Windows implementation; `linux_process_creation_is_recent` does not exist on Windows.

Run (WSL2 Ubuntu 22.04, from the repo checkout; see `docs/linux-build.md` if the environment is not set up): `cargo test -p oxitide-app perf::`
Expected: FAIL — `process_tree_memory_is_positive` panics with `measurement available on Linux and Windows` and `linux_process_creation_is_recent` panics with `creation time from /proc` (the current `cfg(not(windows))` stubs return `None`/`Vec::new()`). This is the TDD baseline.

- [ ] **Step 3: Implement the Linux layer**

In `crates/oxitide-app/Cargo.toml`, insert before `[target.'cfg(windows)'.dependencies]`:

```toml
[target.'cfg(target_os = "linux")'.dependencies]
libc = "0.2"
```

In `crates/oxitide-app/src/perf.rs`, replace:

```rust
#[cfg(not(windows))]
fn process_creation_unix_ms() -> Option<u128> {
    None
}
```

with:

```rust
/// Linux: unix milliseconds of process creation, from `/proc/self/stat`
/// `starttime` anchored to the wall clock by `CLOCK_BOOTTIME` (nanosecond
/// precision, so back-to-back reads cancel exactly).
#[cfg(target_os = "linux")]
fn process_creation_unix_ms() -> Option<u128> {
    let stat = std::fs::read_to_string("/proc/self/stat").ok()?;
    let starttime = parse_stat_starttime(&stat)?;
    let ticks_per_sec = clock_ticks_per_sec()?;
    creation_unix_ms(now_unix_ms(), boottime_seconds()?, starttime, ticks_per_sec)
}

/// Seconds since boot from `CLOCK_BOOTTIME`.
#[cfg(target_os = "linux")]
fn boottime_seconds() -> Option<f64> {
    let mut ts = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let rc = unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) };
    if rc != 0 {
        return None;
    }
    Some(ts.tv_sec as f64 + ts.tv_nsec as f64 / 1_000_000_000.0)
}

#[cfg(not(any(windows, target_os = "linux")))]
fn process_creation_unix_ms() -> Option<u128> {
    None
}
```

Then gate the three Toolhelp-only helpers (Windows non-test still compiles them; the Linux test build keeps them for their unit tests, and Linux non-test builds no longer see them). Replace:

```rust
/// One process from the Toolhelp snapshot: `(pid, parent pid, name, creation
/// time in unix ms when readable)`.
type ProcessRow = (u32, u32, String, Option<u64>);
```

with:

```rust
/// One process from the Toolhelp snapshot: `(pid, parent pid, name, creation
/// time in unix ms when readable)`.
#[cfg(any(windows, test))]
type ProcessRow = (u32, u32, String, Option<u64>);
```

add `#[cfg(any(windows, test))]` immediately above `fn tracked_process_name(name: &str) -> bool {` and above `fn is_descendant(processes: &[ProcessRow], self_pid: u32, pid: u32) -> bool {` (their doc comments stay in place above the attribute).

Delete the now-unused non-Windows stub:

```rust
#[cfg(not(windows))]
fn creation_ms_for_pid(_pid: u32) -> Option<u64> {
    None
}
```

Finally, replace:

```rust
#[cfg(not(windows))]
fn process_memory_rows() -> Vec<(String, u64, bool)> {
    Vec::new()
}
```

with:

```rust
/// Linux clock ticks per second; `/proc` starttime is measured in ticks.
#[cfg(target_os = "linux")]
fn clock_ticks_per_sec() -> Option<u64> {
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    (ticks > 0).then_some(ticks as u64)
}

/// Linux page size for the `statm` fallback.
#[cfg(target_os = "linux")]
fn page_size_bytes() -> Option<u64> {
    let page_size = unsafe { libc::sysconf(libc::_SC_PAGESIZE) };
    (page_size > 0).then_some(page_size as u64)
}

/// One `/proc/<pid>/task/<tid>/children` list per thread of `pid`.
#[cfg(target_os = "linux")]
fn read_task_children(pid: u32) -> Vec<Vec<u32>> {
    let Ok(tasks) = std::fs::read_dir(format!("/proc/{pid}/task")) else {
        return Vec::new();
    };
    tasks
        .flatten()
        .filter_map(|entry| std::fs::read_to_string(entry.path().join("children")).ok())
        .map(|text| parse_children(&text))
        .collect()
}

/// `/proc/<pid>/comm`, `None` when the process exited or is unreadable.
#[cfg(target_os = "linux")]
fn read_process_name(pid: u32) -> Option<String> {
    parse_comm(&std::fs::read_to_string(format!("/proc/{pid}/comm")).ok()?)
}

/// Resident memory in MiB; `None` when the process exited (zombie) or both
/// `/proc` sources are unreadable.
#[cfg(target_os = "linux")]
fn read_process_memory_mb(pid: u32) -> Option<u64> {
    let smaps_rollup = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).ok();
    let statm = std::fs::read_to_string(format!("/proc/{pid}/statm")).ok();
    let page_size = page_size_bytes().unwrap_or(0);
    let kb = memory_kb_from_proc(smaps_rollup.as_deref(), statm.as_deref(), page_size)?;
    resident_kb_to_mb(kb)
}

/// Linux: resident memory of this process and every descendant, by walking
/// `/proc/<pid>/task/*/children`; one row per process: (name, mb, is_self).
#[cfg(target_os = "linux")]
fn process_memory_rows() -> Vec<(String, u64, bool)> {
    use std::collections::{BTreeSet, VecDeque};

    let self_pid = std::process::id();
    let mut sampled = BTreeSet::new();
    let mut queue = VecDeque::from([self_pid]);
    let mut rows = Vec::new();
    while let Some(pid) = queue.pop_front() {
        if !sampled.insert(pid) {
            continue;
        }
        for child in new_child_pids(&sampled, &read_task_children(pid)) {
            queue.push_back(child);
        }
        let Some(name) = read_process_name(pid) else {
            continue;
        };
        let Some(mb) = read_process_memory_mb(pid) else {
            continue;
        };
        rows.push((name, mb, pid == self_pid));
    }
    rows
}

#[cfg(not(any(windows, target_os = "linux")))]
fn process_memory_rows() -> Vec<(String, u64, bool)> {
    Vec::new()
}
```

- [ ] **Step 4: Run the gates on both platforms to verify they pass**

Run (Windows): `cargo test --workspace`
Expected: all pass.

Run (Windows): `cargo clippy --workspace -- -D warnings`
Expected: no warnings (Windows implementations unchanged).

Run (WSL2 Ubuntu 22.04): `cargo test -p oxitide-app perf::`
Expected: PASS — including `process_tree_memory_is_positive` (self RSS > 0), `linux_process_creation_is_recent`, and `memory_samples_name_each_process_and_flag_the_app` (self row named and flagged, aggregate = row sum).

Run (WSL2 Ubuntu 22.04): `cargo clippy --workspace -- -D warnings`
Expected: no warnings — the gated helpers and the Linux functions leave no dead code on either platform.

- [ ] **Step 5: Commit**

```bash
git add crates/oxitide-app/src/perf.rs crates/oxitide-app/Cargo.toml Cargo.lock
git commit -m "feat(perf): measure Linux process creation and tree memory from /proc"
```

---

### Task 3: Perf-harness exemption note and `AGENTS.md` pointers

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-03-track8-perf-exemption.md`
- Modify: `AGENTS.md` (Commands and Performance harness sections only)

**Interfaces:**
- Consumes: the Linux metrics from Task 2; the Track 3 harness facts in `scripts/perf/` and `docs/superpowers/plans/notes/2026-10-02-track3-phase-d.md`.
- Produces: the documented exemption linked from `AGENTS.md`; no code change.

- [ ] **Step 1: Confirm the exemption has no harness README to live in**

Run: `Get-ChildItem scripts/perf -Filter "README*"`
Expected: no output (no `scripts/perf/README*` exists). If a README has appeared by execution time, put the content from Step 2 in that README instead of creating the note, and point both `AGENTS.md` links at the README.

- [ ] **Step 2: Create the exemption note**

Create `docs/superpowers/plans/notes/2026-10-03-track8-perf-exemption.md`:

```markdown
# Track 8 perf-harness exemption (2026-10-03)

Authority: `docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md`
§4.8 and T8-D3. Implemented by Plan 8.5. There is no `scripts/perf/README*`
in the repo (checked 2026-10-03), so the exemption is recorded here and
linked from `AGENTS.md`.

## What stays Windows-only

`scripts/perf/run-scenario.ps1` and `scripts/perf/drive.mjs` drive the app
through the **WebView2 CDP endpoint**: the runner sets
`WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS=--remote-debugging-port=<port>`, and
the driver resolves `/json`, connects with a WebSocket, and drives
`Runtime.evaluate` against `window.__perf` / Tauri `invoke`. They are tied to
the Windows WebView2 runtime and the `.exe`, so the scripted S1–S10 scenarios
are **not ported** in Track 8 (spec §9 records the full
WebKitWebDriver/tauri-driver port as deferred).

## What Linux gets instead

`crates/oxitide-app/src/perf.rs` implements the process metrics for
`#[cfg(target_os = "linux")]` from `/proc`:

- process creation time: `/proc/self/stat` `starttime` (parsed after the last
  `)`, so `comm` with spaces/parens survives) anchored to the wall clock with
  `CLOCK_BOOTTIME`;
- tree memory: walk `/proc/<pid>/task/*/children`, then read each PID's
  `smaps_rollup` `Rss:` (fallback: `statm` resident pages × page size).

Launching the Linux binary with `OXITIDE_PERF_SCENARIO=<id>` (and optionally
`OXITIDE_PERF_DIR=<dir>`) writes the same JSONL schema as Windows, and
`node scripts/perf/check.mjs --dir <dir>` aggregates it against
`scripts/perf/budgets.json`.

## Budget coverage on Linux

Measurable now: process-relative `t_ms` marks (`first_paint_ms`,
`ui_ready_ms`, `startup_step_max_ms`), memory rows (`memory_max_mb`,
`memory_drift_mb`, `memory_drift_app_mb`), and playback-driven audio rows
(`ttfs_ms`, `advance_max_gap_ms`, `cache_hits`/`cache_misses`) when a human
drives the app as S4/S8/S9/S10 always required.

Not measurable until the deferred harness port: the scripted scenario runs
themselves and the `ui_metrics` rows they produce (`frame_p95/p99_ms`,
`long_task_max_ms`, S5/S6 interaction rows, S7 `queue_build_ms`, S8's
scripted loop). `check.mjs` marks `required` rows as failing when a corpus
lacks them, so a Linux corpus is read under this exemption; the Windows
corpus remains the gate for interaction rows. The Windows Track 3 budgets and
the documented S4 finding are unchanged
(`docs/superpowers/plans/notes/2026-10-02-track3-phase-d.md`).
```

- [ ] **Step 3: Update `AGENTS.md` (exactly two insertions)**

Insertion 1 — in the Commands section, immediately after the UI line:

Before:

```markdown
- UI: `npm --prefix ui run check` (svelte-check); `npm --prefix ui test` (vitest). Single test: `npm --prefix ui test -- src/lib/format.test.ts`.
```

After:

```markdown
- UI: `npm --prefix ui run check` (svelte-check); `npm --prefix ui test` (vitest). Single test: `npm --prefix ui test -- src/lib/format.test.ts`.
- Linux (WSL2 Ubuntu 22.04): environment, apt prerequisites, and build/package commands live in `docs/linux-build.md`; run the same `cargo test --workspace` and `cargo clippy --workspace -- -D warnings` gates there — they compile the `cfg(target_os = "linux")` code.
```

Insertion 2 — in the Performance harness section, immediately after the intro line and before the runner bullet:

Before:

```markdown
Track 3's measurement gate lives in `scripts/perf/` (manual; no CI):
```

After:

```markdown
Track 3's measurement gate lives in `scripts/perf/` (manual; no CI):

- Windows-only exemption (Track 8, T8-D3): the scripted runner/driver (`run-scenario.ps1`, `drive.mjs`) speaks the WebView2 CDP endpoint and is not ported to WebKitGTK. On Linux the `/proc` metrics in `crates/oxitide-app/src/perf.rs` keep process-relative marks, memory, and playback rows measurable via `OXITIDE_PERF_SCENARIO` + `node scripts/perf/check.mjs`; frame/interaction budgets stay unmeasured until the deferred harness port. Authority: `docs/superpowers/plans/notes/2026-10-03-track8-perf-exemption.md`.
```

- [ ] **Step 4: Verify the `AGENTS.md` diff is exactly the two insertions**

Run: `git diff -- AGENTS.md`
Expected: one added `- Linux (WSL2 Ubuntu 22.04):` bullet under Commands and one added `- Windows-only exemption (Track 8, T8-D3):` bullet under Performance harness; no other lines changed.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-03-track8-perf-exemption.md AGENTS.md
git commit -m "docs: record the Linux perf-harness exemption"
```

---

### Task 4: Track 8 smoke checklists and full gates

**Files:**
- Create: `docs/superpowers/plans/notes/2026-10-03-track8-wslg-smoke.md`
- Create: `docs/superpowers/plans/notes/2026-10-03-track8-bare-metal-checklist.md`

**Interfaces:**
- Consumes: every task above; spec §5.3–§5.4; the Plan 8.1–8.4 artifacts the checklists reference.
- Produces: the two human checklists (WSLg-verifiable items; deferred bare-metal items + completion-gate protocol); no code change.

- [ ] **Step 1: Create the WSLg smoke checklist**

Create `docs/superpowers/plans/notes/2026-10-03-track8-wslg-smoke.md`:

```markdown
# Track 8 WSLg smoke checklist (2026-10-03)

Human-in-the-loop checks for the WSL2-verifiable half of Track 8, per
`docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` §5.3.
Run inside the WSL2 Ubuntu 22.04 environment built by Plan 8.1
(`docs/linux-build.md`), with WSLg for GUI windows and shared audio. These
items may complete in WSL2 (spec §5.4); the bare-metal items live in
`2026-10-03-track8-bare-metal-checklist.md`.

Record each item as PASS/FAIL with date, distro/kernel, and evidence
(screenshot, command output, log excerpt). A FAIL becomes an issue; do not
silently skip an item.

## Shell, login, and keyring

- [ ] **GUI launch (8.1–8.3).** Start Vite (`npm --prefix ui run dev`) and
      launch the debug binary from the repo root under WSLg; the OxiTide
      window renders and the terminal shows no GTK/WebKit init errors.
- [ ] **Login flow (8.1).** With no Secret Service running, Settings/Account
      and the login view show the session-only notice; browser PKCE login
      (WSLg `xdg-open`) + paste-back completes and the shell loads home.
- [ ] **Session-only keyring notice (8.1).** Quit and relaunch → still logged
      out (tokens were memory-only) and the notice explains why. Start
      `gnome-keyring-daemon` (or another Secret Service), log in again,
      relaunch → still logged in.
- [ ] **Shared audio (8.2).** Play a track; WSLg routes through PulseAudio
      (`PULSE_SERVER`), audio is audible on the Windows host, position
      advances, and pause/resume/seek/next work. WSLg exposes no `hw:` PCM:
      expect Shared or `ExclusiveFallback` notices, never a crash.
- [ ] **MPRIS via playerctl (8.4).** With a session D-Bus,
      `playerctl -l` lists the player; `status`, `metadata`, `play-pause`,
      `next`, `previous`, and `stop` mirror the app, and `Raise` focuses the
      window. (Check once Plan 8.4 has landed.)

## Packaging launch (8.3)

- [ ] **deb.** `sudo apt install ./target/release/bundle/deb/*.deb` in WSL2;
      the installed binary launches under WSLg and its `.desktop` entry
      carries `MimeType=x-scheme-handler/tidal;`.
- [ ] **AppImage.** `chmod +x` the AppImage and run it under WSLg; the window
      renders and the version matches the build.
- [ ] **`tidal://` best effort.** If `xdg-open tidal://...` reaches the app
      under WSLg, record it; full registration/launch verification is on the
      bare-metal checklist.

Close-out: when every item is PASS (or explicitly recorded as exempt), note
the result here and proceed to the completion gate in
`2026-10-03-track8-bare-metal-checklist.md` / spec §5.4.
```

- [ ] **Step 2: Create the deferred bare-metal checklist**

Create `docs/superpowers/plans/notes/2026-10-03-track8-bare-metal-checklist.md`:

```markdown
# Track 8 bare-metal completion checklist (2026-10-03)

Deferred bare-metal checks for Track 8, per
`docs/superpowers/specs/2026-10-03-oxitide-linux-support-design.md` §5.3–§5.4.
These cannot be verified in WSL2 and **gate Track 8 closure**. Record each
item with date, distro/kernel, DE/session (X11/Wayland), audio server
(PipeWire/PulseAudio), and evidence. Cover the spike's representative matrix
(`2026-10-01-linux-support-spike.md`): one PipeWire distro + one PulseAudio
distro, one X11 session + one Wayland session.

## Audio

- [ ] **Exclusive / bit-perfect on a real DAC.** Select the DAC in Exclusive
      mode: the engine opens `hw:` directly, status reports bit-perfect, and
      the same file plays without server resampling. Hold the device in
      another app (or pick an unsupported format) → `ExclusiveFallback`
      reopens shared with the existing notice.
- [ ] **Hotplug.** Unplug/replug a USB DAC while paused → enumeration lists it
      again; default-device switch mid-playback and resume behave as on
      Windows. (On-demand enumeration only; no hotplug events is accepted.)
- [ ] **Hardware mixer volume.** With a writable card Master/PCM element the
      slider drives the mixer and stays bit-perfect; with none, software
      scaling applies the "volume in software" notice.

## Desktop integration

- [ ] **Real-DE MPRIS + media keys.** On GNOME and KDE (and XFCE/Cinnamon
      where available) the desktop media widget reflects metadata and
      playback, hardware media keys route to OxiTide, and `playerctl` agrees.
- [ ] **`tidal://` registration/launch.** After deb/AUR install,
      `xdg-open 'tidal://...'` launches or focuses the app and routes the
      deep link; a second invocation hits the single-instance path.
- [ ] **Wayland behavior.** Under Wayland, size and normal restore keep
      working; mini-player placement and saved-position restore degrade
      (documented limitation) but never visibly misplace the window. Under
      X11 both restore fully.

## Packaging and platform floor

- [ ] **AUR install.** Build/install `packaging/aur/PKGBUILD` on Arch against
      `webkit2gtk-4.1`; the app launches and plays.
- [ ] **WebKitGTK 4.1 floor.** On Ubuntu 22.04 / Debian 12, the Phase 2 CSS
      pass (`@property`, `backdrop-filter`, `color-mix` in
      `ui/src/styles/`) renders correctly in both modes.
- [ ] **AppImage oldest-base launch.** Run the AppImage built on Ubuntu 22.04
      on 22.04 (the floor) and on a current distro; it launches and plays.

## Completion-gate protocol (spec §5.4)

Track 8 closes only when this checklist is run and recorded on bare metal, or
the owner explicitly records a completion-note exemption naming what remains
unverified. On closure: record the run evidence here (or in a dated completion
note), list every FAIL/exemption with its follow-up, and update the Track 8
status in the roadmap. WSLg-verifiable items are closed by
`2026-10-03-track8-wslg-smoke.md`; only the deferred items above gate closure.
```

- [ ] **Step 3: Run every Windows gate**

Run: `cargo test --workspace`
Expected: all pass.

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings.

Run: `npm --prefix ui run check`
Expected: 0 errors.

Run: `npm --prefix ui test`
Expected: all suites pass.

- [ ] **Step 4: Run the WSL2/Linux gates**

In the WSL2 Ubuntu 22.04 environment from `docs/linux-build.md`, from the repo checkout:

Run: `cargo test --workspace`
Expected: all pass.

Run: `cargo clippy --workspace -- -D warnings`
Expected: no warnings.

Run: `cargo build -p oxitide-app`
Expected: builds with the Tauri prerequisites installed.

Optional, not gating (no Windows runtime code changed): `powershell -ExecutionPolicy Bypass -File scripts/perf/run-scenario.ps1 -Scenario S5` followed by `node scripts/perf/check.mjs --scenario S5`; expect the Phase D S5 rows to pass as recorded in `docs/superpowers/plans/notes/2026-10-02-track3-phase-d.md`.

- [ ] **Step 5: Commit**

```bash
git add docs/superpowers/plans/notes/2026-10-03-track8-wslg-smoke.md docs/superpowers/plans/notes/2026-10-03-track8-bare-metal-checklist.md
git commit -m "test: add the Track 8 WSLg and bare-metal smoke checklists"
```
