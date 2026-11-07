use std::collections::HashMap;

use percent_encoding::{AsciiSet, CONTROLS, percent_decode_str, utf8_percent_encode};
use serde_json::Value;
use url::Url;

use crate::http::{int, objects, str, url};
use crate::{Catalog, Channel, Http, IptvError, Result};

/// What a URL path may hold as is; the rest of a portal `cmd` is percent-encoded.
const CMD: &AsciiSet = &CONTROLS.add(b' ').add(b'"').add(b'#').add(b'%').add(b'<').add(b'>').add(b'?').add(b'[').add(b'\\').add(b']').add(b'^').add(b'`').add(b'{').add(b'|').add(b'}');

/// Stalker / MAG middleware portal, authenticated by MAC address.
pub struct StalkerClient {
    pub portal: Url,
    pub mac: String,
    http: Http,
}

impl StalkerClient {
    pub fn new(portal: Url, mac: &str, http: Http) -> Self {
        StalkerClient { portal, mac: mac.to_uppercase(), http }
    }

    /// Users paste `http://host/c/`, `http://host/stalker_portal/c/` or the API itself; find the API.
    pub fn endpoint(&self) -> Url {
        if self.portal.path().ends_with(".php") {
            return self.portal.clone();
        }
        let mut u = self.portal.clone();
        u.set_path(if self.portal.path().contains("stalker_portal") { "/stalker_portal/server/load.php" } else { "/portal.php" });
        u.set_query(None);
        u
    }

    async fn js(&self, kind: &str, action: &str, extra: &[(&str, &str)], token: Option<&str>) -> Result<Value> {
        let mut u = self.endpoint();
        {
            let mut q = u.query_pairs_mut();
            q.append_pair("type", kind).append_pair("action", action);
            for (k, v) in extra {
                q.append_pair(k, v);
            }
            q.append_pair("JsHttpRequest", "1-xml");
        }
        let cookie = format!("mac={}; stb_lang=en; timezone=Europe/Paris", self.mac);
        let bearer = token.map(|t| format!("Bearer {t}"));
        let mut headers = vec![("User-Agent", "MAG200 stbapp ver: 2 rev: 250 Safari/533.3"), ("Cookie", cookie.as_str())];
        if let Some(b) = &bearer {
            headers.push(("Authorization", b));
        }
        let mut root = self.http.json(u, &headers).await?;
        Ok(root.get_mut("js").map(Value::take).unwrap_or(Value::Null))
    }

    pub async fn handshake(&self) -> Result<String> {
        let js = self.js("stb", "handshake", &[], None).await?;
        let token = str(js.get("token")).ok_or(IptvError::AuthFailed)?;
        self.js("stb", "get_profile", &[], Some(&token)).await?;
        Ok(token)
    }

    pub async fn catalog(&self) -> Result<Catalog> {
        let token = self.handshake().await?;
        let genres: HashMap<String, String> = objects(self.js("itv", "get_genres", &[], Some(&token)).await?)
            .iter()
            .filter_map(|g| Some((str(g.get("id"))?, str(g.get("title"))?)))
            .collect();
        let mut all = self.js("itv", "get_all_channels", &[], Some(&token)).await?;
        let data = all.get_mut("data").map(Value::take).unwrap_or(Value::Null);
        let channels = objects(data)
            .iter()
            .filter_map(|c| {
                let id = str(c.get("id"))?;
                let cmd = str(c.get("cmd"))?;
                Some(Channel {
                    name: str(c.get("name"))?,
                    number: int(c.get("number")),
                    group: str(c.get("tv_genre_id")).and_then(|g| genres.get(&g).cloned()),
                    logo: url(c.get("logo")),
                    epg_id: str(c.get("xmltv_id")),
                    stream: Url::parse(&format!("stalker:{}", utf8_percent_encode(&cmd, CMD))).ok()?,
                    headers: HashMap::new(),
                    catchup_days: None,
                    id: format!("stk-{id}"),
                })
            })
            .collect();
        // shortcut: Stalker VOD and EPG are not fetched; add `vod/get_ordered_list` and `itv/get_short_epg` when a portal needs them.
        Ok(Catalog { channels, ..Default::default() })
    }

