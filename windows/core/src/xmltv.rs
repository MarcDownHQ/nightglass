use std::collections::{HashMap, HashSet};
use std::io::BufRead;
use std::ops::RangeInclusive;

use flate2::bufread::MultiGzDecoder;
use quick_xml::XmlVersion;
use quick_xml::escape::resolve_predefined_entity;
use quick_xml::events::{BytesStart, Event};
use url::Url;

use crate::{Http, Programme, Result};

/// Downloads (or reads) a guide and keeps the programmes inside `window` (Unix seconds).
/// Parsing is CPU-bound: call it off the async runtime for big guides.
pub async fn load(url: &Url, window: RangeInclusive<i64>, channels: Option<&HashSet<String>>, http: &Http) -> Result<Vec<Programme>> {
    Ok(parse(&http.fetch(url).await?, window, channels))
}

/// Streaming XMLTV parser. Guides run to hundreds of MB, so only programmes inside `window` are kept.
/// `.xml.gz` guides are inflated on the fly.
pub fn parse(data: &[u8], window: RangeInclusive<i64>, channels: Option<&HashSet<String>>) -> Vec<Programme> {
    if data.len() > 18 && data[0] == 0x1f && data[1] == 0x8b {
        read(std::io::BufReader::new(MultiGzDecoder::new(data)), &window, channels)
    } else {
        read(data, &window, channels)
    }
}

#[derive(PartialEq)]
enum Field {
    Title,
    Desc,
}

fn read<R: BufRead>(input: R, window: &RangeInclusive<i64>, wanted: Option<&HashSet<String>>) -> Vec<Programme> {
    let mut reader = quick_xml::Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut current: Option<(String, i64, i64)> = None;
    let (mut title, mut desc) = (String::new(), String::new());
    let mut field = None;

    loop {
        let text = match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => {
                match e.local_name().as_ref() {
                    "programme" => {
                        current = programme(&e, window, wanted);
                        title.clear();
                        desc.clear();
                    }
                    // Guides repeat <title> per language: keep the first.
                    "title" if current.is_some() && title.is_empty() => field = Some(Field::Title),
                    "desc" if current.is_some() && desc.is_empty() => field = Some(Field::Desc),
                    _ => {}
                }
                None
            }
            Ok(Event::End(e)) => {
                match e.local_name().as_ref() {
                    "title" | "desc" => field = None,
                    "programme" => {
                        if let Some((channel, start, end)) = current.take() {
                            let desc = desc.trim();
                            out.push(Programme {
                                channel,
                                start,
                                end,
                                title: title.trim().to_string(),
                                desc: (!desc.is_empty()).then(|| desc.to_string()),
                            });
                        }
                    }
                    _ => {}
                }
                None
            }
            Ok(Event::Text(t)) if field.is_some() => Some(t.xml10_content().into_owned()),
            Ok(Event::CData(t)) if field.is_some() => Some(t.xml10_content().into_owned()),
            Ok(Event::GeneralRef(r)) if field.is_some() => {
                if r.is_char_ref() {
                    r.resolve_char_ref().ok().flatten().map(String::from)
                } else {
                    resolve_predefined_entity(&r.xml10_content()).map(String::from)
                }
            }
            // A broken guide keeps what was read so far.
            Ok(Event::Eof) | Err(_) => break,
            _ => None,
        };
        if let Some(text) = text {
            match field {
                Some(Field::Title) => title.push_str(&text),
                Some(Field::Desc) => desc.push_str(&text),
                None => {}
            }
        }
        buf.clear();
    }
    out
}

/// `parse` on a guide file, read as it goes rather than loaded whole. Gzip is detected by its magic bytes.
pub fn parse_file(path: &std::path::Path, window: RangeInclusive<i64>, channels: Option<&HashSet<String>>) -> std::io::Result<Vec<Programme>> {
    let mut input = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(path)?);
    Ok(if input.fill_buf()?.starts_with(&[0x1f, 0x8b]) {
        read(std::io::BufReader::new(MultiGzDecoder::new(input)), &window, channels)
    } else {
        read(input, &window, channels)
    })
}

/// `channels` on a guide file.
pub fn channels_file(path: &std::path::Path) -> std::io::Result<Vec<(String, String)>> {
    let mut input = std::io::BufReader::with_capacity(1 << 16, std::fs::File::open(path)?);
    Ok(if input.fill_buf()?.starts_with(&[0x1f, 0x8b]) {
        read_channels(std::io::BufReader::new(MultiGzDecoder::new(input)))
    } else {
        read_channels(input)
    })
}

