//! Performance harness core: process-start milestones, audio/cache counters,
//! memory samples, and the JSONL report sink for a perf session.
//!
//! Inactive unless `OXITIDE_PERF_SCENARIO` is set at launch. In normal runs
//! nothing is written and the per-event cost is a single `OnceLock` read.

use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use oxitide_audio::perf::AudioPerfEvent;
use serde::Serialize;

pub const SCENARIO_ENV: &str = "OXITIDE_PERF_SCENARIO";
pub const DIR_ENV: &str = "OXITIDE_PERF_DIR";

static SESSION: OnceLock<Arc<PerfSession>> = OnceLock::new();

#[derive(Debug, Clone, Serialize)]
pub struct PerfMark {
    pub name: String,
    /// Milliseconds since process creation (monotonic within the run).
    pub t_ms: u64,
    /// Wall-clock time of the mark.
    pub unix_ms: u128,
}

#[derive(Debug, Clone, Serialize)]
pub struct PerfSnapshot {
    pub active: bool,
    pub run_id: String,
    pub scenario: String,
    pub marks: Vec<PerfMark>,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub last_ttfs_ms: Option<u64>,
    pub memory_mb: Option<u64>,
}

#[derive(Default)]
struct Counters {
    cache_hits: u64,
    cache_misses: u64,
    last_ttfs_ms: Option<u64>,
}

pub struct PerfSession {
    run_id: String,
    scenario: String,
    jsonl_path: PathBuf,
    /// Session-creation instant; `t_ms` adds `loader_offset_ms` to this.
    start: Instant,
    process_start_unix_ms: u128,
    /// Milliseconds between process creation and session creation, so `t_ms`
    /// stays process-relative (loader time included).
    loader_offset_ms: u64,
    out: Mutex<BufWriter<File>>,
    marks: Mutex<Vec<PerfMark>>,
    counters: Mutex<Counters>,
    play_pending: Mutex<Option<Instant>>,
}

impl PerfSession {
    /// Create the run file under `dir`. Fails cleanly when `dir` cannot be
    /// created or the run file cannot be opened.
    pub fn create_in(dir: &Path, scenario: &str) -> std::io::Result<Arc<PerfSession>> {
        std::fs::create_dir_all(dir)?;
        let unix_secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let run_id = format!("{scenario}-{unix_secs}");
        let jsonl_path = dir.join(format!("{run_id}.jsonl"));
        let file = OpenOptions::new().create(true).append(true).open(&jsonl_path)?;
        let process_start_unix_ms =
            process_creation_unix_ms().unwrap_or_else(now_unix_ms);
        let loader_offset_ms = now_unix_ms()
            .saturating_sub(process_start_unix_ms)
            .min(u64::MAX as u128) as u64;
        let session = Arc::new(PerfSession {
            run_id,
            scenario: scenario.to_string(),
            jsonl_path,
            start: Instant::now(),
            process_start_unix_ms,
            loader_offset_ms,
            out: Mutex::new(BufWriter::new(file)),
            marks: Mutex::new(Vec::new()),
            counters: Mutex::new(Counters::default()),
            play_pending: Mutex::new(None),
        });
        session.write(serde_json::json!({
            "kind": "session",
            "run_id": session.run_id,
            "scenario": session.scenario,
            "unix_ms": session.process_start_unix_ms,
        }));
        Ok(session)
    }

    pub fn jsonl_path(&self) -> &Path {
        &self.jsonl_path
    }

    fn write(&self, mut value: serde_json::Value) {
        if let serde_json::Value::Object(map) = &mut value {
            map.insert("run_id".into(), serde_json::Value::String(self.run_id.clone()));
        }
        let mut out = self.out.lock().unwrap();
        let mut line = value.to_string();
        line.push('\n');
        if let Err(e) = out.write_all(line.as_bytes()).and_then(|()| out.flush()) {
            tracing::warn!("perf write failed: {e}");
        }
    }

