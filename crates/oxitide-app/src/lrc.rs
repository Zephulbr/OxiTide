//! Pure LRC parsing (Track 4a spec §5.3, §7). No I/O: every branch is
//! unit-testable.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricWord {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LyricLine {
    /// Milliseconds from track start; `None` for plain (unsynced) lyrics.
    pub time_ms: Option<u64>,
    pub text: String,
    /// Enhanced-LRC word stamps (`<mm:ss.xx>`), empty for standard LRC lines.
    /// `default` keeps previously cached DTOs loadable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub words: Vec<LyricWord>,
}

/// Shift a parsed stamp by the document offset, clamped at zero.
fn shift(ms: i64, offset: i64) -> u64 {
    ms.saturating_add(offset).max(0) as u64
}

/// Parse an LRC document. Multiple stamps on one line expand into one line per
/// stamp; `[offset:±ms]` shifts every stamp; `[mm:ss.xx]`/`[mm:ss.xxx]` are both
/// accepted; metadata tags and malformed lines are skipped; blank timed lines
/// are kept as gaps. Enhanced-LRC `<mm:ss.xx>` word stamps are extracted into
/// `words` (text before the first stamp takes the line's time; malformed tags
/// stay literal text).
pub fn parse_lrc(text: &str) -> Vec<LyricLine> {
    let offset = offset_ms(text);
    let mut lines = Vec::new();
    for raw in text.lines() {
        let (stamps, body) = split_tags(raw);
        if stamps.is_empty() {
            continue; // metadata or a bare, untimed line
        }
        let body = body.trim_end_matches('\r');
        let first_ms = shift(stamps[0], offset);
        let (line_text, words) = split_words(body, first_ms, offset);
        for stamp in stamps {
            lines.push(LyricLine {
                time_ms: Some(shift(stamp, offset)),
                text: line_text.clone(),
                words: words.clone(),
            });
        }
    }
    lines
}

