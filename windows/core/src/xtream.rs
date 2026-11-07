use std::collections::HashMap;

use serde_json::Value;
use url::Url;

use crate::http::{double, int, objects, str, url};
use crate::xmltv::{civil_from_days, days_from_civil};
use crate::{Catalog, Channel, Episode, Http, IptvError, Movie, Result, Series};

/// Xtream Codes `player_api.php`.
pub struct XtreamClient {
    pub server: Url,
    pub username: String,
    pub password: String,
    http: Http,
}

impl XtreamClient {
    pub fn new(server: Url, username: &str, password: &str, http: Http) -> Self {
        XtreamClient { server, username: username.to_string(), password: password.to_string(), http }
    }

    /// `server` + path segments, each percent-encoded (usernames and passwords may hold anything).
    fn path(&self, segments: &[&str]) -> Url {
        let mut u = self.server.clone();
        if let Ok(mut path) = u.path_segments_mut() {
            path.pop_if_empty().extend(segments);
        }
        u
    }

    fn with_credentials(&self, file: &str) -> Url {
        let mut u = self.path(&[file]);
        u.query_pairs_mut().append_pair("username", &self.username).append_pair("password", &self.password);
        u
    }

    async fn get(&self, action: Option<&str>, extra: &[(&str, &str)]) -> Result<Value> {
        let mut u = self.with_credentials("player_api.php");
        {
            let mut q = u.query_pairs_mut();
            if let Some(action) = action {
                q.append_pair("action", action);
            }
            for (k, v) in extra {
                q.append_pair(k, v);
            }
        }
        self.http.json(u, &[]).await
    }

    /// Empty sections come back as `[]`, `{}` or `null` depending on the panel.
    async fn list(&self, action: &str) -> Result<Vec<serde_json::Map<String, Value>>> {
        Ok(objects(self.get(Some(action), &[]).await?))
    }

    async fn categories(&self, action: &str) -> Result<HashMap<String, String>> {
        Ok(self
            .list(action)
            .await?
            .iter()
            .filter_map(|c| Some((str(c.get("category_id"))?, str(c.get("category_name"))?)))
            .collect())
    }

    fn media(&self, kind: &str, id: &str, ext: &str) -> Url {
        self.path(&[kind, &self.username, &self.password, &format!("{id}.{ext}")])
    }

    pub async fn authenticate(&self) -> Result<()> {
        let root = self.get(None, &[]).await?;
        match int(root.get("user_info").and_then(|u| u.get("auth"))) {
            Some(1) => Ok(()),
            _ => Err(IptvError::AuthFailed),
        }
    }

    pub async fn catalog(&self) -> Result<Catalog> {
        self.authenticate().await?;
        let (lc, vc, sc, live, vod, series) = tokio::try_join!(
            self.categories("get_live_categories"),
            self.categories("get_vod_categories"),
            self.categories("get_series_categories"),
            self.list("get_live_streams"),
            self.list("get_vod_streams"),
            self.list("get_series"),
        )?;
        let group = |cats: &HashMap<String, String>, s: &serde_json::Map<String, Value>| {
            str(s.get("category_id")).and_then(|id| cats.get(&id).cloned())
        };
        let channels = live
            .iter()
            .filter_map(|s| {
                let id = str(s.get("stream_id"))?;
                Some(Channel {
                    name: str(s.get("name"))?,
                    number: int(s.get("num")),
                    group: group(&lc, s),
                    logo: url(s.get("stream_icon")),
                    epg_id: str(s.get("epg_channel_id")),
                    stream: self.media("live", &id, "ts"),
                    headers: HashMap::new(),
                    // `tv_archive_duration` is in days; some panels leave it empty with replay on.
                    catchup_days: (int(s.get("tv_archive")) == Some(1)).then(|| int(s.get("tv_archive_duration")).unwrap_or(1).max(1)),
                    id: format!("xt-live-{id}"),
                })
            })
            .collect();
        let movies = vod
            .iter()
            .filter_map(|s| {
                let id = str(s.get("stream_id"))?;
                let ext = str(s.get("container_extension")).unwrap_or_else(|| "mp4".into());
                Some(Movie {
                    name: str(s.get("name"))?,
                    group: group(&vc, s),
                    poster: url(s.get("stream_icon")),
                    stream: self.media("movie", &id, &ext),
                    rating: double(s.get("rating")),
                    plot: str(s.get("plot")),
                    id: format!("xt-vod-{id}"),
                })
            })
            .collect();
        let shows = series
            .iter()
            .filter_map(|s| {
                Some(Series {
                    id: str(s.get("series_id"))?,
                    name: str(s.get("name"))?,
                    group: group(&sc, s),
                    poster: url(s.get("cover")),
                    rating: double(s.get("rating")),
                    plot: str(s.get("plot")),
                })
            })
            .collect();
        Ok(Catalog { channels, movies, series: shows, epg_url: Some(self.with_credentials("xmltv.php")) })
    }

