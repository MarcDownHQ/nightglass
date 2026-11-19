//! Channels listed in a JSON file: `{"channels": [{"name", "url", "logo"?, "group"?, "epgId"?, "programmes"?}], "epg"?}`,
//! where a programme is `{"start", "end", "title", "desc"?}` in Unix seconds. With the `browser` setting, the list and
//! the streams are fetched with that browser's handshake, the streams through the app.

#![allow(clippy::too_many_arguments, reason = "the lowered signature wit-bindgen generates for the filter export")]

use serde::Deserialize;

wit_bindgen::generate!({ path: "../wit", world: "provider" });

use nightglass::provider::http::{Request, send};
use nightglass::provider::log::{Level, log};
use nightglass::provider::types::{Browser, Field, FieldKind, Programme, Relay};

struct JsonList;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct List {
    channels: Vec<Entry>,
    epg: Option<String>,
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
    programmes: Vec<Show>,
}

#[derive(Deserialize)]
struct Show {
    start: i64,
    end: i64,
    title: String,
    desc: Option<String>,
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
        let url = setting(&config, "url").ok_or_else(|| Error::Setting("url".into()))?;
        let mut headers = vec![("accept".to_string(), "application/json".to_string())];
        if let Some(token) = setting(&config, "token") {
            headers.push(("authorization".into(), format!("Bearer {token}")));
        }
        let response = send(&Request { method: "GET".into(), url: url.into(), headers, body: None, browser: browser(&config)? })?;
        match response.status {
            200..=299 => {}
            401 | 403 => return Err(Error::AuthFailed),
            status => return Err(Error::Http(status)),
        }
        let list: List = serde_json::from_slice(&response.body).map_err(|e| Error::BadResponse(e.to_string()))?;
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
                catchup_days: None,
            });
        }
        Ok(Catalog { channels, movies: Vec::new(), series: Vec::new(), epg_url: list.epg, account: None, programmes })
    }

    fn stream_url(config: Config, channel: Channel) -> Result<Playback, Error> {
        let relay = browser(&config)?.map(|b| Relay { browser: Some(b), filter: false });
        Ok(Playback { url: channel.stream, headers: Vec::new(), relay })
    }

    fn catchup_url(_config: Config, _channel: Channel, _start: i64, _end: i64) -> Result<Playback, Error> {
        Err(Error::BadResponse("no replay on this source".into()))
    }

    fn filter(_config: Config, _url: String, media: Media) -> Result<Media, Error> {
        Ok(media)
    }

    fn episodes(_config: Config, _series: Series) -> Result<Vec<Episode>, Error> {
        Ok(Vec::new())
    }
}

export!(JsonList);
