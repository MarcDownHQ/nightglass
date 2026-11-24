use std::collections::{BTreeMap, HashMap};

use serde::{Deserialize, Serialize};
use url::Url;
use uuid::Uuid;

use crate::{Browser, Config};

/// A live channel, whatever protocol it came from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Channel {
    pub id: String,
    pub name: String,
    pub number: Option<i64>,
    pub group: Option<String>,
    pub logo: Option<Url>,
    pub epg_id: Option<String>,
    /// Direct stream for M3U/Xtream. For Stalker this is the portal `cmd`, resolved by `Client::stream_url`.
    pub stream: Url,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Seconds of past programmes the server can replay (Xtream `tv_archive`); none without replay.
    #[serde(default)]
    pub catchup_window: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Movie {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub poster: Option<Url>,
    pub stream: String,
    pub rating: Option<f64>,
    pub plot: Option<String>,
    /// Seconds.
    #[serde(default)]
    pub duration: Option<u32>,
    /// Unix seconds, for a replay that expires.
    #[serde(default)]
    pub available_until: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Series {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub poster: Option<Url>,
    pub rating: Option<f64>,
    pub plot: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Episode {
    pub id: String,
    pub title: String,
    /// Dated broadcasts (news, magazines) have neither season nor number.
    pub season: Option<i64>,
    pub number: Option<i64>,
    pub stream: String,
    pub plot: Option<String>,
    /// Unix seconds.
    #[serde(default)]
    pub aired: Option<i64>,
    /// Seconds.
    #[serde(default)]
    pub duration: Option<u32>,
    /// A still of this episode; the series' poster stands for the show.
    #[serde(default)]
    pub thumbnail: Option<Url>,
    /// Unix seconds, for a replay that expires.
    #[serde(default)]
    pub available_until: Option<i64>,
}

/// What a player opens for a channel or a replay.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Playback {
    pub url: Url,
    /// Sent with every request of the stream, over the channel's own.
    #[serde(default)]
    pub headers: HashMap<String, String>,
    /// Fetched by the app rather than by the player, which then opens the stream on 127.0.0.1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relay: Option<Relay>,
    /// Played alongside `url`, with the same headers and relay, in order of preference.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tracks: Vec<Track>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub chapters: Vec<Chapter>,
}

impl From<Url> for Playback {
    fn from(url: Url) -> Self {
        Playback { url, headers: HashMap::new(), relay: None, tracks: Vec::new(), chapters: Vec::new() }
    }
}

/// What the player may open from a catalog or a plugin: never `file:` or anything else outside the network.
pub const STREAM_SCHEMES: [&str; 9] = ["http", "https", "rtmp", "rtmps", "rtsp", "rtp", "udp", "srt", "mms"];

impl Playback {
    /// A movie's or an episode's stream as kept in the catalog.
    pub fn stored(stream: &str) -> Result<Self> {
        Url::parse(stream)
            .ok()
            .filter(|u| STREAM_SCHEMES.contains(&u.scheme()))
            .map(Self::from)
            .ok_or_else(|| IptvError::BadResponse(format!("not a stream address: {stream}")))
    }

    /// The one address mpv opens: `url` itself, or an EDL joining it and its tracks.
    pub fn player_url(&self) -> String {
        if self.tracks.is_empty() {
            return self.url.to_string();
        }
        let mut edl = format!("edl://!no_clip;{}", edl_quote(self.url.as_str()));
        for track in &self.tracks {
            edl.push_str(";!new_stream;!no_clip");
            let meta: Vec<String> = [("lang", &track.language), ("title", &track.title)]
                .into_iter()
                .filter_map(|(key, value)| value.as_deref().map(|v| format!("{key}={}", edl_quote(v))))
                .collect();
            if !meta.is_empty() {
                edl.push_str(&format!(";!track_meta,{}", meta.join(",")));
            }
            edl.push(';');
            edl.push_str(&edl_quote(track.url.as_str()));
        }
        edl
    }
}

/// mpv's length-prefixed quoting, which lets a value hold any character.
fn edl_quote(value: &str) -> String {
    format!("%{}%{value}", value.len())
}

/// A stream kept apart from the main one: an audio in another language, the audio of a video-only stream, subtitles.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub url: Url,
    /// An ISO 639 code, which mpv's language preferences match.
    #[serde(default)]
    pub language: Option<String>,
    /// Shown in the track menus.
    #[serde(default)]
    pub title: Option<String>,
}

