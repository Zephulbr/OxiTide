use quick_xml::events::Event;
use quick_xml::Reader;

use crate::error::{Error, Result};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct TimelineEntry {
    pub t: Option<u64>,
    pub d: u64,
    pub r: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct SegmentTemplate {
    pub initialization: Option<String>,
    pub media: String,
    pub start_number: u64,
    pub timescale: u64,
    pub duration: Option<u64>,
    pub timeline: Vec<TimelineEntry>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Representation {
    pub id: Option<String>,
    pub codecs: Option<String>,
    pub bandwidth: Option<u64>,
    pub sample_rate: Option<u32>,
    pub bit_depth: Option<u32>,
    pub base_urls: Vec<String>,
    pub template: Option<SegmentTemplate>,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct Mpd {
    pub base_urls: Vec<String>,
    pub media_presentation_duration: Option<String>,
    pub representations: Vec<Representation>,
}

impl Mpd {
    /// TIDAL audio manifests expose one audio `Representation`; prefer a FLAC
    /// representation when more than one is present.
    pub fn audio_representation(&self) -> Option<&Representation> {
        self.representations
            .iter()
            .find(|r| {
                r.codecs
                    .as_deref()
                    .map(|c| c.contains("flac"))
                    .unwrap_or(false)
            })
            .or_else(|| self.representations.first())
    }
}

/// Timing of one media segment in SegmentTemplate ticks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SegmentTiming {
    pub start_ticks: u64,
    pub duration_ticks: u64,
}

impl Representation {
    pub fn base_url(&self, mpd_base: &[String]) -> String {
        let mut base = String::new();
        for b in mpd_base.iter().chain(self.base_urls.iter()) {
            base.push_str(b);
        }
        base
    }

    pub fn segment_count(&self, mpd_duration: Option<&str>) -> Result<u64> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("representation has no SegmentTemplate".into()))?;
        if !t.timeline.is_empty() {
            let ts = if t.timescale == 0 { 1 } else { t.timescale };
            let total_secs = mpd_duration.and_then(parse_iso8601_duration);
            let mut n: u64 = 0;
            let mut time = t.timeline[0].t.unwrap_or(0);
            for e in &t.timeline {
                if let Some(start) = e.t {
                    time = start;
                }
                let reps = repeat_count(e, time, ts, total_secs);
                n += reps;
                time += e.d * reps;
            }
            return Ok(n);
        }
        let dur = t
            .duration
            .ok_or_else(|| Error::Manifest("no segment duration".into()))?;
        let ts = if t.timescale == 0 { 1 } else { t.timescale };
        let seg_secs = dur as f64 / ts as f64;
        let total = parse_iso8601_duration(
            mpd_duration.ok_or_else(|| Error::Manifest("no mediaPresentationDuration".into()))?,
        )
        .ok_or_else(|| Error::Manifest("bad mediaPresentationDuration".into()))?;
        Ok((total / seg_secs).ceil() as u64)
    }

    pub fn init_url(&self, mpd_base: &[String]) -> Option<String> {
        let t = self.template.as_ref()?;
        let init = t.initialization.as_ref()?;
        Some(resolve_template(
            &self.base_url(mpd_base),
            init,
            self.id.as_deref().unwrap_or(""),
            self.bandwidth.unwrap_or(0),
            1,
            0,
        ))
    }

    /// Start/duration ticks for every media segment, in the same order as
    /// `media_urls`.
    pub fn segment_timings(&self, mpd_duration: Option<&str>) -> Result<Vec<SegmentTiming>> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("no SegmentTemplate".into()))?;
        if !t.timeline.is_empty() {
            let ts = if t.timescale == 0 { 1 } else { t.timescale };
            let total_secs = mpd_duration.and_then(parse_iso8601_duration);
            let mut timings = Vec::new();
            let mut time = t.timeline[0].t.unwrap_or(0);
            for e in &t.timeline {
                if let Some(start) = e.t {
                    time = start;
                }
                let reps = repeat_count(e, time, ts, total_secs);
                for _ in 0..reps {
                    timings.push(SegmentTiming { start_ticks: time, duration_ticks: e.d });
                    time += e.d;
                }
            }
            return Ok(timings);
        }
        let n = self.segment_count(mpd_duration)?;
        let duration_ticks = t
            .duration
            .ok_or_else(|| Error::Manifest("no segment duration".into()))?;
        Ok((0..n)
            .map(|i| SegmentTiming { start_ticks: duration_ticks * i, duration_ticks })
            .collect())
    }

    pub fn media_urls(
        &self,
        mpd_duration: Option<&str>,
        mpd_base: &[String],
    ) -> Result<Vec<String>> {
        let t = self
            .template
            .as_ref()
            .ok_or_else(|| Error::Manifest("no SegmentTemplate".into()))?;
        let base = self.base_url(mpd_base);
        let repr_id = self.id.clone().unwrap_or_default();
        let bw = self.bandwidth.unwrap_or(0);
        let timings = self.segment_timings(mpd_duration)?;
        Ok(timings
            .iter()
            .enumerate()
            .map(|(i, timing)| {
                let (number, time) = if t.timeline.is_empty() {
                    (t.start_number + i as u64, 0)
                } else {
                    (0, timing.start_ticks)
                };
                resolve_template(&base, &t.media, &repr_id, bw, number, time)
            })
            .collect())
    }
}

