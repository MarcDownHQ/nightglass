//! The providers shipped with the app: M3U, Xtream Codes and Stalker.

use std::sync::Arc;

use url::Url;

use crate::provider::{BoxFuture, Config, ConfigExt, Field, FieldKind, Preset, Provider, ProviderInfo, Providers};
use crate::{Catalog, Channel, Episode, Http, M3uSource, Result, Series, StalkerClient, XtreamClient};

impl Providers {
    pub fn builtin(http: &Http) -> Self {
        let mut providers = Providers::default();
        for p in [M3u::provider(http), Xtream::provider(http), Stalker::provider(http)] {
            providers.add(p).expect("built-in ids are unique");
        }
        providers
    }
}

fn field(key: &str, label: &str, kind: FieldKind, required: bool, placeholder: Option<&str>) -> Field {
    Field { key: key.into(), label: label.into(), kind, required, placeholder: placeholder.map(Into::into), default: None }
}

struct M3u {
    info: ProviderInfo,
    http: Http,
}

impl M3u {
    fn provider(http: &Http) -> Arc<dyn Provider> {
        let info = ProviderInfo {
            id: "m3u".into(),
            name: "M3U".into(),
            fields: vec![
                field("url", "Playlist URL or file", FieldKind::Location, true, Some("https://… .m3u")),
                field("epg", "Guide URL (XMLTV, optional)", FieldKind::Location, false, Some("https://… .xml")),
            ],
            resolves_streams: false,
            presets: vec![Preset {
                label: "Use iptv-org France".into(),
                name: "iptv-org · France".into(),
                config: [
                    ("url".to_string(), "https://iptv-org.github.io/iptv/countries/fr.m3u".to_string()),
                    ("epg".to_string(), "https://xmltvfr.fr/xmltv/xmltv.xml".to_string()),
                ]
                .into(),
            }],
        };
        Arc::new(M3u { info, http: http.clone() })
    }
}

impl Provider for M3u {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn catalog<'a>(&'a self, config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
        Box::pin(async move { M3uSource::new(config.url("url")?, config.optional_url("epg")?, self.http.clone()).catalog().await })
    }
}

struct Xtream {
    info: ProviderInfo,
    http: Http,
}

impl Xtream {
    fn provider(http: &Http) -> Arc<dyn Provider> {
        let info = ProviderInfo {
            id: "xtream".into(),
            name: "Xtream".into(),
            fields: vec![
                field("server", "Server", FieldKind::Url, true, Some("http://provider:8080")),
                field("username", "Username", FieldKind::Text, true, None),
                field("password", "Password", FieldKind::Secret, true, None),
            ],
            resolves_streams: false,
            presets: Vec::new(),
        };
        Arc::new(Xtream { info, http: http.clone() })
    }

    fn client(&self, config: &Config) -> Result<XtreamClient> {
        Ok(XtreamClient::new(config.url("server")?, config.text("username")?, config.text("password")?, self.http.clone()))
    }
}

impl Provider for Xtream {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn catalog<'a>(&'a self, config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
        Box::pin(async move { self.client(config)?.catalog().await })
    }

    fn catchup_url<'a>(&'a self, config: &'a Config, channel: &'a Channel, start: i64, end: i64) -> BoxFuture<'a, Result<Url>> {
        Box::pin(async move { self.client(config)?.catchup_url(channel, start, end).await })
    }

    fn episodes<'a>(&'a self, config: &'a Config, series: &'a Series) -> BoxFuture<'a, Result<Vec<Episode>>> {
        Box::pin(async move { self.client(config)?.episodes(series).await })
    }
}

struct Stalker {
    info: ProviderInfo,
    http: Http,
}

impl Stalker {
    fn provider(http: &Http) -> Arc<dyn Provider> {
        let mut mac = field("mac", "MAC address", FieldKind::Mac, true, Some("00:1A:79:XX:XX:XX"));
        mac.default = Some("00:1A:79:".into());
        let info = ProviderInfo {
            id: "stalker".into(),
            name: "Stalker".into(),
            fields: vec![field("portal", "Portal URL", FieldKind::Url, true, Some("http://portal/c/")), mac],
            resolves_streams: true,
            presets: Vec::new(),
        };
        Arc::new(Stalker { info, http: http.clone() })
    }

    fn client(&self, config: &Config) -> Result<StalkerClient> {
        Ok(StalkerClient::new(config.url("portal")?, config.text("mac")?, self.http.clone()))
    }
}

impl Provider for Stalker {
    fn info(&self) -> &ProviderInfo {
        &self.info
    }

    fn catalog<'a>(&'a self, config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
        Box::pin(async move { self.client(config)?.catalog().await })
    }

    fn stream_url<'a>(&'a self, config: &'a Config, channel: &'a Channel) -> BoxFuture<'a, Result<Url>> {
        Box::pin(async move { self.client(config)?.stream_url(channel).await })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_three_protocols() {
        let ids: Vec<String> = Providers::builtin(&Http::new()).infos().into_iter().map(|i| i.id).collect();
        assert_eq!(ids, ["m3u", "xtream", "stalker"]);
    }

    #[tokio::test]
    async fn m3u_reads_its_config() {
        let path = std::env::temp_dir().join(format!("nightglass-builtin-{}.m3u", std::process::id()));
        std::fs::write(&path, "#EXTM3U\n#EXTINF:-1,One\nhttp://s.test/1\n").unwrap();
        let providers = Providers::builtin(&Http::new());
        let config: Config = [("url".to_string(), Url::from_file_path(&path).unwrap().to_string())].into();
        let catalog = providers.get("m3u").unwrap().catalog(&config).await.unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(catalog.channels.len(), 1);
    }

    #[tokio::test]
    async fn missing_setting_is_named() {
        let providers = Providers::builtin(&Http::new());
        let config: Config = [("server".to_string(), "http://x.test".to_string())].into();
        let err = providers.get("xtream").unwrap().catalog(&config).await.unwrap_err();
        assert_eq!(err, crate::IptvError::Setting("username".into()));
    }
}