/// A chapter of a movie or an episode, `start` in seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Chapter {
    pub start: f64,
    pub title: String,
}

/// The chapters the player can use: in order, without those that have no title or no readable start.
pub fn chapters_in_order(chapters: Vec<Chapter>) -> Vec<Chapter> {
    let mut kept: Vec<Chapter> = chapters.into_iter().filter(|c| c.start.is_finite() && !c.title.trim().is_empty()).collect();
    kept.sort_by(|a, b| a.start.total_cmp(&b.start));
    kept.into_iter().map(|c| Chapter { start: c.start.max(0.0), ..c }).collect()
}

/// How the app fetches a stream for the player: playlists are rewritten to come back through it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Relay {
    /// For servers that only answer browsers.
    #[serde(default)]
    pub browser: Option<Browser>,
    /// Every answer goes through `Provider::filter` before the player gets it.
    #[serde(default)]
    pub filter: bool,
}

/// A playlist, segment or key on its way to the player.
#[derive(Clone, Debug, PartialEq)]
pub struct Media {
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

/// One guide entry. Times are Unix seconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Programme {
    pub channel: String,
    pub start: i64,
    pub end: i64,
    pub title: String,
    pub desc: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Catalog {
    pub channels: Vec<Channel>,
    pub movies: Vec<Movie>,
    pub series: Vec<Series>,
    /// XMLTV guide advertised by the source (M3U header, Xtream `xmltv.php`).
    pub epg_url: Option<Url>,
    /// The subscription, for sources that say (Xtream `user_info`).
    #[serde(default)]
    pub account: Option<Account>,
    /// Guide entries the source sends with its catalog, matched to channels by `epg_id` like a guide's.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub programmes: Vec<Programme>,
}

/// An IPTV subscription as the server describes it.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    /// "Active", "Expired", "Banned", "Disabled"…
    pub status: Option<String>,
    /// Unix seconds; none for an account that never expires.
    pub expires: Option<i64>,
    pub max_connections: Option<i64>,
    pub active_connections: Option<i64>,
    pub trial: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(from = "StoredSource")]
pub struct Source {
    pub id: Uuid,
    pub name: String,
    /// A `ProviderInfo::id`.
    pub provider: String,
    pub config: Config,
}

impl Source {
    pub fn new(name: impl Into<String>, provider: impl Into<String>, config: Config) -> Self {
        Source { id: Uuid::new_v4(), name: name.into(), provider: provider.into(), config }
    }
}

/// Sources saved before providers were a list: `"kind": {"xtream": {"server": …}}`.
#[derive(Deserialize)]
#[serde(untagged)]
enum StoredSource {
    Current { id: Uuid, name: String, provider: String, config: Config },
    Legacy { id: Uuid, name: String, kind: BTreeMap<String, BTreeMap<String, Option<String>>> },
}

impl From<StoredSource> for Source {
    fn from(stored: StoredSource) -> Self {
        match stored {
            StoredSource::Current { id, name, provider, config } => Source { id, name, provider, config },
            StoredSource::Legacy { id, name, kind } => {
                let (provider, fields) = kind.into_iter().next().unwrap_or_default();
                let config = fields.into_iter().filter_map(|(k, v)| Some((k, v?))).collect();
                Source { id, name, provider, config }
            }
        }
    }
}

/// What a plugin needs the user to do outside the app before it can go on: open `url`, enter `code`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Prompt {
    pub message: String,
    pub url: Option<Url>,
    pub code: Option<String>,
    /// Seconds before asking again, while the prompt is shown.
    pub retry_in: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum IptvError {
    #[error("The server answered {0}.")]
    Http(u16),
    #[error("The server refused the credentials.")]
    AuthFailed,
    #[error("Unexpected answer: {0}.")]
    BadResponse(String),
    #[error("Connection failed: {0}")]
    Network(String),
    #[error("Can't read the file: {0}")]
    File(String),
    #[error("Missing or invalid setting: {0}.")]
    Setting(String),
    #[error("No provider \"{0}\" is installed.")]
    UnknownProvider(String),
    #[error("The plugin failed: {0}")]
    Plugin(String),
    #[error("{}", .0.message)]
    ActionRequired(Box<Prompt>),
}