fn resolve_template(
    base: &str,
    tmpl: &str,
    repr_id: &str,
    bandwidth: u64,
    number: u64,
    time: u64,
) -> String {
    let expanded = tmpl
        .replace("$RepresentationID$", repr_id)
        .replace("$Number$", &number.to_string())
        .replace("$Time$", &time.to_string())
        .replace("$Bandwidth$", &bandwidth.to_string());
    format!("{base}{expanded}")
}

fn repeat_count(entry: &TimelineEntry, time: u64, timescale: u64, total_secs: Option<f64>) -> u64 {
    if entry.r < 0 {
        match total_secs {
            Some(total) if entry.d > 0 => {
                let end_ts = (total * timescale as f64) as u64;
                (end_ts.saturating_sub(time) / entry.d).max(1)
            }
            _ => 1,
        }
    } else {
        entry.r as u64 + 1
    }
}

fn parse_iso8601_duration(s: &str) -> Option<f64> {
    // Supports PT#H#M#S and P#DT#H#M#S (no years/months).
    let s = s.strip_prefix('P')?;
    let (date, time) = s.split_once('T').unwrap_or((s, ""));
    let mut secs = 0.0;
    let mut num = String::new();
    for c in date.chars() {
        if c == 'D' {
            secs += num.parse::<f64>().ok()? * 86_400.0;
            num.clear();
        } else {
            num.push(c);
        }
    }
    num.clear();
    for c in time.chars() {
        match c {
            'H' => {
                secs += num.parse::<f64>().ok()? * 3600.0;
                num.clear();
            }
            'M' => {
                secs += num.parse::<f64>().ok()? * 60.0;
                num.clear();
            }
            'S' => {
                secs += num.parse::<f64>().ok()?;
                num.clear();
            }
            _ => num.push(c),
        }
    }
    Some(secs)
}

fn local_name(name: quick_xml::name::QName) -> String {
    name.local_name().into_inner().to_string()
}

fn attr(e: &quick_xml::events::BytesStart, key: &str) -> Option<String> {
    e.attributes().flatten().find_map(|a| {
        if a.key.local_name().as_ref() == key {
            a.normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .ok()
                .map(|v| v.into_owned())
        } else {
            None
        }
    })
}