    /// Portal links are short-lived: ask for a fresh one at play time.
    pub async fn stream_url(&self, channel: &Channel) -> Result<Url> {
        let encoded = channel.stream.as_str().strip_prefix("stalker:").unwrap_or_default();
        let cmd = percent_decode_str(encoded).decode_utf8_lossy();
        let token = self.handshake().await?;
        let js = self.js("itv", "create_link", &[("cmd", &cmd)], Some(&token)).await?;
        str(js.get("cmd"))
            .and_then(|link| Url::parse(strip_player(&link)).ok())
            .ok_or_else(|| IptvError::BadResponse("create_link".into()))
    }
}

/// `cmd` values look like `ffmpeg http://…` or `auto http://…`.
fn strip_player(cmd: &str) -> &str {
    match cmd.split_once(' ') {
        Some((player, link)) if !player.contains(':') => link,
        _ => cmd,
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    fn client(portal: &str) -> StalkerClient {
        StalkerClient::new(Url::parse(portal).unwrap(), "x", Http::new())
    }

    #[test]
    fn endpoint_discovery() {
        assert_eq!(client("http://p.test:8080/c/").endpoint().as_str(), "http://p.test:8080/portal.php");
        assert_eq!(client("http://p.test/stalker_portal/c/").endpoint().as_str(), "http://p.test/stalker_portal/server/load.php");
        assert_eq!(client("http://p.test/server/load.php").endpoint().as_str(), "http://p.test/server/load.php");
    }

    #[tokio::test]
    async fn catalog_and_link() {
        let s = MockServer::start().await;
        for (action, body) in [
            ("handshake", r#"{"js":{"token":"TOK"}}"#),
            ("get_profile", r#"{"js":{"id":1}}"#),
            ("get_genres", r#"{"js":[{"id":"*","title":"All"},{"id":"3","title":"Sport"}]}"#),
            (
                "get_all_channels",
                r#"{"js":{"data":[{"id":"10","name":"Orbit Sport","number":"3","cmd":"ffmpeg http://localhost/ch/10_","tv_genre_id":"3","logo":""}]}}"#,
            ),
            ("create_link", r#"{"js":{"cmd":"ffmpeg http://cdn.test/live/10.ts?token=abc"}}"#),
        ] {
            Mock::given(path("/portal.php"))
                .and(query_param("action", action))
                .respond_with(ResponseTemplate::new(200).set_body_string(body))
                .mount(&s)
                .await;
        }
        let c = StalkerClient::new(Url::parse(&format!("{}/c/", s.uri())).unwrap(), "00:1a:79:00:00:01", Http::new());
        let cat = c.catalog().await.unwrap();
        let ch = &cat.channels[0];
        assert_eq!((ch.name.as_str(), ch.group.as_deref(), ch.number), ("Orbit Sport", Some("Sport"), Some(3)));
        assert_eq!(c.stream_url(ch).await.unwrap().as_str(), "http://cdn.test/live/10.ts?token=abc");

        let requests = s.received_requests().await.unwrap();
        let last = requests.last().unwrap();
        let cmd = last.url.query_pairs().find(|(k, _)| k == "cmd").map(|(_, v)| v.into_owned());
        assert_eq!(cmd.as_deref(), Some("ffmpeg http://localhost/ch/10_"));
        assert_eq!(last.headers.get("Authorization").unwrap(), "Bearer TOK");
        assert!(last.headers.get("Cookie").unwrap().to_str().unwrap().contains("mac=00:1A:79:00:00:01"));
    }

    #[test]
    fn strip_player_prefix() {
        assert_eq!(strip_player("auto http://a/b"), "http://a/b");
        assert_eq!(strip_player("http://a/b"), "http://a/b");
    }
}
