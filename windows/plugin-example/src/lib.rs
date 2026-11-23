//! Channels, movies and series listed in a JSON file:
//! `{"channels": [{"name", "url", "logo"?, "group"?, "epgId"?, "programmes"?}], "movies"?: [{"name", "url", "audio"?}],
//! "series"?: [{"name", "episodes": [{"title", "url", "season"?, "number"?, "aired"?}]}], "epg"?}`, where a programme is
//! `{"start", "end", "title", "desc"?}` and `aired` in Unix seconds. A movie's `audio` is played alongside its video.
//! With the `browser` setting, the list and the streams are fetched with that browser's handshake, the streams through
//! the app. A list served with an `ETag` is kept, and asked again with `If-None-Match`.

#![allow(clippy::too_many_arguments, reason = "the lowered signature wit-bindgen generates for the filter export")]

use serde::{Deserialize, Serialize};

wit_bindgen::generate!({ path: "../wit", world: "provider" });

use nightglass::provider::http::{Request, send};
use nightglass::provider::log::{Level, log};
use nightglass::provider::storage;
use nightglass::provider::types::{Browser, Field, FieldKind, Programme, Relay, Track};

struct JsonList;

#[derive(Serialize, Deserialize)]
struct Kept {
    etag: String,
    body: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct List {
    channels: Vec<Entry>,
    #[serde(default)]
    movies: Vec<Film>,
    #[serde(default)]
    series: Vec<Show>,
    epg: Option<String>,
}

#[derive(Deserialize)]
struct Film {
    name: String,
    url: String,
    audio: Option<String>,
}

#[derive(Deserialize)]
struct Show {
    name: String,
    episodes: Vec<Part>,
}

#[derive(Deserialize)]
struct Part {
    title: String,
    url: String,
    season: Option<i64>,
    number: Option<i64>,
    aired: Option<i64>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Entry {
    name: String,
    url: String,
    logo: Option<String>,
    group: Option<String>,
    epg_id: Option<String>,
    #[serde(default)]
    programmes: Vec<Slot>,
}

#[derive(Deserialize)]
struct Slot {
    start: i64,
    end: i64,
    title: String,
    desc: Option<String>,
}

fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |h, b| (h ^ u64::from(*b)).wrapping_mul(0x0100_0000_01b3))
}

fn setting<'a>(config: &'a Config, key: &str) -> Option<&'a str> {
    config.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

fn browser(config: &Config) -> Result<Option<Browser>, Error> {
    match setting(config, "browser").map(|b| b.trim().to_ascii_lowercase()).as_deref() {
        None | Some("") => Ok(None),
        Some("chrome") => Ok(Some(Browser::Chrome)),
        Some("edge") => Ok(Some(Browser::Edge)),
        Some("firefox") => Ok(Some(Browser::Firefox)),
        Some("safari") => Ok(Some(Browser::Safari)),
        Some(_) => Err(Error::Setting("browser".into())),
    }
}

impl Guest for JsonList {
    fn describe() -> ProviderInfo {
        ProviderInfo {
            id: "json-list".into(),
            name: "JSON list".into(),
            fields: vec![
                Field { key: "url".into(), label: "List URL".into(), kind: FieldKind::Url, required: true, placeholder: Some("https://… .json".into()), default: None },
                Field { key: "token".into(), label: "Access token".into(), kind: FieldKind::Secret, required: false, placeholder: None, default: None },
                Field { key: "browser".into(), label: "Browser".into(), kind: FieldKind::Text, required: false, placeholder: Some("chrome, edge, firefox or safari".into()), default: None },
            ],
            resolves_streams: true,
            presets: Vec::new(),
        }
    }