pub fn parse(xml: &str) -> Result<Mpd> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    let mut mpd = Mpd::default();
    let mut current_repr: Option<Representation> = None;
    let mut base_text: Option<bool> = None; // true = capture into mpd, false = repr

    let mut depth: i64 = 0;
    loop {
        let event = match reader.read_event() {
            Err(e) => return Err(Error::Mpd(e.to_string())),
            Ok(event) => event,
        };
        let is_empty = matches!(event, Event::Empty(_));
        match event {
            Event::Eof => break,
            Event::Text(t) if base_text.is_some() => {
                let text = quick_xml::escape::unescape(&t.into_inner())
                    .map_err(|e| Error::Mpd(e.to_string()))?
                    .into_owned();
                if base_text == Some(false) {
                    if let Some(r) = current_repr.as_mut() {
                        if !text.trim().is_empty() {
                            r.base_urls.push(text);
                        }
                    }
                } else if !text.is_empty() {
                    mpd.base_urls.push(text);
                }
                base_text = None;
            }
            Event::Start(e) | Event::Empty(e) => {
                if !is_empty {
                    depth += 1;
                }
                match local_name(e.name()).as_str() {
                    "MPD" => {
                        mpd.media_presentation_duration = attr(&e, "mediaPresentationDuration");
                    }
                    "BaseURL" => {
                        base_text = Some(current_repr.is_none());
                    }
                    "Representation" => {
                        let r = Representation {
                            id: attr(&e, "id"),
                            codecs: attr(&e, "codecs"),
                            bandwidth: attr(&e, "bandwidth").and_then(|v| v.parse().ok()),
                            sample_rate: attr(&e, "audioSamplingRate").and_then(|v| v.parse().ok()),
                            bit_depth: attr(&e, "bitDepth").and_then(|v| v.parse().ok()),
                            ..Default::default()
                        };
                        if is_empty {
                            mpd.representations.push(r);
                        } else {
                            current_repr = Some(r);
                        }
                    }
                    "SegmentTemplate" => {
                        let t = SegmentTemplate {
                            initialization: attr(&e, "initialization"),
                            media: attr(&e, "media").unwrap_or_default(),
                            start_number: attr(&e, "startNumber")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1),
                            timescale: attr(&e, "timescale")
                                .and_then(|v| v.parse().ok())
                                .unwrap_or(1),
                            duration: attr(&e, "duration").and_then(|v| v.parse().ok()),
                            timeline: Vec::new(),
                        };
                        if let Some(r) = current_repr.as_mut() {
                            r.template = Some(t);
                        }
                    }
                    "S" => {
                        if let Some(r) = current_repr.as_mut() {
                            if let Some(t) = r.template.as_mut() {
                                t.timeline.push(TimelineEntry {
                                    t: attr(&e, "t").and_then(|v| v.parse().ok()),
                                    d: attr(&e, "d").and_then(|v| v.parse().ok()).unwrap_or(0),
                                    r: attr(&e, "r").and_then(|v| v.parse().ok()).unwrap_or(0),
                                });
                            }
                        }
                    }
                    _ => {}
                }
            }
            Event::End(e) => {
                depth -= 1;
                match local_name(e.name()).as_str() {
                    "BaseURL" => base_text = None,
                    "Representation" => {
                        if let Some(r) = current_repr.take() {
                            mpd.representations.push(r);
                        }
                    }
                    _ => {}
                }
            }
            _ => {}
        }
    }
    if depth != 0 {
        return Err(Error::Mpd("unclosed element at end of document".into()));
    }
    Ok(mpd)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NUMBER_MPD: &str = r#"<?xml version="1.0"?>
<MPD xmlns="urn:mpeg:dash:schema:mpd:2011" mediaPresentationDuration="PT8S">
  <BaseURL>https://cdn.example/track/</BaseURL>
  <Period>
    <AdaptationSet>
      <Representation id="audio" codecs="flac" bandwidth="1500000" audioSamplingRate="44100" bitDepth="16">
        <SegmentTemplate initialization="init-$RepresentationID$.mp4" media="seg-$Number$.mp4" startNumber="1" duration="4" timescale="1"/>
      </Representation>
    </AdaptationSet>
  </Period>
