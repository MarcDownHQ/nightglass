//! Between the WIT records and the core models. Entries a plugin sends with an unreadable stream
//! address are dropped rather than failing the whole catalog.

use nightglass_core::{Account, Browser, Catalog, Channel, Config, Episode, Field, FieldKind, IptvError, Media, Movie, Playback, Preset, Programme, ProviderInfo, Relay, Result, Series};
use url::Url;

use crate::wit;

pub fn config(config: &Config) -> wit::Config {
    config.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
}

pub fn info(info: wit::ProviderInfo) -> std::result::Result<ProviderInfo, String> {
    let valid = !info.id.is_empty() && info.id.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    if !valid {
        return Err(format!("invalid provider id \"{}\": use lowercase letters, digits and -", info.id));
    }
    if let Some(key) = duplicate(info.fields.iter().map(|f| &f.key)) {
        return Err(format!("two fields named \"{key}\""));
    }
    if let Some(label) = duplicate(info.presets.iter().map(|p| &p.label)) {
        return Err(format!("two presets named \"{label}\""));
    }
    Ok(ProviderInfo {
        id: info.id,
        name: info.name,
        fields: info
            .fields
            .into_iter()
            .map(|f| Field {
                key: f.key,
                label: f.label,
                kind: match f.kind {
                    wit::FieldKind::Text => FieldKind::Text,
                    wit::FieldKind::Secret => FieldKind::Secret,
                    wit::FieldKind::Url => FieldKind::Url,
                    wit::FieldKind::Location => FieldKind::Location,
                    wit::FieldKind::Mac => FieldKind::Mac,
                },
                required: f.required,
                placeholder: f.placeholder,
                default: f.default,
            })
            .collect(),
        resolves_streams: info.resolves_streams,
        presets: info.presets.into_iter().map(|p| Preset { label: p.label, name: p.name, config: p.config.into_iter().collect() }).collect(),
    })
}

fn duplicate<'a>(items: impl Iterator<Item = &'a String>) -> Option<&'a String> {
    let mut seen = std::collections::HashSet::new();
    items.into_iter().find(|i| !seen.insert(*i))
}

const STREAM_SCHEMES: [&str; 9] = ["http", "https", "rtmp", "rtmps", "rtsp", "rtp", "udp", "srt", "mms"];

/// A playable address: never `file:` or anything else mpv would open outside the network.
pub fn stream(s: &str) -> Result<Url> {
    Url::parse(s.trim())
        .ok()
        .filter(|u| STREAM_SCHEMES.contains(&u.scheme()))
        .ok_or_else(|| IptvError::BadResponse(format!("invalid stream address from the plugin: {s}")))
}

/// Guides, logos and posters are fetched by the app: http(s) only.
fn optional_url(s: Option<String>) -> Option<Url> {
    s.and_then(|s| Url::parse(s.trim()).ok()).filter(|u| matches!(u.scheme(), "http" | "https"))
}

pub fn catalog(c: wit::Catalog) -> Catalog {
    Catalog {
        channels: c.channels.into_iter().filter_map(channel).collect(),
        movies: c.movies.into_iter().filter_map(movie).collect(),
        series: c.series.into_iter().map(series).collect(),
        epg_url: optional_url(c.epg_url),
        account: c.account.map(|a| Account {
            status: a.status,
            expires: a.expires,
            max_connections: a.max_connections,
            active_connections: a.active_connections,
            trial: a.trial,
        }),
        programmes: c.programmes.into_iter().filter_map(programme).collect(),
    }
}

fn programme(p: wit::Programme) -> Option<Programme> {
    (!p.channel.is_empty() && p.end > p.start).then_some(Programme { channel: p.channel, start: p.start, end: p.end, title: p.title, desc: p.desc })
}

pub fn browser(b: wit::Browser) -> Browser {
    match b {
        wit::Browser::Chrome => Browser::Chrome,
        wit::Browser::Edge => Browser::Edge,
        wit::Browser::Firefox => Browser::Firefox,
        wit::Browser::Safari => Browser::Safari,
    }
}