    fn load_catalog(config: Config) -> Result<Catalog, Error> {
        let list = list(&config)?;
        log(Level::Info, &format!("{} channels", list.channels.len()));
        let mut programmes = Vec::new();
        let mut channels = Vec::new();
        for (i, e) in list.channels.into_iter().enumerate() {
            let epg_id = e.epg_id.or_else(|| (!e.programmes.is_empty()).then(|| e.url.clone()));
            for show in e.programmes {
                let channel = epg_id.clone().unwrap_or_default();
                programmes.push(Programme { channel, start: show.start, end: show.end, title: show.title, desc: show.desc });
            }
            channels.push(Channel {
                id: e.url.clone(),
                name: e.name,
                number: Some(i as i64 + 1),
                group: e.group,
                logo: e.logo,
                epg_id,
                stream: e.url,
                headers: Vec::new(),
                catchup_window: None,
            });
        }
        let movies = list
            .movies
            .into_iter()
            .map(|m| {
                let stream = serde_json::to_string(&(&m.url, &m.audio)).expect("strings serialize");
                Movie { id: m.url, name: m.name, group: None, poster: None, stream, rating: None, plot: None, duration: None, available_until: None }
            })
            .collect();
        let series = list.series.into_iter().map(|s| Series { id: s.name.clone(), name: s.name, group: None, poster: None, rating: None, plot: None }).collect();
        Ok(Catalog { channels, movies, series, epg_url: list.epg, account: None, programmes })
    }

    fn stream_url(config: Config, channel: Channel) -> Result<Playback, Error> {
        Ok(Playback { url: channel.stream, headers: Vec::new(), relay: relay(&config)?, tracks: Vec::new() })
    }

    fn movie_url(config: Config, movie: Movie) -> Result<Playback, Error> {
        let (url, audio): (String, Option<String>) = serde_json::from_str(&movie.stream).map_err(|e| Error::BadResponse(e.to_string()))?;
        let tracks = audio.into_iter().map(|url| Track { url, language: None, title: None }).collect();
        Ok(Playback { url, headers: Vec::new(), relay: relay(&config)?, tracks })
    }

    fn episode_url(config: Config, _series: Series, episode: Episode) -> Result<Playback, Error> {
        Ok(Playback { url: episode.stream, headers: Vec::new(), relay: relay(&config)?, tracks: Vec::new() })
    }

    fn catchup_url(_config: Config, _channel: Channel, _start: i64, _end: i64) -> Result<Playback, Error> {
        Err(Error::BadResponse("no replay on this source".into()))
    }

    fn filter(_config: Config, _url: String, media: Media) -> Result<Media, Error> {
        Ok(media)
    }

    fn episodes(config: Config, series: Series) -> Result<Vec<Episode>, Error> {
        let show = list(&config)?.series.into_iter().find(|s| s.name == series.id);
        let parts = show.map(|s| s.episodes).unwrap_or_default();
        Ok(parts
            .into_iter()
            .map(|p| Episode { id: p.url.clone(), title: p.title, season: p.season, number: p.number, stream: p.url, plot: None, aired: p.aired, duration: None, thumbnail: None, available_until: None })
            .collect())
    }
}

fn relay(config: &Config) -> Result<Option<Relay>, Error> {
    Ok(browser(config)?.map(|b| Relay { browser: Some(b), filter: false }))
}

fn list(config: &Config) -> Result<List, Error> {
    let url = setting(config, "url").ok_or_else(|| Error::Setting("url".into()))?;
    let mut headers = vec![("accept".to_string(), "application/json".to_string())];
    if let Some(token) = setting(config, "token") {
        headers.push(("authorization".into(), format!("Bearer {token}")));
    }
    let key = format!("list:{:016x}", fnv1a(url.as_bytes()));
    let kept: Option<Kept> = storage::get(&key).and_then(|v| serde_json::from_str(&v).ok());
    if let Some(kept) = &kept {
        headers.push(("if-none-match".into(), kept.etag.clone()));
    }
    let response = send(&Request { method: "GET".into(), url: url.into(), headers, body: None, browser: browser(config)? })?;
    let body = match (response.status, kept) {
        (304, Some(kept)) => kept.body,
        (200..=299, _) => {
            let body = String::from_utf8(response.body).map_err(|e| Error::BadResponse(e.to_string()))?;
            match response.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case("etag")) {
                Some((_, etag)) => {
                    let kept = serde_json::to_string(&Kept { etag: etag.clone(), body: body.clone() }).expect("strings serialize");
                    if let Err(e) = storage::set(&key, &kept, None, true) {
                        log(Level::Warn, &format!("list not kept: {e}"));
                    }
                }
                None => storage::delete(&key),
            }
            body
        }
        (401 | 403, _) => return Err(Error::AuthFailed),
        (status, _) => return Err(Error::Http(status)),
    };
    serde_json::from_str(&body).map_err(|e| Error::BadResponse(e.to_string()))
}

export!(JsonList);