    /// `timeshift/<user>/<pass>/<minutes>/<YYYY-MM-DD:HH-MM>/<id>.ts`, with the start in the server's
    /// own clock: its offset from UTC is read from `server_info.time_now`.
    pub async fn catchup_url(&self, channel: &Channel, start: i64, end: i64) -> Result<Url> {
        let id = channel.id.strip_prefix("xt-live-").ok_or_else(|| IptvError::BadResponse("not an Xtream channel".into()))?;
        let root = self.get(None, &[]).await?;
        let server_now = str(root.get("server_info").and_then(|i| i.get("time_now"))).and_then(|t| parse_clock(&t));
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        Ok(self.timeshift(id, start, end, server_now.map_or(0, |s| offset(s, now))))
    }

    fn timeshift(&self, id: &str, start: i64, end: i64, offset: i64) -> Url {
        let minutes = ((end - start) as f64 / 60.0).ceil().max(1.0) as i64;
        let at = start + offset;
        let (y, m, d) = civil_from_days(at.div_euclid(86_400));
        let secs = at.rem_euclid(86_400);
        let stamp = format!("{y:04}-{m:02}-{d:02}:{:02}-{:02}", secs / 3600, secs / 60 % 60);
        self.path(&["timeshift", &self.username, &self.password, &minutes.to_string(), &stamp, &format!("{id}.ts")])
    }

    pub async fn episodes(&self, series: &Series) -> Result<Vec<Episode>> {
        let info = self.get(Some("get_series_info"), &[("series_id", &series.id)]).await?;
        // `episodes` is a map keyed by season number on most panels, a list of lists on a few.
        let seasons: Vec<Value> = match info.get("episodes") {
            Some(Value::Object(by_season)) => {
                let mut s: Vec<_> = by_season.iter().collect();
                s.sort_by_key(|(k, _)| k.parse::<i64>().unwrap_or(0));
                s.into_iter().map(|(_, v)| v.clone()).collect()
            }
            Some(Value::Array(list)) => list.clone(),
            _ => Vec::new(),
        };
        Ok(seasons
            .into_iter()
            .flat_map(objects)
            .filter_map(|e| {
                let id = str(e.get("id"))?;
                let number = int(e.get("episode_num")).unwrap_or(0);
                let ext = str(e.get("container_extension")).unwrap_or_else(|| "mp4".into());
                Some(Episode {
                    title: str(e.get("title")).unwrap_or_else(|| format!("Episode {number}")),
                    season: int(e.get("season")).unwrap_or(1),
                    number,
                    stream: self.media("series", &id, &ext),
                    plot: str(e.get("info").and_then(|i| i.get("plot"))),
                    id,
                })
            })
            .collect())
    }
}

/// `2026-11-07 16:20:00` read as if it were UTC.
fn parse_clock(s: &str) -> Option<i64> {
    let n: Vec<i64> = s.split(|c: char| !c.is_ascii_digit()).filter(|p| !p.is_empty()).map(|p| p.parse().ok()).collect::<Option<_>>()?;
    let [y, mo, d, h, mi, se] = n.get(..6)?.try_into().ok()?;
    Some(days_from_civil(y, mo, d) * 86_400 + h * 3600 + mi * 60 + se)
}

