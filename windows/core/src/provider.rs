//! The contract every source type meets, built-in or plugin: what the add form asks for, and the
//! calls that turn a source's settings into the common models.

use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::{Catalog, Channel, Episode, IptvError, Media, Movie, Playback, Result, Series};

/// A source's settings, keyed by `Field::key`. Fields left empty are absent.
pub type Config = BTreeMap<String, String>;

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    pub id: String,
    pub name: String,
    pub fields: Vec<Field>,
    /// Stream links are short-lived: `stream_url` is asked at play time.
    #[serde(default)]
    pub resolves_streams: bool,
    #[serde(default)]
    pub presets: Vec<Preset>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Field {
    pub key: String,
    pub label: String,
    pub kind: FieldKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default)]
    pub placeholder: Option<String>,
    #[serde(default)]
    pub default: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum FieldKind {
    Text,
    Secret,
    /// An http(s) address.
    Url,
    /// An http(s) address or a local file.
    Location,
    Mac,
}

/// Ready-made settings the add form can fill in.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub label: String,
    pub name: String,
    pub config: Config,
}

pub trait Provider: Send + Sync {
    fn info(&self) -> &ProviderInfo;

    fn catalog<'a>(&'a self, config: &'a Config) -> BoxFuture<'a, Result<Catalog>>;

    fn stream_url<'a>(&'a self, _config: &'a Config, channel: &'a Channel) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move { Ok(channel.stream.clone().into()) })
    }

    /// A past programme of `channel`, from `start` to `end` (Unix seconds), on servers that keep them.
    fn catchup_url<'a>(&'a self, _config: &'a Config, _channel: &'a Channel, _start: i64, _end: i64) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async { Err(IptvError::BadResponse("no replay on this source".into())) })
    }

    fn episodes<'a>(&'a self, _config: &'a Config, _series: &'a Series) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    /// Asked at play time when the provider `resolves_streams`; otherwise the movie's `stream` is played as is.
    fn movie_url<'a>(&'a self, _config: &'a Config, movie: &'a Movie) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move { Playback::stored(&movie.stream) })
    }

    /// Asked at play time when the provider `resolves_streams`; otherwise the episode's `stream` is played as is.
    fn episode_url<'a>(&'a self, _config: &'a Config, _series: &'a Series, episode: &'a Episode) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(async move { Playback::stored(&episode.stream) })
    }

    /// An answer `url` gave while the app relays a playback that asked for it (`Relay::filter`), changed before the
    /// player gets it.
    fn filter<'a>(&'a self, _config: &'a Config, _url: &'a Url, media: Media) -> BoxFuture<'a, Result<Media>> {
        Box::pin(async { Ok(media) })
    }
}

/// The providers the app knows, looked up by id.
#[derive(Clone, Default)]
pub struct Providers(Vec<Arc<dyn Provider>>);

impl Providers {
    pub fn add(&mut self, provider: Arc<dyn Provider>) -> std::result::Result<(), String> {
        let id = &provider.info().id;
        if self.0.iter().any(|p| &p.info().id == id) {
            return Err(format!("a provider named \"{id}\" already exists"));
        }
        self.0.push(provider);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<&Arc<dyn Provider>> {
        self.0.iter().find(|p| p.info().id == id).ok_or_else(|| IptvError::UnknownProvider(id.into()))
    }

    pub fn infos(&self) -> Vec<ProviderInfo> {
        self.0.iter().map(|p| p.info().clone()).collect()
    }
}

/// Reads typed values out of a `Config`.
pub trait ConfigExt {
    fn text(&self, key: &str) -> Result<&str>;
    fn url(&self, key: &str) -> Result<Url>;
    fn optional_url(&self, key: &str) -> Result<Option<Url>>;
}

impl ConfigExt for Config {
    fn text(&self, key: &str) -> Result<&str> {
        self.get(key).map(String::as_str).filter(|s| !s.trim().is_empty()).ok_or_else(|| IptvError::Setting(key.into()))
    }

    fn url(&self, key: &str) -> Result<Url> {
        Url::parse(self.text(key)?).map_err(|_| IptvError::Setting(key.into()))
    }

    fn optional_url(&self, key: &str) -> Result<Option<Url>> {
        match self.get(key).map(|s| s.trim()) {
            None | Some("") => Ok(None),
            Some(_) => self.url(key).map(Some),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fake(ProviderInfo);

    impl Provider for Fake {
        fn info(&self) -> &ProviderInfo {
            &self.0
        }
        fn catalog<'a>(&'a self, _config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
            Box::pin(async { Ok(Catalog::default()) })
        }
    }

    fn fake(id: &str) -> Arc<dyn Provider> {
        Arc::new(Fake(ProviderInfo { id: id.into(), name: id.into(), fields: Vec::new(), resolves_streams: false, presets: Vec::new() }))
    }

    #[test]
    fn ids_are_unique() {
        let mut providers = Providers::default();
        providers.add(fake("a")).unwrap();
        assert!(providers.add(fake("a")).is_err());
        assert!(providers.get("a").is_ok());
        assert_eq!(providers.get("b").err(), Some(IptvError::UnknownProvider("b".into())));
    }

    #[tokio::test]
    async fn defaults_play_the_stored_link() {
        let channel = Channel {
            id: "1".into(),
            name: "One".into(),
            number: None,
            group: None,
            logo: None,
            epg_id: None,
            stream: Url::parse("http://x.test/1.ts").unwrap(),
            headers: Default::default(),
            catchup_days: None,
        };
        let p = fake("a");
        let config = Config::new();
        assert_eq!(p.stream_url(&config, &channel).await.unwrap(), channel.stream.clone().into());
        assert!(p.catchup_url(&config, &channel, 0, 1).await.is_err());
    }

    #[test]
    fn config_values() {
        let config: Config = [("u".to_string(), " http://x.test/ ".to_string()), ("e".to_string(), "".to_string())].into();
        assert_eq!(config.url("u").unwrap().as_str(), "http://x.test/");
        assert_eq!(config.optional_url("e").unwrap(), None);
        assert_eq!(config.text("e").err(), Some(IptvError::Setting("e".into())));
        assert_eq!(config.text("u").unwrap(), " http://x.test/ ");
    }
}