    fn elapsed_ms(&self) -> u64 {
        let session_ms = self.start.elapsed().as_millis().min(u64::MAX as u128) as u64;
        self.loader_offset_ms.saturating_add(session_ms)
    }

    pub fn mark(&self, name: &str) {
        let t_ms = self.elapsed_ms();
        let unix_ms = self.process_start_unix_ms + t_ms as u128;
        let mark = PerfMark { name: name.to_string(), t_ms, unix_ms };
        self.marks.lock().unwrap().push(mark.clone());
        self.write(serde_json::json!({
            "kind": "mark",
            "name": mark.name,
            "t_ms": mark.t_ms,
            "unix_ms": mark.unix_ms,
        }));
    }

    /// Called by the play commands so the next `FirstPcm` yields a TTFS.
    pub fn note_play_command(&self) {
        *self.play_pending.lock().unwrap() = Some(Instant::now());
    }

    pub fn audio_event(&self, event: AudioPerfEvent) {
        let mut counters = self.counters.lock().unwrap();
        match &event {
            AudioPerfEvent::CacheHit { .. } => counters.cache_hits += 1,
            AudioPerfEvent::CacheMiss { .. } => counters.cache_misses += 1,
            AudioPerfEvent::FirstPcm { .. } => {
                if let Some(started) = self.play_pending.lock().unwrap().take() {
                    // A pending play that never produced audio (pause toggles,
                    // errors) must not pair with a much later first PCM.
                    if started.elapsed() <= Duration::from_secs(30) {
                        let ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
                        counters.last_ttfs_ms = Some(ms);
                        self.write(serde_json::json!({
                            "kind": "ttfs",
                            "ms": ms,
                            "t_ms": self.elapsed_ms(),
                        }));
                    }
                }
            }
            _ => {}
        }
        let (kind, track_id, gap_ms) = match &event {
            AudioPerfEvent::ResolveStart { track_id } => ("resolve_start", *track_id, None),
            AudioPerfEvent::DecodeStart { track_id } => ("decode_start", *track_id, None),
            AudioPerfEvent::InfoReady { track_id } => ("info_ready", *track_id, None),
            AudioPerfEvent::OutputOpened { track_id } => ("output_opened", *track_id, None),
            AudioPerfEvent::CacheHit { track_id } => ("cache_hit", *track_id, None),
            AudioPerfEvent::CacheMiss { track_id } => ("cache_miss", *track_id, None),
            AudioPerfEvent::FirstPcm { track_id } => ("first_pcm", *track_id, None),
            AudioPerfEvent::Advance { track_id, gap_ms } => ("advance", *track_id, Some(*gap_ms)),
        };
        self.write(serde_json::json!({
            "kind": "audio",
            "event": kind,
            "track_id": track_id,
            "gap_ms": gap_ms,
            "t_ms": self.elapsed_ms(),
        }));
    }

    pub fn ui_metrics(&self, metrics: serde_json::Value) {
        self.write(serde_json::json!({
            "kind": "ui_metrics",
            "t_ms": self.elapsed_ms(),
            "metrics": metrics,
        }));
    }

    pub fn write_memory_sample(&self) {
        let rows = process_memory_rows();
        if rows.is_empty() {
            return;
        }
        let total: u64 = rows.iter().map(|(_, mb, _)| mb).sum();
        let t_ms = self.elapsed_ms();
        self.write(serde_json::json!({
            "kind": "memory",
            "mb": total,
            "t_ms": t_ms,
        }));
        for (name, mb, is_self) in rows {
            self.write(serde_json::json!({
                "kind": "memory_process",
                "name": name,
                "mb": mb,
                "self": is_self,
                "t_ms": t_ms,
            }));
        }
    }

    pub fn snapshot(&self) -> PerfSnapshot {
        let counters = self.counters.lock().unwrap();
        PerfSnapshot {
            active: true,
            run_id: self.run_id.clone(),
            scenario: self.scenario.clone(),
            marks: self.marks.lock().unwrap().clone(),
            cache_hits: counters.cache_hits,
            cache_misses: counters.cache_misses,
            last_ttfs_ms: counters.last_ttfs_ms,
            memory_mb: process_tree_memory_mb(),
        }
    }
}