/// The server's clock minus UTC, to the quarter hour (time zones are whole quarters).
fn offset(server_clock: i64, now: i64) -> i64 {
    ((server_clock - now) as f64 / 900.0).round() as i64 * 900
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{path, query_param, query_param_is_missing};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    async fn server(auth: i32) -> MockServer {
        let s = MockServer::start().await;
        let routes = [
            (None, format!(r#"{{"user_info":{{"auth":{auth},"status":"Active"}},"server_info":{{}}}}"#)),
            (Some("get_live_categories"), r#"[{"category_id":"1","category_name":"News"}]"#.into()),
            (
                Some("get_live_streams"),
                r#"[{"num":1,"name":"Wire 24","stream_id":42,"stream_icon":"http://logo.test/w.png","epg_channel_id":"wire.fr","category_id":"1"},{"name":"broken"}]"#.into(),
            ),
            (Some("get_vod_categories"), "[]".into()),
            (
                Some("get_vod_streams"),
                r#"[{"stream_id":"7","name":"Blade","category_id":null,"rating":"7.5","container_extension":"mkv","stream_icon":""}]"#.into(),
            ),
            (Some("get_series_categories"), "{}".into()),
            (Some("get_series"), "null".into()),
            (
                Some("get_series_info"),
                r#"{"episodes":{"2":[{"id":"902","episode_num":1,"title":"S2E1","season":2,"container_extension":"mp4"}],"1":[{"id":"901","episode_num":"1","title":"S1E1","season":1,"container_extension":"mkv","info":{"plot":"Pilot"}}]}}"#.into(),
            ),
        ];
        for (action, body) in routes {
            let mock = Mock::given(path("/player_api.php"));
            let mock = match action {
                Some(a) => mock.and(query_param("action", a)),
                None => mock.and(query_param_is_missing("action")),
            };
            mock.respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(&s).await;
        }
        s
    }

    fn client(s: &MockServer, password: &str) -> XtreamClient {
        XtreamClient::new(Url::parse(&s.uri()).unwrap(), "u", password, Http::new())
    }

    #[tokio::test]
    async fn catalog() {
        let s = server(1).await;
        let cat = client(&s, "p").catalog().await.unwrap();
        assert_eq!(cat.channels.len(), 1);
        let ch = &cat.channels[0];
        assert_eq!((ch.name.as_str(), ch.group.as_deref(), ch.epg_id.as_deref(), ch.number), ("Wire 24", Some("News"), Some("wire.fr"), Some(1)));
        assert_eq!(ch.stream.as_str(), format!("{}/live/u/p/42.ts", s.uri()));
        let movie = &cat.movies[0];
        assert_eq!(movie.stream.as_str(), format!("{}/movie/u/p/7.mkv", s.uri()));
        assert_eq!(movie.rating, Some(7.5));
        assert_eq!(movie.poster, None);
        assert!(cat.series.is_empty());
        assert_eq!(cat.epg_url.unwrap().as_str(), format!("{}/xmltv.php?username=u&password=p", s.uri()));
    }

    #[tokio::test]
    async fn rejects_bad_credentials() {
        let s = server(0).await;
        assert_eq!(client(&s, "bad").catalog().await, Err(IptvError::AuthFailed));
    }

    #[tokio::test]
    async fn episodes_sorted_by_season() {
        let s = server(1).await;
        let show = Series { id: "5".into(), name: "Show".into(), group: None, poster: None, rating: None, plot: None };
        let eps = client(&s, "p").episodes(&show).await.unwrap();
        assert_eq!(eps.iter().map(|e| e.title.as_str()).collect::<Vec<_>>(), ["S1E1", "S2E1"]);
        assert_eq!(eps[0].stream.as_str(), format!("{}/series/u/p/901.mkv", s.uri()));
        assert_eq!(eps[0].plot.as_deref(), Some("Pilot"));
    }

    #[test]
    fn timeshift_url_uses_the_server_clock() {
        let c = XtreamClient::new(Url::parse("http://panel.test/").unwrap(), "u", "p", Http::new());
        // 2026-11-07 19:00 UTC, 45 min, on a server one hour ahead.
        let start = days_from_civil(2026, 11, 7) * 86_400 + 19 * 3600;
        let url = c.timeshift("42", start, start + 45 * 60, 3600);
        assert_eq!(url.as_str(), "http://panel.test/timeshift/u/p/45/2026-11-07:20-00/42.ts");
        assert_eq!(offset(parse_clock("2026-11-07 20:00:07").unwrap(), start), 3600);
        assert_eq!(civil_from_days(days_from_civil(2024, 2, 29)), (2024, 2, 29));
    }

    #[test]
    fn credentials_are_escaped_in_paths() {
        let c = XtreamClient::new(Url::parse("http://panel.test:8080/").unwrap(), "a b", "p/#?", Http::new());
        assert_eq!(c.media("live", "1", "ts").as_str(), "http://panel.test:8080/live/a%20b/p%2F%23%3F/1.ts");
    }
}