pub fn playback(p: wit::Playback) -> Result<Playback> {
    let relay = p.relay.map(|r| Relay { browser: r.browser.map(browser), filter: r.filter });
    let url = stream(&p.url)?;
    if relay.is_some() && !matches!(url.scheme(), "http" | "https") {
        return Err(IptvError::BadResponse(format!("only http(s) streams can be relayed: {url}")));
    }
    Ok(Playback { url, headers: p.headers.into_iter().collect(), relay })
}

pub fn to_wit_media(m: Media) -> wit::Media {
    wit::Media { content_type: m.content_type, body: m.body }
}

pub fn media(m: wit::Media) -> Media {
    Media { content_type: m.content_type, body: m.body }
}

fn channel(c: wit::Channel) -> Option<Channel> {
    Some(Channel {
        stream: stream(&c.stream).ok()?,
        id: c.id,
        name: c.name,
        number: c.number,
        group: c.group,
        logo: optional_url(c.logo),
        epg_id: c.epg_id,
        headers: c.headers.into_iter().collect(),
        catchup_days: c.catchup_days,
    })
}

fn movie(m: wit::Movie) -> Option<Movie> {
    Some(Movie { stream: stream(&m.stream).ok()?, id: m.id, name: m.name, group: m.group, poster: optional_url(m.poster), rating: m.rating, plot: m.plot })
}

fn series(s: wit::Series) -> Series {
    Series { id: s.id, name: s.name, group: s.group, poster: optional_url(s.poster), rating: s.rating, plot: s.plot }
}

pub fn episode(e: wit::Episode) -> Option<Episode> {
    Some(Episode { stream: stream(&e.stream).ok()?, id: e.id, title: e.title, season: e.season, number: e.number, plot: e.plot })
}

pub fn to_wit_channel(c: &Channel) -> wit::Channel {
    wit::Channel {
        id: c.id.clone(),
        name: c.name.clone(),
        number: c.number,
        group: c.group.clone(),
        logo: c.logo.as_ref().map(Url::to_string),
        epg_id: c.epg_id.clone(),
        stream: c.stream.to_string(),
        headers: c.headers.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        catchup_days: c.catchup_days,
    }
}

pub fn to_wit_series(s: &Series) -> wit::Series {
    wit::Series {
        id: s.id.clone(),
        name: s.name.clone(),
        group: s.group.clone(),
        poster: s.poster.as_ref().map(Url::to_string),
        rating: s.rating,
        plot: s.plot.clone(),
    }
}

pub fn from_wit_error(e: wit::Error) -> IptvError {
    match e {
        wit::Error::Http(status) => IptvError::Http(status),
        wit::Error::AuthFailed => IptvError::AuthFailed,
        wit::Error::BadResponse(m) => IptvError::BadResponse(m),
        wit::Error::Network(m) => IptvError::Network(m),
        wit::Error::Setting(m) => IptvError::Setting(m),
    }
}

pub fn to_wit_error(e: IptvError) -> wit::Error {
    match e {
        IptvError::Http(status) => wit::Error::Http(status),
        IptvError::AuthFailed => wit::Error::AuthFailed,
        IptvError::BadResponse(m) => wit::Error::BadResponse(m),
        IptvError::Setting(m) => wit::Error::Setting(m),
        other => wit::Error::Network(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(key: &str) -> wit::Field {
        wit::Field { key: key.into(), label: key.into(), kind: wit::FieldKind::Text, required: false, placeholder: None, default: None }
    }

    fn provider(fields: Vec<wit::Field>) -> wit::ProviderInfo {
        wit::ProviderInfo { id: "p".into(), name: "P".into(), fields, resolves_streams: false, presets: Vec::new() }
    }

    #[test]
    fn field_keys_are_unique() {
        assert!(info(provider(vec![field("a"), field("b")])).is_ok());
        assert_eq!(info(provider(vec![field("a"), field("a")])).unwrap_err(), "two fields named \"a\"");
    }

    #[test]
    fn streams_stay_on_the_network() {
        assert!(stream("rtsp://cam.test/live").is_ok());
        assert!(stream("file:///etc/passwd").is_err());
        assert!(stream("file://attacker/share/x.ts").is_err());
        assert_eq!(optional_url(Some("file://attacker/share/epg.xml".into())), None);
        assert!(optional_url(Some("https://s.test/epg.xml".into())).is_some());
    }
}
