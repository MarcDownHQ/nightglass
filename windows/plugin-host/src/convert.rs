//! Between the WIT records and the core models. Entries a plugin sends with an unreadable stream
//! address are dropped rather than failing the whole catalog.

use nightglass_core::{Account, Browser, Catalog, Channel, Chapter, Config, Episode, Field, FieldKind, IptvError, Media, Movie, Playback, Preset, Programme, Prompt, ProviderInfo, Relay, Result, STREAM_SCHEMES, Series, Track};
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

/// With `resolves_streams`, a movie's or an episode's stream is the plugin's own text, given back at play time.
fn vod_stream(s: String, resolves: bool) -> Option<String> {
    if resolves { Some(s) } else { stream(&s).ok().map(String::from) }
}

pub fn catalog(c: wit::Catalog, resolves: bool) -> Catalog {
    Catalog {
        channels: c.channels.into_iter().filter_map(channel).collect(),
        movies: c.movies.into_iter().filter_map(|m| movie(m, resolves)).collect(),
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
    let url = |raw: &str| {
        let url = stream(raw)?;
        if relay.is_some() && !matches!(url.scheme(), "http" | "https") {
            return Err(IptvError::BadResponse(format!("only http(s) streams can be relayed: {url}")));
        }
        Ok(url)
    };
    let tracks = p.tracks.iter().map(|t| Ok(Track { url: url(&t.url)?, language: t.language.clone(), title: t.title.clone() })).collect::<Result<_>>()?;
    let chapters = p.chapters.into_iter().map(|c| Chapter { start: c.start, title: c.title }).collect();
    Ok(Playback { url: url(&p.url)?, headers: p.headers.into_iter().collect(), relay, tracks, chapters })
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
        catchup_window: c.catchup_window.map(i64::from),
    })
}

fn movie(m: wit::Movie, resolves: bool) -> Option<Movie> {
    Some(Movie { stream: vod_stream(m.stream, resolves)?, id: m.id, name: m.name, group: m.group, poster: optional_url(m.poster), rating: m.rating, plot: m.plot, duration: m.duration, available_until: m.available_until })
}

fn series(s: wit::Series) -> Series {
    Series { id: s.id, name: s.name, group: s.group, poster: optional_url(s.poster), rating: s.rating, plot: s.plot }
}

pub fn episode(e: wit::Episode, resolves: bool) -> Option<Episode> {
    Some(Episode { stream: vod_stream(e.stream, resolves)?, id: e.id, title: e.title, season: e.season, number: e.number, plot: e.plot, aired: e.aired, duration: e.duration, thumbnail: optional_url(e.thumbnail), available_until: e.available_until })
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
        catchup_window: c.catchup_window.map(|seconds| u32::try_from(seconds.max(0)).unwrap_or(u32::MAX)),
    }
}

pub fn to_wit_movie(m: &Movie) -> wit::Movie {
    wit::Movie {
        id: m.id.clone(),
        name: m.name.clone(),
        group: m.group.clone(),
        poster: m.poster.as_ref().map(Url::to_string),
        stream: m.stream.clone(),
        rating: m.rating,
        plot: m.plot.clone(),
        duration: m.duration,
        available_until: m.available_until,
    }
}