pub type Result<T> = std::result::Result<T, IptvError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn episodes_reach_the_page_in_camel_case() {
        let episode = Episode { id: "e".into(), title: "JT".into(), season: None, number: None, stream: "jt".into(), plot: None, aired: None, duration: None, thumbnail: None, available_until: Some(1) };
        let json = serde_json::to_value(&episode).unwrap();
        assert_eq!(json["availableUntil"], 1);
        let old: Episode = serde_json::from_str(r#"{"id":"e","title":"S1E1","season":1,"number":1,"stream":"http://s.test/1.mkv","plot":null}"#).unwrap();
        assert_eq!((old.season, old.aired), (Some(1), None), "an episode kept before 0.3.0 still reads");
    }

    #[test]
    fn reads_sources_saved_by_older_versions() {
        let old = r#"{"id":"6f1c6c2e-8f4e-4f7e-9d3a-1b2c3d4e5f60","name":"P","kind":{"m3u":{"url":"http://x.test/a.m3u","epg":null}}}"#;
        let source: Source = serde_json::from_str(old).unwrap();
        assert_eq!(source.provider, "m3u");
        assert_eq!(source.config, Config::from([("url".to_string(), "http://x.test/a.m3u".to_string())]));
        let again: Source = serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
        assert_eq!(again, source);
    }

    #[test]
    fn prompts_reach_the_page_in_camel_case() {
        let prompt = Prompt { message: "Sign in".into(), url: Some(Url::parse("https://tv.test/activate").unwrap()), code: Some("WDJB-MJHT".into()), retry_in: Some(5) };
        let json = serde_json::to_value(&prompt).unwrap();
        assert_eq!(json, serde_json::json!({"message": "Sign in", "url": "https://tv.test/activate", "code": "WDJB-MJHT", "retryIn": 5}));
        assert_eq!(serde_json::from_value::<Prompt>(json).unwrap(), prompt);
        assert_eq!(IptvError::ActionRequired(Box::new(prompt)).to_string(), "Sign in");
    }

    #[test]
    fn chapters_are_kept_only_when_there_are_some() {
        let mut playback = Playback::from(Url::parse("http://s.test/film.mkv").unwrap());
        assert!(serde_json::to_value(&playback).unwrap().get("chapters").is_none());
        playback.chapters = vec![Chapter { start: 90.5, title: "Two".into() }];
        let json = serde_json::to_value(&playback).unwrap();
        assert_eq!(json["chapters"], serde_json::json!([{"start": 90.5, "title": "Two"}]));
        assert_eq!(serde_json::from_value::<Playback>(json).unwrap(), playback);
        let old: Playback = serde_json::from_str(r#"{"url":"http://s.test/film.mkv","headers":{}}"#).unwrap();
        assert!(old.chapters.is_empty(), "a playback kept before 0.4.0 still reads");
    }

    #[test]
    fn chapters_in_order_without_the_unusable() {
        let chapter = |start: f64, title: &str| Chapter { start, title: title.into() };
        let given = vec![chapter(600.0, "Three"), chapter(f64::NAN, "Lost"), chapter(0.0, "One"), chapter(120.0, " "), chapter(-4.0, "Before"), chapter(f64::INFINITY, "Never"), chapter(300.0, "Two")];
        let kept: Vec<_> = chapters_in_order(given).into_iter().map(|c| (c.start, c.title)).collect();
        assert_eq!(kept, [(0.0, "Before".to_string()), (0.0, "One".into()), (300.0, "Two".into()), (600.0, "Three".into())]);
    }

    fn track(url: &str, language: Option<&str>, title: Option<&str>) -> Track {
        Track { url: Url::parse(url).unwrap(), language: language.map(Into::into), title: title.map(Into::into) }
    }

    #[test]
    fn a_playback_without_tracks_plays_its_url() {
        let playback = Playback::from(Url::parse("http://s.test/a,b.m3u8").unwrap());
        assert_eq!(playback.player_url(), "http://s.test/a,b.m3u8");
    }

    #[test]
    fn tracks_are_joined_into_one_edl() {
        let mut playback = Playback::from(Url::parse("http://s.test/video.mp4?a=1;b=2").unwrap());
        playback.tracks = vec![track("http://s.test/fr.m4a", Some("fr"), Some("Français, doublé")), track("http://s.test/en.vtt", None, None)];
        assert_eq!(
            playback.player_url(),
            "edl://!no_clip;%31%http://s.test/video.mp4?a=1;b=2;!new_stream;!no_clip;!track_meta,lang=%2%fr,title=%18%Français, doublé;%20%http://s.test/fr.m4a;!new_stream;!no_clip;%20%http://s.test/en.vtt"
        );
    }
}