/// The channels a guide describes, as (id, first display name), in guide order: what a channel
/// without a guide can be matched with by hand.
pub fn channels(data: &[u8]) -> Vec<(String, String)> {
    if data.len() > 18 && data[0] == 0x1f && data[1] == 0x8b {
        read_channels(std::io::BufReader::new(MultiGzDecoder::new(data)))
    } else {
        read_channels(data)
    }
}

fn read_channels<R: BufRead>(input: R) -> Vec<(String, String)> {
    let mut reader = quick_xml::Reader::from_reader(input);
    let mut buf = Vec::new();
    let mut out = Vec::new();
    let mut id: Option<String> = None;
    let mut name = String::new();
    let mut in_name = false;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                "channel" => {
                    id = e.try_get_attribute("id").ok().flatten().and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok()).map(|v| v.into_owned());
                    name.clear();
                }
                "display-name" if id.is_some() && name.is_empty() => in_name = true,
                _ => {}
            },
            // `<channel id="x"/>`: no name, the id stands for it.
            Ok(Event::Empty(e)) if e.local_name().as_ref() == "channel" => {
                if let Some(id) = e.try_get_attribute("id").ok().flatten().and_then(|a| a.normalized_value(XmlVersion::Implicit1_0).ok()) {
                    out.push((id.to_string(), id.into_owned()));
                }
            }
            Ok(Event::Text(t)) if in_name => name.push_str(&t.xml10_content()),
            Ok(Event::End(e)) => match e.local_name().as_ref() {
                "display-name" => in_name = false,
                "channel" => {
                    if let Some(id) = id.take() {
                        let label = if name.trim().is_empty() { id.clone() } else { name.trim().to_string() };
                        out.push((id, label));
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) | Err(_) => break,
            _ => {}
        }
        buf.clear();
    }
    out
}

fn programme(e: &BytesStart, window: &RangeInclusive<i64>, wanted: Option<&HashSet<String>>) -> Option<(String, i64, i64)> {
    let mut channel = None;
    let (mut start, mut end) = (None, None);
    for a in e.attributes().flatten() {
        let value = a.normalized_value(XmlVersion::Implicit1_0).ok()?;
        match a.key.local_name().as_ref() {
            "channel" => channel = Some(value.into_owned()),
            "start" => start = date(&value),
            "stop" => end = date(&value),
            _ => {}
        }
    }
    let (channel, start, end) = (channel?, start?, end?);
    let keep = wanted.is_none_or(|w| w.contains(&channel)) && end >= *window.start() && start <= *window.end();
    keep.then_some((channel, start, end))
}

/// `20261104200000 +0100` in Unix seconds; without an offset the time is UTC.
pub(crate) fn date(s: &str) -> Option<i64> {
    let mut parts = s.split(' ').filter(|p| !p.is_empty());
    let first = parts.next()?;
    let digits = first.get(..14)?;
    if !digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: i64 = digits.parse().ok()?;
    let (y, mo, d) = (n / 10_000_000_000, n / 100_000_000 % 100, n / 1_000_000 % 100);
    let (h, mi, se) = (n / 10_000 % 100, n / 100 % 100, n % 100);
    // Some guides glue the offset to the time: `20261104200000+0100`.
    let offset = parts
        .next()
        .or_else(|| Some(&first[14..]).filter(|o| !o.is_empty()))
        .and_then(|o| o.trim_start_matches('+').parse::<i64>().ok())
        .map_or(0, |o| o / 100 * 3600 + o % 100 * 60);
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se - offset)
}

/// Days since 1970-01-01 in the proleptic Gregorian calendar (Howard Hinnant's algorithm).
pub(crate) fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The civil date of a day count since 1970-01-01: the inverse of `days_from_civil`.
pub(crate) fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

#[derive(Debug, Default, PartialEq)]
pub struct NowNext<'a> {
    pub now: Option<&'a Programme>,
    pub next: Option<&'a Programme>,
}