/// Start the session from the environment. `None` (with no side effects) when
/// `OXITIDE_PERF_SCENARIO` is unset; a warning and `None` when the directory is
/// unusable.
pub fn install() -> Option<Arc<PerfSession>> {
    let scenario = std::env::var(SCENARIO_ENV)
        .ok()
        .filter(|s| !s.trim().is_empty())?;
    let dir = std::env::var(DIR_ENV)
        .ok()
        .filter(|s| !s.trim().is_empty())
        .map(PathBuf::from)
        .or_else(default_dir)?;
    match PerfSession::create_in(&dir, &scenario) {
        Ok(session) => {
            tracing::info!("perf session {} -> {}", session.run_id, dir.display());
            std::thread::Builder::new()
                .name("oxitide-perf-mem".into())
                .spawn({
                    let session = session.clone();
                    move || loop {
                        std::thread::sleep(Duration::from_secs(2));
                        session.write_memory_sample();
                    }
                })
                .ok();
            let _ = SESSION.set(session.clone());
            Some(session)
        }
        Err(e) => {
            tracing::warn!("perf session disabled: {e}");
            None
        }
    }
}

pub fn session() -> Option<&'static Arc<PerfSession>> {
    SESSION.get()
}

pub fn mark(name: &str) {
    if let Some(session) = session() {
        session.mark(name);
    }
}

pub fn note_play_command() {
    if let Some(session) = session() {
        session.note_play_command();
    }
}

pub fn audio_event(event: AudioPerfEvent) {
    if let Some(session) = session() {
        session.audio_event(event);
    }
}

pub fn ui_metrics(metrics: serde_json::Value) {
    if let Some(session) = session() {
        session.ui_metrics(metrics);
    }
}

pub fn snapshot() -> PerfSnapshot {
    snapshot_for(session())
}

fn snapshot_for(session: Option<&Arc<PerfSession>>) -> PerfSnapshot {
    session.map(|s| s.snapshot()).unwrap_or(PerfSnapshot {
        active: false,
        run_id: String::new(),
        scenario: String::new(),
        marks: Vec::new(),
        cache_hits: 0,
        cache_misses: 0,
        last_ttfs_ms: None,
        memory_mb: None,
    })
}

#[cfg(debug_assertions)]
fn default_dir() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    Some(root.join(".superpowers/perf/runs"))
}

#[cfg(not(debug_assertions))]
fn default_dir() -> Option<PathBuf> {
    None
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0)
}

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

/// Windows: the process creation time in unix milliseconds from
/// `GetProcessTimes`; `None` when unavailable.
#[cfg(windows)]
fn process_creation_unix_ms() -> Option<u128> {
    use windows::Win32::Foundation::FILETIME;
    use windows::Win32::System::Threading::{GetCurrentProcess, GetProcessTimes};

    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(GetCurrentProcess(), &mut creation, &mut exit, &mut kernel, &mut user)
            .ok()?;
    }
    let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    // FILETIME counts 100 ns intervals since 1601-01-01; unix epoch offset:
    const UNIX_EPOCH_FILETIME_TICKS: u64 = 116_444_736_000_000_000;
    if ticks < UNIX_EPOCH_FILETIME_TICKS {
        return None;
    }
    Some(((ticks - UNIX_EPOCH_FILETIME_TICKS) / 10_000) as u128)
}

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