</MPD>"#;

    const TIMELINE_MPD: &str = r#"<MPD><Period><AdaptationSet>
      <Representation id="a" codecs="flac">
        <SegmentTemplate initialization="init.mp4" media="$Time$.mp4" timescale="44100">
          <SegmentTimeline><S d="176400" r="2"/></SegmentTimeline>
        </SegmentTemplate>
      </Representation>
    </AdaptationSet></Period></MPD>"#;

    #[test]
    fn parses_number_template_and_counts_segments() {
        let mpd = parse(NUMBER_MPD).unwrap();
        assert_eq!(mpd.base_urls, vec!["https://cdn.example/track/"]);
        let r = mpd.audio_representation().unwrap();
        assert_eq!(r.codecs.as_deref(), Some("flac"));
        assert_eq!(r.sample_rate, Some(44100));
        assert_eq!(r.bit_depth, Some(16));
        let duration = mpd.media_presentation_duration.as_deref();
        assert_eq!(r.segment_count(duration).unwrap(), 2);
        assert_eq!(
            r.media_urls(duration, &mpd.base_urls).unwrap(),
            vec![
                "https://cdn.example/track/seg-1.mp4",
                "https://cdn.example/track/seg-2.mp4",
            ]
        );
        assert_eq!(
            r.init_url(&mpd.base_urls).as_deref(),
            Some("https://cdn.example/track/init-audio.mp4")
        );
    }

    #[test]
    fn parses_timeline_and_counts_segments() {
        let mpd = parse(TIMELINE_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        assert_eq!(r.segment_count(None).unwrap(), 3);
        assert_eq!(
            r.media_urls(None, &[]).unwrap(),
            vec!["0.mp4", "176400.mp4", "352800.mp4"]
        );
    }

    #[test]
    fn segment_timings_match_number_template_durations() {
        let mpd = parse(NUMBER_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        let timings = r.segment_timings(mpd.media_presentation_duration.as_deref()).unwrap();
        assert_eq!(
            timings,
            vec![
                SegmentTiming { start_ticks: 0, duration_ticks: 4 },
                SegmentTiming { start_ticks: 4, duration_ticks: 4 },
            ]
        );
    }

    #[test]
    fn segment_timings_match_timeline_offsets() {
        let mpd = parse(TIMELINE_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        let timings = r.segment_timings(None).unwrap();
        assert_eq!(
            timings,
            vec![
                SegmentTiming { start_ticks: 0, duration_ticks: 176_400 },
                SegmentTiming { start_ticks: 176_400, duration_ticks: 176_400 },
                SegmentTiming { start_ticks: 352_800, duration_ticks: 176_400 },
            ]
        );
    }

    const REPEAT_MPD: &str = r#"<MPD mediaPresentationDuration="PT8S"><Period><AdaptationSet>
      <Representation id="a" codecs="flac">
        <SegmentTemplate initialization="init.mp4" media="$Time$.mp4" timescale="1">
          <SegmentTimeline><S t="0" d="4" r="-1"/></SegmentTimeline>
        </SegmentTemplate>
      </Representation>
    </AdaptationSet></Period></MPD>"#;

    #[test]
    fn repeat_minus_one_fills_until_duration() {
        let mpd = parse(REPEAT_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        assert_eq!(r.segment_count(Some("PT8S")).unwrap(), 2);
        assert_eq!(
            r.media_urls(Some("PT8S"), &[]).unwrap(),
            vec!["0.mp4", "4.mp4"]
        );
    }

    #[test]
    fn repeat_minus_one_without_duration_yields_one_segment() {
        let mpd = parse(REPEAT_MPD).unwrap();
        let r = mpd.audio_representation().unwrap();
        assert_eq!(r.segment_count(None).unwrap(), 1);
        assert_eq!(r.media_urls(None, &[]).unwrap(), vec!["0.mp4"]);
    }

    #[test]
    fn malformed_xml_is_a_typed_error() {
        assert!(matches!(parse("<MPD><Period>"), Err(Error::Mpd(_))));
    }

    #[test]
    fn missing_template_errors() {
        let mpd =
            parse("<MPD><Period><AdaptationSet><Representation/></AdaptationSet></Period></MPD>")
                .unwrap();
        let r = mpd.audio_representation().unwrap();
        assert!(matches!(r.segment_count(None), Err(Error::Manifest(_))));
    }
}