pub fn to_wit_episode(e: &Episode) -> wit::Episode {
    wit::Episode {
        id: e.id.clone(),
        title: e.title.clone(),
        season: e.season,
        number: e.number,
        stream: e.stream.clone(),
        plot: e.plot.clone(),
        aired: e.aired,
        duration: e.duration,
        thumbnail: e.thumbnail.as_ref().map(Url::to_string),
        available_until: e.available_until,
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

/// How often a shown sign-in prompt is asked again, in seconds: never in a tight loop, never past the browser's timer range.
const RETRY_IN: (u32, u32) = (2, 3600);

pub fn from_wit_error(e: wit::Error) -> IptvError {
    match e {
        wit::Error::Http(status) => IptvError::Http(status),
        wit::Error::AuthFailed => IptvError::AuthFailed,
        wit::Error::BadResponse(m) => IptvError::BadResponse(m),
        wit::Error::Network(m) => IptvError::Network(m),
        wit::Error::Setting(m) => IptvError::Setting(m),
        wit::Error::ActionRequired(p) => IptvError::ActionRequired(Box::new(Prompt { message: p.message, url: optional_url(p.url), code: p.code, retry_in: p.retry_in.map(|seconds| seconds.clamp(RETRY_IN.0, RETRY_IN.1)) })),
    }
}

pub fn to_wit_error(e: IptvError) -> wit::Error {
    match e {
        IptvError::Http(status) => wit::Error::Http(status),
        IptvError::AuthFailed => wit::Error::AuthFailed,
        IptvError::BadResponse(m) => wit::Error::BadResponse(m),
        IptvError::Setting(m) => wit::Error::Setting(m),
        IptvError::ActionRequired(p) => wit::Error::ActionRequired(wit::Prompt { message: p.message, url: p.url.map(String::from), code: p.code, retry_in: p.retry_in }),
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

    fn with_track(url: &str, relay: Option<wit::Relay>) -> wit::Playback {
        let track = wit::Track { url: url.into(), language: Some("fr".into()), title: Some("Français".into()) };
        wit::Playback { url: "https://s.test/video.mp4".into(), headers: Vec::new(), relay, tracks: vec![track], chapters: Vec::new() }
    }

    fn wit_movie(stream: &str) -> wit::Movie {
        wit::Movie { id: "m".into(), name: "M".into(), group: None, poster: None, stream: stream.into(), rating: None, plot: None, duration: None, available_until: None }
    }

    #[test]
    fn opaque_streams_only_for_providers_that_resolve_them() {
        assert_eq!(movie(wit_movie("fr-tv:4711"), true).unwrap().stream, "fr-tv:4711", "kept as given, for movie-url");
        assert!(movie(wit_movie("fr-tv:4711"), false).is_none());
        assert!(movie(wit_movie("file:///etc/passwd"), false).is_none());
        assert_eq!(movie(wit_movie("http://s.test/m.mkv"), false).unwrap().stream, "http://s.test/m.mkv");
        let dated = wit::Episode { id: "e".into(), title: "JT".into(), season: None, number: None, stream: "jt-21-11".into(), plot: None, aired: Some(1_795_300_000), duration: None, thumbnail: None, available_until: None };
        let e = episode(dated, true).unwrap();
        assert_eq!((e.season, e.number, e.aired, e.stream.as_str()), (None, None, Some(1_795_300_000), "jt-21-11"));
        assert_eq!(to_wit_episode(&e).stream, "jt-21-11");
        assert_eq!(to_wit_movie(&movie(wit_movie("fr-tv:4711"), true).unwrap()).stream, "fr-tv:4711");
    }

    #[test]
    fn durations_stills_and_expiry_carried_through() {
        let mut m = wit_movie("http://s.test/m.mkv");
        (m.duration, m.available_until) = (Some(5400), Some(1_796_000_000));
        let kept = movie(m, false).unwrap();
        assert_eq!((kept.duration, kept.available_until), (Some(5400), Some(1_796_000_000)));
        assert_eq!((to_wit_movie(&kept).duration, to_wit_movie(&kept).available_until), (Some(5400), Some(1_796_000_000)));
        let still = |thumbnail: &str| wit::Episode {
            id: "e".into(),
            title: "JT".into(),
            season: None,
            number: None,
            stream: "jt".into(),
            plot: None,
            aired: None,
            duration: Some(1800),
            thumbnail: Some(thumbnail.into()),
            available_until: Some(1_796_000_000),
        };
        let e = episode(still("https://img.test/jt.jpg"), true).unwrap();
        assert_eq!((e.duration, e.thumbnail.as_ref().map(Url::as_str), e.available_until), (Some(1800), Some("https://img.test/jt.jpg"), Some(1_796_000_000)));
        assert_eq!(to_wit_episode(&e).thumbnail.as_deref(), Some("https://img.test/jt.jpg"));
        assert_eq!(episode(still("file:///home/me/still.jpg"), true).unwrap().thumbnail, None, "stills are fetched by the app: http(s) only");
    }

    #[test]
    fn replay_windows_in_seconds() {
        let wit_channel = wit::Channel {
            id: "c".into(),
            name: "C".into(),
            number: None,
            group: None,
            logo: None,
            epg_id: None,
            stream: "http://s.test/c.m3u8".into(),
            headers: Vec::new(),
            catchup_window: Some(4 * 3600),
        };
        let mut c = channel(wit_channel).unwrap();
        assert_eq!(c.catchup_window, Some(4 * 3600));
        assert_eq!(to_wit_channel(&c).catchup_window, Some(4 * 3600));
        c.catchup_window = Some(i64::MAX);
        assert_eq!(to_wit_channel(&c).catchup_window, Some(u32::MAX));
    }

    #[test]
    fn chapters_carried_through() {
        let mut p = with_track("https://s.test/fr.m4a", None);
        p.chapters = vec![wit::Chapter { start: 600.5, title: "Two".into() }, wit::Chapter { start: 0.0, title: "One".into() }];
        let chapters = playback(p).unwrap().chapters;
        assert_eq!(chapters, [Chapter { start: 600.5, title: "Two".into() }, Chapter { start: 0.0, title: "One".into() }], "as given: the app sorts them");
    }

    #[test]
    fn prompts_round_trip() {
        let wit_prompt = |url: &str| wit::Prompt { message: "Sign in".into(), url: Some(url.into()), code: Some("WDJB-MJHT".into()), retry_in: Some(5) };
        let IptvError::ActionRequired(prompt) = from_wit_error(wit::Error::ActionRequired(wit_prompt("https://tv.test/activate"))) else { panic!("not a prompt") };
        assert_eq!(*prompt, Prompt { message: "Sign in".into(), url: Some(Url::parse("https://tv.test/activate").unwrap()), code: Some("WDJB-MJHT".into()), retry_in: Some(5) });
        let wit::Error::ActionRequired(back) = to_wit_error(IptvError::ActionRequired(prompt)) else { panic!("not a prompt") };
        assert_eq!((back.message.as_str(), back.url.as_deref(), back.code.as_deref(), back.retry_in), ("Sign in", Some("https://tv.test/activate"), Some("WDJB-MJHT"), Some(5)));
        let IptvError::ActionRequired(local) = from_wit_error(wit::Error::ActionRequired(wit_prompt("file:///etc/passwd"))) else { panic!("not a prompt") };
        assert_eq!((local.url, local.code.as_deref()), (None, Some("WDJB-MJHT")), "the app opens the link: http(s) only");
    }

    #[test]
    fn tracks_are_checked_like_the_stream() {
        let tracks = playback(with_track("https://s.test/fr.m4a", None)).unwrap().tracks;
        assert_eq!((tracks[0].url.as_str(), tracks[0].language.as_deref(), tracks[0].title.as_deref()), ("https://s.test/fr.m4a", Some("fr"), Some("Français")));
        assert!(playback(with_track("file:///home/me/fr.m4a", None)).is_err());
        assert!(playback(with_track("rtmp://s.test/fr", None)).is_ok());
        assert!(playback(with_track("rtmp://s.test/fr", Some(wit::Relay { browser: None, filter: false }))).is_err(), "only http(s) tracks can be relayed");
    }
}
