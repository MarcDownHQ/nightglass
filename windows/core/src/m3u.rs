use std::collections::HashMap;

use url::Url;

use crate::{Catalog, Channel, Http, Result};

pub struct Playlist {
    pub channels: Vec<Channel>,
    pub epg_url: Option<Url>,
}

pub fn parse(text: &str) -> Playlist {
    let mut channels = Vec::new();
    let mut epg = None;
    let mut info: Option<(HashMap<String, String>, String)> = None;
    let mut headers = HashMap::new();
    let mut extgrp: Option<String> = None;

    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    for raw in text.split(['\n', '\r']) {
        let line = raw.trim_matches([' ', '\t']);
        if line.is_empty() {
            continue;
        }
        if line.starts_with("#EXTM3U") {
            let a = attributes(line);
            epg = a
                .get("url-tvg")
                .or_else(|| a.get("x-tvg-url"))
                .and_then(|v| v.split(',').next())
                .and_then(|v| Url::parse(v).ok());
        } else if line.starts_with("#EXTINF") {
            info = Some((attributes(line), title(line)));
            headers = HashMap::new();
            extgrp = None;
        } else if let Some(opt) = line.strip_prefix("#EXTVLCOPT:") {
            let Some((key, value)) = opt.split_once('=') else { continue };
            match key {
                "http-user-agent" => headers.insert("User-Agent".to_string(), value.to_string()),
                "http-referrer" | "http-referer" => headers.insert("Referer".to_string(), value.to_string()),
                _ => None,
            };
        } else if let Some(group) = line.strip_prefix("#EXTGRP:") {
            extgrp = Some(group.to_string());
        } else if !line.starts_with('#')
            && let Some((a, title)) = &info
            && let Ok(stream) = Url::parse(line)
        {
            let group = a.get("group-title").or(extgrp.as_ref()).and_then(|g| g.split(';').next()).filter(|g| !g.is_empty());
            let name = if title.is_empty() {
                a.get("tvg-name")
                    .cloned()
                    .unwrap_or_else(|| stream.path_segments().and_then(|mut s| s.next_back()).unwrap_or_default().to_string())
            } else {
                title.clone()
            };
            channels.push(Channel {
                id: stream.to_string(),
                name,
                number: a.get("tvg-chno").and_then(|n| n.parse().ok()),
                group: group.map(str::to_string),
                logo: a.get("tvg-logo").and_then(|l| Url::parse(l).ok()),
                epg_id: a.get("tvg-id").filter(|id| !id.is_empty()).cloned(),
                stream,
                headers: std::mem::take(&mut headers),
            });
            info = None;
        }
    }
    Playlist { channels, epg_url: epg }
}

/// `key="value"` pairs of an `#EXTINF` / `#EXTM3U` line, keys lowercased.
fn attributes(line: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
    let mut i = 0;
    while let Some(eq) = line[i..].find("=\"").map(|p| p + i) {
        let key_start = line[..eq]
            .rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
            .map_or(0, |p| p + 1);
        let value_start = eq + 2;
        let Some(len) = line[value_start..].find('"') else { break };
        if key_start < eq {
            out.insert(line[key_start..eq].to_ascii_lowercase(), line[value_start..value_start + len].to_string());
        }
        i = value_start + len + 1;
    }
    out
}

/// Display name: everything after the first comma that sits outside quotes.
fn title(line: &str) -> String {
    let mut in_quotes = false;
    for (i, c) in line.char_indices() {
        match c {
            '"' => in_quotes = !in_quotes,
            ',' if !in_quotes => return line[i + 1..].trim_matches([' ', '\t']).to_string(),
            _ => {}
        }
    }
    String::new()
}

pub struct M3uSource {
    pub url: Url,
    pub epg: Option<Url>,
    http: Http,
}

impl M3uSource {
    pub fn new(url: Url, epg: Option<Url>, http: Http) -> Self {
        M3uSource { url, epg, http }
    }

    pub async fn catalog(&self) -> Result<Catalog> {
        let data = self.http.fetch(&self.url).await?;
        let playlist = parse(&String::from_utf8_lossy(&data));
        // shortcut: every M3U entry is treated as live TV; VOD inside M3U playlists is not split out.
        Ok(Catalog { channels: playlist.channels, epg_url: self.epg.clone().or(playlist.epg_url), ..Default::default() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iptv_org_style() {
        let text = "#EXTM3U x-tvg-url=\"https://epg.example/guide.xml.gz,https://other.example/g.xml\"\n\
            #EXTINF:-1 tvg-id=\"6ter.fr@SD\" tvg-logo=\"https://i.imgur.com/2nd5Cox.png\" group-title=\"Entertainment\",6ter\n\
            http://149.62.177.107:9999/6ter/index.m3u8\n\
            #EXTINF:-1 tvg-id=\"Africa24Sport.fr@SD\" group-title=\"News;Sports\",Africa 24, Sport (1080p)\n\
            #EXTVLCOPT:http-user-agent=Mozilla/5.0 (X11)\n\
            #EXTVLCOPT:http-referrer=https://ref.example/\n\
            https://example.com/a24.m3u8\r\n\
            #EXTINF:-1,No attributes\n\
            #EXTGRP:Misc\n\
            https://example.com/plain.ts\n\
            #EXTINF:-1 tvg-name=\"Orphan\",\n\
            not a url\n";
        let r = parse(text);
        assert_eq!(r.epg_url.unwrap().as_str(), "https://epg.example/guide.xml.gz");
        assert_eq!(r.channels.len(), 3);
        assert_eq!(r.channels[0].name, "6ter");
        assert_eq!(r.channels[0].epg_id.as_deref(), Some("6ter.fr@SD"));
        assert_eq!(r.channels[0].group.as_deref(), Some("Entertainment"));
        assert_eq!(r.channels[0].logo.as_ref().unwrap().as_str(), "https://i.imgur.com/2nd5Cox.png");
        assert_eq!(r.channels[1].name, "Africa 24, Sport (1080p)");
        assert_eq!(r.channels[1].group.as_deref(), Some("News"));
        assert_eq!(
            r.channels[1].headers,
            HashMap::from([
                ("User-Agent".to_string(), "Mozilla/5.0 (X11)".to_string()),
                ("Referer".to_string(), "https://ref.example/".to_string()),
            ])
        );
        assert_eq!(r.channels[2].group.as_deref(), Some("Misc"));
        assert!(r.channels[2].headers.is_empty());
    }

    #[test]
    fn title_ignores_commas_in_quotes() {
        assert_eq!(title(r#"#EXTINF:-1 group-title="A, B",Name"#), "Name");
    }

    #[test]
    fn byte_order_mark_does_not_hide_the_header() {
        let r = parse("\u{feff}#EXTM3U url-tvg=\"https://g.example/x.xml\"\n");
        assert_eq!(r.epg_url.unwrap().as_str(), "https://g.example/x.xml");
    }
}
