//! HLS playlists (RFC 8216) of protected fMP4 streams: their Widevine keys, and the same playlists without them,
//! every playlist and segment named through the relay.

use nightglass_drm::DrmError;
use nightglass_drm::pssh::{Pssh, WIDEVINE};
use url::Url;

use super::dash::{ByteRange, Source};

const WIDEVINE_KEYFORMAT: &str = "urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed";

pub(super) fn is_playlist(text: &str) -> bool {
    text.trim_start_matches('\u{feff}').starts_with("#EXTM3U")
}

/// What a rewritten playlist names, for the relay to number.
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Named {
    Playlist(Url),
    /// An init or media segment of the track whose init segment is `track`.
    Segment { url: Url, range: Option<ByteRange>, track: Source },
    /// An AES-128 key, or a segment of a clear stream.
    Plain(Url),
}

/// The value of attribute `name` in a tag's attribute list, unquoted.
pub(super) fn attribute(line: &str, name: &str) -> Option<String> {
    let list = line.split_once(':')?.1;
    let mut rest = list;
    while !rest.is_empty() {
        let (key, after) = rest.split_once('=')?;
        let (value, next) = match after.strip_prefix('"') {
            Some(quoted) => {
                let end = quoted.find('"')?;
                (&quoted[..end], quoted[end + 1..].trim_start_matches(','))
            }
            None => after.split_once(',').unwrap_or((after, "")),
        };
        if key.trim().eq_ignore_ascii_case(name) {
            return Some(value.to_string());
        }
        rest = next;
    }
    None
}

fn is_key(line: &str) -> bool {
    line.starts_with("#EXT-X-KEY:") || line.starts_with("#EXT-X-SESSION-KEY:")
}

fn is_widevine(line: &str) -> bool {
    attribute(line, "KEYFORMAT").is_some_and(|f| f.eq_ignore_ascii_case(WIDEVINE_KEYFORMAT))
}

/// The Widevine `pssh` boxes of the playlist's keys, from their `data:` URIs.
pub(super) fn widevine_pssh(text: &str) -> Vec<Pssh> {
    text.lines()
        .map(str::trim)
        .filter(|l| is_key(l) && is_widevine(l))
        .filter_map(|l| attribute(l, "URI"))
        .filter_map(|uri| uri.strip_prefix("data:").and_then(|d| d.split_once("base64,")).map(|(_, b)| b.to_string()))
        .filter_map(|b| Pssh::from_base64(&b, WIDEVINE).ok())
        .collect()
}

/// The first playlist a master playlist names.
pub(super) fn first_variant(text: &str, base: &Url) -> Option<Url> {
    let mut lines = text.lines().map(str::trim);
    while let Some(line) = lines.next() {
        if line.starts_with("#EXT-X-STREAM-INF") {
            let uri = lines.find(|l| !l.is_empty() && !l.starts_with('#'))?;
            return base.join(uri).ok();
        }
    }
    None
}

/// The init segments a media playlist names.
pub(super) fn maps(text: &str, base: &Url) -> Vec<Source> {
    text.lines().map(str::trim).filter(|l| l.starts_with("#EXT-X-MAP:")).filter_map(|l| map(l, base)).collect()
}

fn map(line: &str, base: &Url) -> Option<Source> {
    let url = base.join(&attribute(line, "URI")?).ok()?;
    let range = attribute(line, "BYTERANGE").and_then(|r| byte_range(&r, 0));
    Some((url, range))
}

/// `n[@o]`, `o` defaulting to `next`.
fn byte_range(text: &str, next: u64) -> Option<ByteRange> {
    let (length, offset) = match text.split_once('@') {
        Some((length, offset)) => (length.trim().parse::<u64>().ok()?, offset.trim().parse().ok()?),
        None => (text.trim().parse().ok()?, next),
    };
    (length > 0).then(|| (offset, offset + length - 1))
}

