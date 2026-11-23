//! Console + file log. Every line is scrubbed before it is written anywhere.

use std::fs::File;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

use crate::scrub::Scrubber;

/// Longest logged line, applied after scrubbing.
const MAX_LINE: usize = 4000;

pub struct Log {
    pub scrubber: Scrubber,
    file: Mutex<Option<File>>,
    path: Option<PathBuf>,
    start: Instant,
}

impl Log {
    pub fn new(path: Option<PathBuf>) -> Self {
        let file = path.as_ref().and_then(|p| File::create(p).ok());
        let path = if file.is_some() { path } else { None };
        Self {
            scrubber: Scrubber::default(),
            file: Mutex::new(file),
            path,
            start: Instant::now(),
        }
    }

    pub fn path(&self) -> Option<&PathBuf> {
        self.path.as_ref()
    }

    /// A timestamped, scrubbed line on the console and in the log file.
    pub fn line(&self, text: impl AsRef<str>) {
        let elapsed = self.start.elapsed().as_secs_f32();
        let clean = self.scrubber.scrub(text.as_ref());
        for part in clean.lines() {
            let part = crate::scrub::truncate(part, MAX_LINE);
            let line = format!("[{elapsed:7.2}s] {part}");
            println!("{line}");
            if let Some(file) = self.file.lock().unwrap().as_mut() {
                let _ = writeln!(file, "{line}");
            }
        }
    }

    /// Untimestamped (still scrubbed), for headings and the summary.
    pub fn plain(&self, text: impl AsRef<str>) {
        let clean = self.scrubber.scrub(text.as_ref());
        println!("{clean}");
        if let Some(file) = self.file.lock().unwrap().as_mut() {
            let _ = writeln!(file, "{clean}");
        }
    }
}