/// Split enhanced-LRC word stamps out of a line body. The returned text is the
/// body with every valid stamp removed; `words` stays empty when the body has
/// no valid stamps (a standard LRC line, byte-identical to before).
fn split_words(body: &str, first_ms: u64, offset: i64) -> (String, Vec<LyricWord>) {
    let mut words: Vec<LyricWord> = Vec::new();
    let mut segment = String::new();
    let mut segment_ms: Option<u64> = None;
    let mut rest = body;
    let mut found_stamp = false;

    while let Some(open) = rest.find('<') {
        let Some(close_rel) = rest[open + 1..].find('>') else {
            break;
        };
        let close = open + 1 + close_rel;
        if let Some(ms) = parse_stamp(&rest[open + 1..close]) {
            found_stamp = true;
            segment.push_str(&rest[..open]);
            if !segment.is_empty() {
                words.push(LyricWord {
                    time_ms: segment_ms.unwrap_or(first_ms),
                    text: std::mem::take(&mut segment),
                });
            }
            segment_ms = Some(shift(ms, offset));
        } else {
            // Not a stamp (e.g. `<b>`): part of the word text.
            segment.push_str(&rest[..close + 1]);
        }
        rest = &rest[close + 1..];
    }
    segment.push_str(rest);
    if !segment.is_empty() {
        words.push(LyricWord {
            time_ms: segment_ms.unwrap_or(first_ms),
            text: segment,
        });
    }
    if !found_stamp {
        return (body.to_string(), Vec::new());
    }
    let mut line_text = String::new();
    for word in &words {
        line_text.push_str(&word.text);
    }
    (line_text, words)
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
        let tag = &trimmed[1..end];
        if let Some(ms) = parse_stamp(tag) {
            stamps.push(ms);
        } else if tag.contains(':') {
            // Metadata tag ([ar:], [offset:], …): consumed and ignored.
        } else {
            // Bracketed body text like [Chorus]: part of the lyric line.
            rest = trimmed;
            break;
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
            words: Vec::new(),
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
                    text: "Listen".into(),
                    words: Vec::new()
                },
                LyricLine {
                    time_ms: Some(30_880),
                    text: "Watch".into(),
                    words: Vec::new()
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
                text: "ok".into(),
                words: Vec::new()
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
                text: String::new(),
                words: Vec::new()
            }
        );
    }

    #[test]
    fn keeps_bracketed_body_text() {
        let lines = parse_lrc("[00:01.00] [Chorus]\n[00:02.00] verse");
        assert_eq!(lines[0].text, "[Chorus]");
        assert_eq!(lines[1].text, "verse");
    }

    #[test]
    fn split_plain_drops_trailing_empties() {
        let lines = split_plain("one\ntwo\n\n");
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|l| l.time_ms.is_none()));
    }

    #[test]
    fn parses_enhanced_word_stamps() {
        let lines = parse_lrc("[00:10.00]<00:10.00>Hello <00:10.50>world");
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0].time_ms, Some(10_000));
        assert_eq!(lines[0].text, "Hello world");
        assert_eq!(
            lines[0].words,
            vec![
                LyricWord {
                    time_ms: 10_000,
                    text: "Hello ".into()
                },
                LyricWord {
                    time_ms: 10_500,
                    text: "world".into()
                },
            ]
        );
    }

    #[test]
    fn enhanced_prefix_text_takes_the_line_stamp() {
        let lines = parse_lrc("[00:01.00]Intro <00:01.50>word");
        assert_eq!(lines[0].text, "Intro word");
        assert_eq!(
            lines[0].words,
            vec![
                LyricWord {
                    time_ms: 1_000,
                    text: "Intro ".into()
                },
                LyricWord {
                    time_ms: 1_500,
                    text: "word".into()
                },
            ]
        );
    }

    #[test]
    fn enhanced_malformed_tags_stay_literal() {
        let lines = parse_lrc("[00:01.00]<00:01.00>a <b> c");
        assert_eq!(lines[0].text, "a <b> c");
        assert_eq!(
            lines[0].words,
            vec![LyricWord {
                time_ms: 1_000,
                text: "a <b> c".into()
            }]
        );
    }

    #[test]
    fn enhanced_words_apply_offset() {
        let lines = parse_lrc("[offset:500]\n[00:01.00]<00:01.00>a <00:02.00>b");
        assert_eq!(
            lines[0].words,
            vec![
                LyricWord {
                    time_ms: 1_500,
                    text: "a ".into()
                },
                LyricWord {
                    time_ms: 2_500,
                    text: "b".into()
                },
            ]
        );
    }

    #[test]
    fn plain_timed_lines_have_no_words() {
        let lines = parse_lrc("[00:01.00] ok");
        assert_eq!(lines[0].text, "ok");
        assert!(lines[0].words.is_empty());
    }

    #[test]
    fn enhanced_lone_angle_stays_literal() {
        let body = "a < b <00:02.00>c";
        let lines = parse_lrc(&format!("[00:01.00]{body}"));
        assert_eq!(lines[0].text, body);
        assert!(lines[0].words.is_empty());
    }

    #[test]
    fn enhanced_consecutive_tags_drop_empty_segments() {
        let lines = parse_lrc("[00:01.00]<00:01.00><00:02.00>b");
        assert_eq!(lines[0].text, "b");
        assert_eq!(
            lines[0].words,
            vec![LyricWord {
                time_ms: 2_000,
                text: "b".into()
            }]
        );
    }

    #[test]
    fn enhanced_words_clone_across_duplicate_line_stamps() {
        let lines = parse_lrc("[00:01.00][00:02.00]<00:01.00>a");
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0].words, lines[1].words);
        assert_eq!(lines[0].words[0].time_ms, 1_000);
        assert_eq!(lines[1].time_ms, Some(2_000));
    }
}
