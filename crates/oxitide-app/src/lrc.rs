//! Pure LRC parsing (Track 4a spec §5.3, §7). No I/O: every branch is
//! unit-testable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// Milliseconds from track start; `None` for plain (unsynced) lyrics.
    pub time_ms: Option<u64>,
    pub text: String,
}

/// Parse an LRC document. Multiple stamps on one line expand into one line per
/// stamp; `[offset:±ms]` shifts every stamp; `[mm:ss.xx]`/`[mm:ss.xxx]` are both
/// accepted; metadata tags and malformed lines are skipped; blank timed lines
/// are kept as gaps.
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let offset = offset_ms(text);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let (stamps, body) = split_tags(raw);
        if stamps.is_empty() {
            continue; // metadata or a bare, untimed line
        }
        let body = body.trim_end_matches('\r').to_string();
        for stamp in stamps {
            let shifted = stamp.saturating_add(offset).max(0) as u64;
            lines.push(LyricLine {
                time_ms: Some(shifted),
                text: body.clone(),
            });
        }
    }
    lines
}

/// Split leading `[...]` tags off a line, returning the timed stamps (ms) and
/// the remaining text.
fn split_tags(line: &str) -> (Vec<i64>, String) {
    let mut stamps = Vec::new();
    let mut rest = line;
    loop {
        let trimmed = rest.trim_start();
        if !trimmed.starts_with('[') {
            rest = trimmed;
            break;
        }
        let Some(end) = trimmed.find(']') else {
            rest = trimmed;
            break;
        };
        if let Some(ms) = parse_stamp(&trimmed[1..end]) {
            stamps.push(ms);
        }
        rest = &trimmed[end + 1..];
    }
    (stamps, rest.to_string())
}

/// `mm:ss(.xx|.xxx)` or `hh:mm:ss(.xx|.xxx)` → milliseconds.
fn parse_stamp(tag: &str) -> Option<i64> {
    let parts: Vec<&str> = tag.split(':').collect();
    let (hours, minutes, seconds) = match parts.as_slice() {
        [m, s] => (0i64, m.parse::<i64>().ok()?, s.parse::<f64>().ok()?),
        [h, m, s] => (
            h.parse::<i64>().ok()?,
            m.parse::<i64>().ok()?,
            s.parse::<f64>().ok()?,
        ),
        _ => return None,
    };
    Some(hours * 3_600_000 + minutes * 60_000 + (seconds * 1000.0).round() as i64)
}

/// The last `[offset:±ms]` tag in the document, if any.
fn offset_ms(text: &str) -> i64 {
    let mut offset = 0;
    for line in text.lines() {
        let mut rest = line.trim_start();
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else { break };
            if let Some(value) = rest[1..end].strip_prefix("offset:") {
                offset = value.trim().parse::<i64>().unwrap_or(offset);
            }
            rest = &rest[end + 1..];
        }
    }
    offset
}

/// Split plain lyrics into untimed lines, dropping trailing empty lines.
pub fn split_plain(text: &str) -> Vec<LyricLine> {
    let mut lines: Vec<LyricLine> = text
        .lines()
        .map(|line| LyricLine {
            time_ms: None,
            text: line.trim_end_matches('\r').to_string(),
        })
        .collect();
    while lines.last().is_some_and(|line| line.text.trim().is_empty()) {
        lines.pop();
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_centisecond_stamps() {
        let lines = parse_lrc("[00:27.93] Listen\n[00:30.88] Watch");
        assert_eq!(
            lines,
            vec![
                LyricLine {
                    time_ms: Some(27_930),
                    text: "Listen".into()
                },
                LyricLine {
                    time_ms: Some(30_880),
                    text: "Watch".into()
                },
            ]
        );
    }

    #[test]
    fn parses_millisecond_stamps() {
        let lines = parse_lrc("[01:02.345] Hi");
        assert_eq!(lines[0].time_ms, Some(62_345));
    }

    #[test]
    fn expands_multiple_stamps_on_one_line() {
        let lines = parse_lrc("[00:10.00][01:20.00] same");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].time_ms, Some(10_000));
        assert_eq!(lines[1].time_ms, Some(80_000));
        assert_eq!(lines[0].text, "same");
    }

    #[test]
    fn applies_offset_tags() {
        let lines = parse_lrc("[offset:-500]\n[00:01.00] a");
        assert_eq!(lines[0].time_ms, Some(500));
    }

    #[test]
    fn ignores_metadata_and_malformed_lines() {
        let lines = parse_lrc("[ar:Artist]\n[ti:Title]\nnot timed\n[zz:xx] nope\n[00:02.00] ok");
        assert_eq!(
            lines,
            vec![LyricLine {
                time_ms: Some(2_000),
                text: "ok".into()
            }]
        );
    }

    #[test]
    fn keeps_blank_timed_lines() {
        let lines = parse_lrc("[00:01.00] a\n[00:02.00]\n[00:03.00] b");
        assert_eq!(
            lines[1],
            LyricLine {
                time_ms: Some(2_000),
                text: String::new()
            }
        );
    }

    #[test]
    fn split_plain_drops_trailing_empties() {
        let lines = split_plain("one\ntwo\n\n");
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.time_ms.is_none()));
    }
}