/// The playlist without DRM keys, each address it names given by `local`. Byte ranges become addresses of their
/// own, so that every segment is a whole resource for the player.
pub(super) fn rewrite(text: &str, base: &Url, local: &mut dyn FnMut(Named) -> Url) -> Result<String, DrmError> {
    let mut out = String::with_capacity(text.len() * 2);
    let (mut protected, mut variant) = (false, false);
    let mut track: Option<Source> = None;
    let (mut pending, mut next_offset): (Option<String>, u64) = (None, 0);
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim_end_matches('\r');
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push_str(line);
        } else if trimmed.starts_with("#EXT-X-SESSION-KEY:") || trimmed.starts_with("#EXT-X-I-FRAME-STREAM-INF:") || trimmed.starts_with("#EXT-X-PART:") || trimmed.starts_with("#EXT-X-PRELOAD-HINT:") {
            continue;
        } else if trimmed.starts_with("#EXT-X-KEY:") {
            let method = attribute(trimmed, "METHOD").unwrap_or_default().to_ascii_uppercase();
            match method.as_str() {
                "NONE" => {
                    protected = false;
                    out.push_str(trimmed);
                }
                "AES-128" => {
                    protected = false;
                    out.push_str(&with_uri(trimmed, base, |url| local(Named::Plain(url))));
                }
                _ => {
                    protected = true;
                    continue;
                }
            }
        } else if trimmed.starts_with("#EXT-X-MAP:") {
            let Some(source) = map(trimmed, base) else { continue };
            track = Some(source.clone());
            let address = local(Named::Segment { url: source.0.clone(), range: source.1, track: source });
            out.push_str(&format!("#EXT-X-MAP:URI=\"{address}\""));
        } else if let Some(range) = trimmed.strip_prefix("#EXT-X-BYTERANGE:") {
            pending = Some(range.to_string());
            continue;
        } else if trimmed.starts_with("#EXT-X-STREAM-INF:") {
            variant = true;
            out.push_str(trimmed);
        } else if trimmed.starts_with("#EXT-X-MEDIA:") {
            out.push_str(&with_uri(trimmed, base, |url| local(Named::Playlist(url))));
        } else if trimmed.starts_with('#') {
            out.push_str(trimmed);
        } else {
            let Ok(url) = base.join(trimmed) else { continue };
            let named = if variant {
                variant = false;
                Named::Playlist(url)
            } else {
                let range = pending.take().and_then(|r| byte_range(&r, next_offset));
                if let Some((_, end)) = range {
                    next_offset = end + 1;
                }
                match &track {
                    Some(track) => Named::Segment { url, range, track: track.clone() },
                    None if protected => return Err(DrmError::Unsupported("HLS in MPEG-TS with SAMPLE-AES".into())),
                    None => Named::Plain(url),
                }
            };
            out.push_str(local(named).as_str());
        }
        out.push('\n');
    }
    Ok(out)
}

