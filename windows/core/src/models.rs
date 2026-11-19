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
    /// Days of past programmes the server can replay (Xtream `tv_archive`); none without replay.
    #[serde(default)]
    pub catchup_days: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Movie {
    pub id: String,
    pub name: String,
    pub group: Option<String>,
    pub poster: Option<Url>,
    pub stream: Url,
    pub rating: Option<f64>,
    pub plot: Option<String>,
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
pub struct Episode {
    pub id: String,
    pub title: String,
    pub season: i64,
    pub number: i64,
    pub stream: Url,
    pub plot: Option<String>,
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
}

impl From<Url> for Playback {
    fn from(url: Url) -> Self {
        Playback { url, headers: HashMap::new(), relay: None }
    }
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
}

pub type Result<T> = std::result::Result<T, IptvError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_sources_saved_by_older_versions() {
        let old = r#"{"id":"6f1c6c2e-8f4e-4f7e-9d3a-1b2c3d4e5f60","name":"P","kind":{"m3u":{"url":"http://x.test/a.m3u","epg":null}}}"#;
        let source: Source = serde_json::from_str(old).unwrap();
        assert_eq!(source.provider, "m3u");
        assert_eq!(source.config, Config::from([("url".to_string(), "http://x.test/a.m3u".to_string())]));
        let again: Source = serde_json::from_str(&serde_json::to_string(&source).unwrap()).unwrap();
        assert_eq!(again, source);
    }
}
