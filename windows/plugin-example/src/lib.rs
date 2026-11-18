//! Channels listed in a JSON file: `{"channels": [{"name", "url", "logo"?, "group"?, "epgId"?}], "epg"?}`.

use serde::Deserialize;

wit_bindgen::generate!({ path: "../wit", world: "provider" });

use nightglass::provider::http::{Request, send};
use nightglass::provider::log::{Level, log};
use nightglass::provider::types::{Field, FieldKind};

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
}

fn setting<'a>(config: &'a Config, key: &str) -> Option<&'a str> {
    config.iter().find(|(k, _)| k == key).map(|(_, v)| v.as_str())
}

impl Guest for JsonList {
    fn describe() -> ProviderInfo {
        ProviderInfo {
            id: "json-list".into(),
            name: "JSON list".into(),
            fields: vec![
                Field { key: "url".into(), label: "List URL".into(), kind: FieldKind::Url, required: true, placeholder: Some("https://… .json".into()), default: None },
                Field { key: "token".into(), label: "Access token".into(), kind: FieldKind::Secret, required: false, placeholder: None, default: None },
            ],
            resolves_streams: false,
            presets: Vec::new(),
        }
    }

    fn load_catalog(config: Config) -> Result<Catalog, Error> {
        let url = setting(&config, "url").ok_or_else(|| Error::Setting("url".into()))?;
        let mut headers = vec![("accept".to_string(), "application/json".to_string())];
        if let Some(token) = setting(&config, "token") {
            headers.push(("authorization".into(), format!("Bearer {token}")));
        }
        let response = send(&Request { method: "GET".into(), url: url.into(), headers, body: None })?;
        match response.status {
            200..=299 => {}
            401 | 403 => return Err(Error::AuthFailed),
            status => return Err(Error::Http(status)),
        }
        let list: List = serde_json::from_slice(&response.body).map_err(|e| Error::BadResponse(e.to_string()))?;
        log(Level::Info, &format!("{} channels", list.channels.len()));
        let channels = list
            .channels
            .into_iter()
            .enumerate()
            .map(|(i, e)| Channel {
                id: e.url.clone(),
                name: e.name,
                number: Some(i as i64 + 1),
                group: e.group,
                logo: e.logo,
                epg_id: e.epg_id,
                stream: e.url,
                headers: Vec::new(),
                catchup_days: None,
            })
            .collect();
        Ok(Catalog { channels, movies: Vec::new(), series: Vec::new(), epg_url: list.epg, account: None })
    }

    fn stream_url(_config: Config, channel: Channel) -> Result<String, Error> {
        Ok(channel.stream)
    }

    fn catchup_url(_config: Config, _channel: Channel, _start: i64, _end: i64) -> Result<String, Error> {
        Err(Error::BadResponse("no replay on this source".into()))
    }

    fn episodes(_config: Config, _series: Series) -> Result<Vec<Episode>, Error> {
        Ok(Vec::new())
    }
}

export!(JsonList);