/// Now and next per channel ID, at `at` (Unix seconds).
pub fn now_next(programmes: &[Programme], at: i64) -> HashMap<&str, NowNext<'_>> {
    let mut sorted: Vec<&Programme> = programmes.iter().collect();
    sorted.sort_by_key(|p| p.start);
    let mut out: HashMap<&str, NowNext> = HashMap::new();
    for p in sorted {
        let e = out.entry(p.channel.as_str()).or_default();
        if p.start <= at && p.end > at {
            e.now = Some(p);
        } else if p.start > at && e.next.is_none() {
            e.next = Some(p);
        }
    }
    out
}

/// iptv-org ids carry a feed suffix (`6ter.fr@SD`) that guides don't; match ids without it, ignoring case.
pub fn epg_key(id: &str) -> String {
    id.split('@').next().unwrap_or(id).to_lowercase()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<tv>
  <channel id="wire.fr"><display-name>Wire</display-name></channel>
  <programme start="20261104190000 +0100" stop="20261104200000 +0100" channel="wire.fr"><title lang="fr">Le 19/20</title><title lang="en">The 7pm</title><desc>Infos &amp; m&#233;t&#xE9;o</desc></programme>
  <programme start="20261104200000 +0100" stop="20261104210000 +0100" channel="wire.fr"><title><![CDATA[Journal]]></title></programme>
  <programme start="20261110200000 +0100" stop="20261110210000 +0100" channel="wire.fr"><title>Next week</title></programme>
  <programme start="20261104200000 +0100" stop="20261104210000 +0100" channel="other.fr"><title>Other</title></programme>
</tv>
"#;

    fn at() -> i64 {
        date("20261104193000 +0100").unwrap()
    }

    #[test]
    fn parses_window_and_channels() {
        let wanted = HashSet::from(["wire.fr".to_string()]);
        let p = parse(XML.as_bytes(), at()..=at() + 86_400, Some(&wanted));
        assert_eq!(p.iter().map(|p| p.title.as_str()).collect::<Vec<_>>(), ["Le 19/20", "Journal"]);
        assert_eq!(p[0].desc.as_deref(), Some("Infos & météo"));
        assert_eq!(p[0].start, 1_793_815_200); // 2026-11-04 18:00 UTC
        let nn = now_next(&p, at());
        assert_eq!(nn["wire.fr"].now.unwrap().title, "Le 19/20");
        assert_eq!(nn["wire.fr"].next.unwrap().title, "Journal");
    }

    #[test]
    fn gzip() {
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(XML.as_bytes()).unwrap();
        let gz = gz.finish().unwrap();
        assert_eq!(parse(&gz, at()..=at() + 86_400, None).len(), 3);
    }

    #[test]
    fn reads_files_plain_and_gzipped() {
        let dir = std::env::temp_dir();
        let plain = dir.join(format!("ng-xmltv-{}.xml", std::process::id()));
        std::fs::write(&plain, XML).unwrap();
        let gz = dir.join(format!("ng-xmltv-{}.xml.gz", std::process::id()));
        let mut enc = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        enc.write_all(XML.as_bytes()).unwrap();
        std::fs::write(&gz, enc.finish().unwrap()).unwrap();
        for path in [&plain, &gz] {
            assert_eq!(parse_file(path, at()..=at() + 86_400, None).unwrap().len(), 3);
            assert_eq!(channels_file(path).unwrap().len(), 1);
            std::fs::remove_file(path).unwrap();
        }
    }

    #[test]
    fn lists_channels() {
        assert_eq!(channels(XML.as_bytes()), [("wire.fr".to_string(), "Wire".to_string())]);
        // Some guides put each channel's programmes right after it.
        let bare = r#"<tv><channel id="a.fr"/><programme channel="a.fr" start="20261104190000" stop="20261104200000"><title>T</title></programme><channel id="b.fr"><display-name>B</display-name></channel></tv>"#;
        assert_eq!(channels(bare.as_bytes()), [("a.fr".to_string(), "a.fr".to_string()), ("b.fr".to_string(), "B".to_string())]);
    }

    #[test]
    fn dates() {
        assert_eq!(date("20261104180000"), Some(1_793_815_200));
        assert_eq!(date("20261104190000 +0100"), Some(1_793_815_200));
        assert_eq!(date("20261104190000+0100"), Some(1_793_815_200));
        assert_eq!(date("20261104133000 -0430"), Some(1_793_815_200));
        assert_eq!(date("garbage"), None);
    }

    #[test]
    fn keys_ignore_feed_and_case() {
        assert_eq!(epg_key("6ter.fr@SD"), "6ter.fr");
        assert_eq!(epg_key("TF1.fr"), "tf1.fr");
    }
}