/// The tag with its `URI` attribute replaced.
fn with_uri(line: &str, base: &Url, mut local: impl FnMut(Url) -> Url) -> String {
    const MARK: &str = "URI=\"";
    let Some(start) = line.find(MARK).map(|s| s + MARK.len()) else { return line.to_string() };
    let Some(end) = line[start..].find('"').map(|e| e + start) else { return line.to_string() };
    match base.join(&line[start..end]) {
        Ok(url) if matches!(url.scheme(), "http" | "https") => format!("{}{}{}", &line[..start], local(url), &line[end..]),
        _ => line.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PSSH: &str = "AAAAMnBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAABISEAUFBQUFBQUFBQUFBQUFBQU=";

    fn base() -> Url {
        Url::parse("https://cdn.test/hls/master.m3u8").unwrap()
    }

    fn numbered(named: &mut Vec<Named>) -> impl FnMut(Named) -> Url + '_ {
        |n| {
            named.push(n);
            Url::parse(&format!("http://127.0.0.1:1/t/p{}", named.len() - 1)).unwrap()
        }
    }

    #[test]
    fn attributes_read_quoted_or_not() {
        let line = r#"#EXT-X-KEY:METHOD=SAMPLE-AES,URI="data:text/plain;base64,QQ==",KEYFORMAT="urn:uuid:EDEF8BA9-79D6-4ACE-A3C8-27DCD51D21ED",KEYFORMATVERSIONS="1""#;
        assert_eq!(attribute(line, "method").as_deref(), Some("SAMPLE-AES"));
        assert_eq!(attribute(line, "URI").as_deref(), Some("data:text/plain;base64,QQ=="));
        assert_eq!(attribute(line, "KEYFORMATVERSIONS").as_deref(), Some("1"));
        assert!(is_widevine(line));
        assert_eq!(attribute(line, "IV"), None);
    }

    #[test]
    fn master_playlists_lose_their_keys_and_name_their_variants() {
        let master = format!(
            "#EXTM3U\n#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES-CTR,URI=\"data:text/plain;base64,{PSSH}\",KEYFORMAT=\"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed\"\n#EXT-X-SESSION-KEY:METHOD=SAMPLE-AES,URI=\"skd://fairplay\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",URI=\"audio/fr.m3u8\"\n#EXT-X-STREAM-INF:BANDWIDTH=1,AUDIO=\"a\"\nvideo/720.m3u8\n#EXT-X-I-FRAME-STREAM-INF:URI=\"iframes.m3u8\"\n"
        );
        assert_eq!(widevine_pssh(&master).len(), 1);
        assert_eq!(widevine_pssh(&master)[0].kids(), vec![[5; 16]]);
        assert_eq!(first_variant(&master, &base()).unwrap().as_str(), "https://cdn.test/hls/video/720.m3u8");
        let mut named = Vec::new();
        let out = rewrite(&master, &base(), &mut numbered(&mut named)).unwrap();
        assert_eq!(out, "#EXTM3U\n#EXT-X-MEDIA:TYPE=AUDIO,GROUP-ID=\"a\",URI=\"http://127.0.0.1:1/t/p0\"\n#EXT-X-STREAM-INF:BANDWIDTH=1,AUDIO=\"a\"\nhttp://127.0.0.1:1/t/p1\n");
        assert_eq!(named, vec![Named::Playlist(base().join("audio/fr.m3u8").unwrap()), Named::Playlist(base().join("video/720.m3u8").unwrap())]);
    }

    #[test]
    fn media_playlists_name_each_segment_with_its_init() {
        let media = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"data:text/plain;base64,AAAA\",KEYFORMAT=\"urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed\"\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXT-X-MAP:URI=\"v.mp4\",BYTERANGE=\"800@0\"\n#EXTINF:2,\n#EXT-X-BYTERANGE:1000@800\nv.mp4\n#EXTINF:2,\n#EXT-X-BYTERANGE:500\nv.mp4\n#EXT-X-ENDLIST\n";
        let mut named = Vec::new();
        let out = rewrite(media, &base(), &mut numbered(&mut named)).unwrap();
        assert_eq!(out, "#EXTM3U\n#EXT-X-MAP:URI=\"http://127.0.0.1:1/t/p0\"\n#EXTINF:2,\nhttp://127.0.0.1:1/t/p1\n#EXTINF:2,\nhttp://127.0.0.1:1/t/p2\n#EXT-X-ENDLIST\n");
        let v = base().join("v.mp4").unwrap();
        let track = (v.clone(), Some((0, 799)));
        assert_eq!(
            named,
            vec![
                Named::Segment { url: v.clone(), range: Some((0, 799)), track: track.clone() },
                Named::Segment { url: v.clone(), range: Some((800, 1799)), track: track.clone() },
                Named::Segment { url: v, range: Some((1800, 2299)), track },
            ]
        );
        assert_eq!(maps(media, &base()), vec![(base().join("v.mp4").unwrap(), Some((0, 799)))]);
    }

    #[test]
    fn clear_keys_and_segments_stay_as_hls_has_them() {
        let media = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x1\n#EXTINF:2,\na.ts\n";
        let mut named = Vec::new();
        let out = rewrite(media, &base(), &mut numbered(&mut named)).unwrap();
        assert_eq!(out, "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"http://127.0.0.1:1/t/p0\",IV=0x1\n#EXTINF:2,\nhttp://127.0.0.1:1/t/p1\n");
        assert_eq!(named, vec![Named::Plain(base().join("key.bin").unwrap()), Named::Plain(base().join("a.ts").unwrap())]);
    }

    #[test]
    fn sample_aes_in_transport_streams_is_refused_by_name() {
        let media = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://x\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXTINF:2,\na.ts\n";
        let refused = rewrite(media, &base(), &mut |_| Url::parse("http://127.0.0.1/").unwrap());
        assert_eq!(refused, Err(DrmError::Unsupported("HLS in MPEG-TS with SAMPLE-AES".into())));
    }
}