/// Working set of this process plus all descendant processes (the WebView2
/// helpers), in MiB, one row per process: (name, mb, is_the_app_itself).
#[cfg(windows)]
fn process_memory_rows() -> Vec<(String, u64, bool)> {
    use windows::Win32::Foundation::{CloseHandle, HANDLE};
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
    };
    use windows::Win32::System::Threading::{
        OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_VM_READ,
    };

    let self_pid = std::process::id();
    let Ok(snapshot) = (unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) }) else {
        return Vec::new();
    };
    let mut entry = PROCESSENTRY32W {
        dwSize: std::mem::size_of::<PROCESSENTRY32W>() as u32,
        ..Default::default()
    };
    let mut parents: Vec<ProcessRow> = Vec::new();
    unsafe {
        if Process32FirstW(snapshot, &mut entry).is_ok() {
            loop {
                let len = entry
                    .szExeFile
                    .iter()
                    .position(|c| *c == 0)
                    .unwrap_or(entry.szExeFile.len());
                let name = String::from_utf16_lossy(&entry.szExeFile[..len]);
                // Keep the app itself even when the binary is renamed (the
                // test harness runs as `oxitide_app-<hash>.exe`).
                if entry.th32ProcessID != self_pid && !tracked_process_name(&name) {
                    if Process32NextW(snapshot, &mut entry).is_err() {
                        break;
                    }
                    continue;
                }
                let created = creation_ms_for_pid(entry.th32ProcessID);
                parents.push((entry.th32ProcessID, entry.th32ParentProcessID, name, created));
                if Process32NextW(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
    }
    let mut rows = Vec::new();
    for &(pid, _, ref name, _) in &parents {
        if !is_descendant(&parents, self_pid, pid) {
            continue;
        }
        let handle: HANDLE =
            match unsafe { OpenProcess(PROCESS_QUERY_INFORMATION | PROCESS_VM_READ, false, pid) } {
                Ok(handle) => handle,
                Err(_) => continue,
            };
        let mut counters = PROCESS_MEMORY_COUNTERS::default();
        let ok = unsafe {
            GetProcessMemoryInfo(
                handle,
                &mut counters,
                std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            )
        };
        let _ = unsafe { CloseHandle(handle) };
        if ok.is_ok() {
            rows.push((
                name.clone(),
                counters.WorkingSetSize as u64 / (1024 * 1024),
                pid == self_pid,
            ));
        }
    }
    rows
}

/// One process from the Toolhelp snapshot: `(pid, parent pid, name, creation
/// time in unix ms when readable)`.
#[cfg(any(windows, test))]
type ProcessRow = (u32, u32, String, Option<u64>);

/// Processes the memory metric tracks: the app itself, WebView2's children,
/// and the console host Windows spawns alongside them. Restricting the
/// snapshot keeps the PID-reuse creation-time lookup to a handful of
/// `OpenProcess` calls per sample; opening every process on the machine
/// measurably slowed the S5 scroll frames.
#[cfg(any(windows, test))]
fn tracked_process_name(name: &str) -> bool {
    name.eq_ignore_ascii_case("oxitide-app.exe")
        || name.eq_ignore_ascii_case("msedgewebview2.exe")
        || name.eq_ignore_ascii_case("conhost.exe")
}

/// Walk up to 8 ancestors looking for `self_pid`. Creation-time ordering
/// rejects PID-reuse false positives: a "parent" created after its child
/// cannot be the real ancestor (observed once: unrelated browser processes
/// were folded into an S4 memory sample this way).
#[cfg(any(windows, test))]
fn is_descendant(processes: &[ProcessRow], self_pid: u32, pid: u32) -> bool {
    let Some(start) = processes.iter().find(|(p, _, _, _)| *p == pid) else {
        return false;
    };
    let mut child_created = start.3;
    let mut current = pid;
    for _ in 0..8 {
        if current == self_pid {
            return true;
        }
        let Some(entry) = processes.iter().find(|(p, _, _, _)| *p == current) else {
            return false;
        };
        if entry.1 == 0 {
            return false;
        }
        let Some(parent) = processes.iter().find(|(p, _, _, _)| *p == entry.1) else {
            return false;
        };
        if let (Some(child), Some(parent_created)) = (child_created, parent.3) {
            if parent_created > child {
                return false;
            }
        }
        child_created = parent.3;
        current = parent.0;
    }
    false
}

/// Creation time of a process in unix ms, for the PID-reuse guard.
#[cfg(windows)]
fn creation_ms_for_pid(pid: u32) -> Option<u64> {
    use windows::Win32::Foundation::{CloseHandle, FILETIME};
    use windows::Win32::System::Threading::{
        GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION,
    };

    let handle = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    let ok = unsafe { GetProcessTimes(handle, &mut creation, &mut exit, &mut kernel, &mut user) };
    let _ = unsafe { CloseHandle(handle) };
    ok.ok()?;
    let ticks = ((creation.dwHighDateTime as u64) << 32) | creation.dwLowDateTime as u64;
    const UNIX_EPOCH_FILETIME_TICKS: u64 = 116_444_736_000_000_000;
    if ticks < UNIX_EPOCH_FILETIME_TICKS {
        return None;
    }
    Some((ticks - UNIX_EPOCH_FILETIME_TICKS) / 10_000)
}

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

/// Working set of this process plus all descendant processes, in MiB.
fn process_tree_memory_mb() -> Option<u64> {
    let rows = process_memory_rows();
    if rows.is_empty() {
        return None;
    }
    Some(rows.iter().map(|(_, mb, _)| mb).sum())
}

#[cfg(test)]
mod tests {
    use super::*;
    use oxitide_audio::perf::AudioPerfEvent;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir()
            .join(format!("oxitide-perf-test-{}-{tag}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn run_id_contains_the_scenario() {
        let session = PerfSession::create_in(&temp_dir("runid"), "S1").unwrap();
        assert!(session.run_id.starts_with("S1-"));
    }

    #[test]
    fn marks_are_flushed_as_parseable_jsonl() {
        let dir = temp_dir("marks");
        let session = PerfSession::create_in(&dir, "S1").unwrap();
        session.mark("config_loaded");
        session.mark("window_shown");
        let text = std::fs::read_to_string(session.jsonl_path()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("every line must be valid JSON"))
            .collect();
        assert_eq!(lines[0]["kind"], "session");
        assert_eq!(lines[1]["kind"], "mark");
        assert_eq!(lines[1]["name"], "config_loaded");
        assert_eq!(lines[2]["name"], "window_shown");
        assert!(lines[1]["t_ms"].as_u64().is_some());
    }

    #[test]
    fn first_pcm_after_a_play_command_records_ttfs() {
        let session = PerfSession::create_in(&temp_dir("ttfs"), "S3-hit").unwrap();
        session.note_play_command();
        std::thread::sleep(std::time::Duration::from_millis(5));
        session.audio_event(AudioPerfEvent::FirstPcm { track_id: 7 });
        let snapshot = session.snapshot();
        let ttfs = snapshot.last_ttfs_ms.expect("ttfs recorded");
        assert!(ttfs >= 5, "ttfs should include the sleep, got {ttfs}");
    }

    #[test]
    fn audio_events_count_cache_hits_and_misses() {
        let session = PerfSession::create_in(&temp_dir("counters"), "S8").unwrap();
        session.audio_event(AudioPerfEvent::CacheHit { track_id: 1 });
        session.audio_event(AudioPerfEvent::CacheMiss { track_id: 2 });
        session.audio_event(AudioPerfEvent::CacheMiss { track_id: 3 });
        let snapshot = session.snapshot();
        assert_eq!(snapshot.cache_hits, 1);
        assert_eq!(snapshot.cache_misses, 2);
    }

    #[test]
    fn create_in_fails_when_the_path_is_a_file() {
        let file = temp_dir("file");
        std::fs::write(&file, b"not a dir").unwrap();
        assert!(PerfSession::create_in(&file, "S1").is_err());
        let _ = std::fs::remove_file(&file);
    }

    #[test]
    fn snapshot_without_a_session_is_inactive() {
        let snapshot = snapshot_for(None);
        assert!(!snapshot.active);
        assert!(snapshot.marks.is_empty());
    }

    #[test]
    fn marks_are_process_relative_and_share_the_unix_base() {
        let created = process_creation_unix_ms();
        let before_session = now_unix_ms();
        let dir = temp_dir("processbase");
        let session = PerfSession::create_in(&dir, "S1").unwrap();
        session.mark("config_loaded");
        let text = std::fs::read_to_string(session.jsonl_path()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("every line must be valid JSON"))
            .collect();
        let session_line = lines.iter().find(|l| l["kind"] == "session").unwrap();
        let mark = lines.iter().find(|l| l["kind"] == "mark").unwrap();
        let t_ms = mark["t_ms"].as_u64().unwrap() as u128;
        if let Some(created) = created {
            assert!(
                t_ms >= before_session.saturating_sub(created),
                "t_ms {t_ms} must include loader time (process age {})",
                before_session.saturating_sub(created)
            );
        }
        assert_eq!(
            mark["unix_ms"].as_u64().unwrap() as u128,
            session_line["unix_ms"].as_u64().unwrap() as u128 + t_ms,
            "unix_ms must be process start plus t_ms"
        );
    }

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

    #[test]
    fn tracked_process_names_are_the_app_webview2_and_conhost() {
        assert!(tracked_process_name("oxitide-app.exe"));
        assert!(tracked_process_name("msedgewebview2.exe"));
        assert!(tracked_process_name("conhost.exe"));
        assert!(!tracked_process_name("zen.exe"));
        assert!(!tracked_process_name("reg.exe"));
    }

    #[test]
    fn descendant_walk_rejects_pid_reuse() {
        let processes: Vec<ProcessRow> = vec![
            (10, 0, "self".into(), Some(100)),
            (20, 10, "child".into(), Some(200)),
            (30, 20, "grandchild".into(), Some(300)),
        ];
        assert!(is_descendant(&processes, 10, 30));
        assert!(is_descendant(&processes, 10, 10));
        assert!(!is_descendant(&processes, 99, 30));

        // PID 20 was reused after the real parent exited: the current process
        // with that pid was created *after* its supposed child, so the chain
        // must be rejected (the S4 run-4 zen.exe contamination).
        let reused: Vec<ProcessRow> = vec![
            (10, 0, "self".into(), Some(100)),
            (20, 10, "reused".into(), Some(900)),
            (30, 20, "child".into(), Some(300)),
            (40, 30, "zen.exe".into(), Some(450)),
        ];
        assert!(!is_descendant(&reused, 10, 40));
    }

    #[cfg(any(windows, target_os = "linux"))]
    #[test]
    fn memory_samples_name_each_process_and_flag_the_app() {
        let dir = temp_dir("memproc");
        let session = PerfSession::create_in(&dir, "S1").unwrap();
        session.write_memory_sample();
        let text = std::fs::read_to_string(session.jsonl_path()).unwrap();
        let lines: Vec<serde_json::Value> = text
            .lines()
            .map(|l| serde_json::from_str(l).expect("valid JSON"))
            .collect();
        let aggregate = lines.iter().find(|l| l["kind"] == "memory").expect("aggregate line");
        assert!(aggregate["mb"].as_u64().unwrap() > 0);
        let processes: Vec<_> =
            lines.iter().filter(|l| l["kind"] == "memory_process").collect();
        assert!(!processes.is_empty(), "at least the app process must be sampled");
        assert!(
            processes.iter().any(|l| l["self"].as_bool() == Some(true)
                && l["mb"].as_u64().unwrap() > 0
                && l["name"].as_str().is_some_and(|n| !n.is_empty())),
            "the app process must be named and flagged"
        );
        let sum: u64 = processes.iter().map(|l| l["mb"].as_u64().unwrap()).sum();
        assert_eq!(sum, aggregate["mb"].as_u64().unwrap(), "aggregate must be the row sum");
        let _ = std::fs::remove_dir_all(dir);
    }
}
