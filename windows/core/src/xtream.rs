use std::collections::HashMap;

use serde_json::Value;
use url::Url;

use crate::http::{double, int, objects, str, url};
use crate::xmltv::{civil_from_days, days_from_civil};
use crate::{Account, Catalog, Channel, Episode, Http, Info, IptvError, Movie, Result, Series};

/// Xtream Codes `player_api.php`.
pub struct XtreamClient {
    pub server: Url,
    pub username: String,
    pub password: String,
    /// The small catalog (see `KNOWN`): on by default on Android, off elsewhere; the app's setting overrides it.
    pub reduced: bool,
    http: Http,
}

/// Android keeps a small catalog: the streaming platforms and sports people know in France, plus UFC. A category is kept when
/// its name holds one of these (case ignored); French live TV (`FR|`) is kept whole.
const KNOWN: [&str; 21] = [
    "NETFLIX", "AMAZON", "PRIME", "DISNEY", "APPLE", "CANAL", "HBO", "MAX", "PARAMOUNT", "CRUNCHYROLL", "DAZN", "UFC", "WWE", "MARVEL", "OCS",
    "SALTO", "TF1", "M6", "ARTE", "BEIN", "FRANCE",
];

fn is_known(name: &str) -> bool {
    let upper = name.to_uppercase();
    KNOWN.iter().any(|k| upper.contains(k))
}

/// Live categories are named "US| NEWS", "FR| CINÉMA", "AR| …": French, and the known ones of the US and Spain.
fn keep_live_category(name: &str) -> bool {
    !name.chars().any(is_arabic)
        && (name.starts_with("FR|") || ((name.starts_with("US|") || name.starts_with("ES|")) && is_known(name)))
}

fn is_arabic(c: char) -> bool {
    matches!(c, '\u{0600}'..='\u{06FF}' | '\u{0750}'..='\u{077F}' | '\u{FB50}'..='\u{FDFF}' | '\u{FE70}'..='\u{FEFF}')
}

/// Movie and series categories are named by language ("FR - COMÉDIE", "EN - UFC", "IN - …") or by platform ("NETFLIX MOVIES"):
/// keep the known platforms and sports, in English, French or Spanish; drop other languages, Asia and Arabic.
fn keep_vod_category(name: &str) -> bool {
    if name.chars().any(is_arabic) {
        return false;
    }
    let upper = name.to_uppercase();
    if !is_known(name) || ["ASIA", "MENA", "ARAB", "TURK", "INDIA", "HINDI", "KOREA"].iter().any(|w| upper.contains(w)) {
        return false;
    }
    language_ok(name)
}

/// A name starting with a language code ("DE - Film", "IT| Serie") is kept only for English, French and Spanish.
fn language_ok(name: &str) -> bool {
    let rest = name.trim_start_matches(|c: char| !c.is_alphabetic());
    let code: String = rest.chars().take_while(char::is_ascii_alphabetic).collect();
    let after = &rest[code.len()..];
    let coded = (2..=4).contains(&code.len()) && (after.starts_with(" - ") || after.starts_with('|') || after.starts_with(" |") || after.starts_with(" :"));
    !coded || ["EN", "US", "FR", "ES"].contains(&code.to_uppercase().as_str())
}

/// Titles are filtered like categories: no Arabic script, no other language's prefix.
fn keep_title(name: &str) -> bool {
    !name.chars().any(is_arabic) && language_ok(name)
}

/// Panels spell the same fields differently (`plot` or `description`, `releasedate` or `releaseDate`…).
fn parse_info(info: Option<&Value>) -> Info {
    let field = |names: &[&str]| names.iter().find_map(|n| str(info.and_then(|i| i.get(*n))).map(|v| v.trim().to_string()).filter(|v| !v.is_empty()));
    Info {
        plot: field(&["plot", "description"]),
        cast: field(&["cast", "actors"]),
        director: field(&["director"]),
        genre: field(&["genre"]),
        release: field(&["releasedate", "releaseDate", "release_date", "year"]),
        duration: field(&["duration", "episode_run_time"]),
        rating: double(info.and_then(|i| i.get("rating"))).filter(|r| *r > 0.0),
        trailer: field(&["youtube_trailer", "trailer"]),
        backdrop: url(info.and_then(|i| i.get("backdrop_path")).and_then(|b| b.as_array().and_then(|a| a.first()).or(Some(b)))),
    }
}

impl XtreamClient {
    pub fn new(server: Url, username: &str, password: &str, http: Http) -> Self {
        XtreamClient { server, username: username.to_string(), password: password.to_string(), reduced: cfg!(target_os = "android"), http }
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
        self.account().await.map(|_| ())
    }

    /// Checks the credentials and reads the subscription: status, expiry, connections.
    pub async fn account(&self) -> Result<Account> {
        let root = self.get(None, &[]).await?;
        let info = root.get("user_info");
        if int(info.and_then(|u| u.get("auth"))) != Some(1) {
            return Err(IptvError::AuthFailed);
        }
        let field = |name: &str| info.and_then(|u| u.get(name));
        Ok(Account {
            status: str(field("status")),
            // "0", "" or null: never expires.
            expires: int(field("exp_date")).filter(|&t| t > 0),
            max_connections: int(field("max_connections")),
            active_connections: int(field("active_cons")),
            trial: int(field("is_trial")) == Some(1),
            message: str(field("message")).map(|m| m.trim().to_string()).filter(|m| !m.is_empty()),
        })
    }

    fn movie(&self, s: &serde_json::Map<String, Value>, group: Option<String>) -> Option<Movie> {
        let id = str(s.get("stream_id"))?;
        let ext = str(s.get("container_extension")).unwrap_or_else(|| "mp4".into());
        Some(Movie {
            name: str(s.get("name"))?,
            group,
            poster: url(s.get("stream_icon")),
            stream: self.media("movie", &id, &ext),
            rating: double(s.get("rating")),
            plot: str(s.get("plot")),
            id: format!("xt-vod-{id}"),
        })
    }

    fn show(&self, s: &serde_json::Map<String, Value>, group: Option<String>) -> Option<Series> {
        Some(Series {
            id: str(s.get("series_id"))?,
            name: str(s.get("name"))?,
            group,
            poster: url(s.get("cover")),
            rating: double(s.get("rating")),
            plot: str(s.get("plot")),
        })
    }

    /// Movies of the given categories, a few categories at a time; one that fails is skipped. Titles in Arabic script are left out.
    async fn movies_in(&self, cats: &HashMap<String, String>) -> Vec<Movie> {
        use futures_util::stream::{self, StreamExt};
        let this = self;
        let owned: Vec<(String, String)> = cats.iter().map(|(id, name)| (id.clone(), name.clone())).collect();
        let mut batches = stream::iter(owned).map(move |(id, name)| async move { this.movies_of(id, name).await }).buffer_unordered(6);
        let mut out = Vec::new();
        while let Some(batch) = batches.next().await {
            out.extend(batch);
        }
        out
    }

    async fn movies_of(&self, id: String, name: String) -> Vec<Movie> {
        let Ok(value) = self.get(Some("get_vod_streams"), &[("category_id", id.as_str())]).await else { return Vec::new() };
        let list = objects(value);
        list.iter().filter(|s| str(s.get("name")).is_some_and(|n| !self.reduced || keep_title(&n))).filter_map(|s| self.movie(s, Some(name.clone()))).collect()
    }

    async fn shows_in(&self, cats: &HashMap<String, String>) -> Vec<Series> {
        use futures_util::stream::{self, StreamExt};
        let this = self;
        let owned: Vec<(String, String)> = cats.iter().map(|(id, name)| (id.clone(), name.clone())).collect();
        let mut batches = stream::iter(owned).map(move |(id, name)| async move { this.shows_of(id, name).await }).buffer_unordered(6);
        let mut out = Vec::new();
        while let Some(batch) = batches.next().await {
            out.extend(batch);
        }
        out
    }

    async fn shows_of(&self, id: String, name: String) -> Vec<Series> {
        let Ok(value) = self.get(Some("get_series"), &[("category_id", id.as_str())]).await else { return Vec::new() };
        let list = objects(value);
        list.iter().filter(|s| str(s.get("name")).is_some_and(|n| !self.reduced || keep_title(&n))).filter_map(|s| self.show(s, Some(name.clone()))).collect()
    }

    pub async fn catalog(&self) -> Result<Catalog> {
        let account = self.account().await?;
        // A big provider lists 200 000 films and 50 000 series (150 MB of JSON), which a 32-bit TV box cannot hold at once
        // in the page's whole-catalog design: on Android, live TV only until movies and series load by category.
        let (mut lc, mut vc, mut sc, mut live, vod, series) = if cfg!(target_os = "android") {
            let (lc, vc, sc, live) = tokio::try_join!(
                self.categories("get_live_categories"),
                self.categories("get_vod_categories"),
                self.categories("get_series_categories"),
                self.list("get_live_streams"),
            )?;
            (lc, vc, sc, live, Vec::new(), Vec::new())
        } else {
            tokio::try_join!(
                self.categories("get_live_categories"),
                self.categories("get_vod_categories"),
                self.categories("get_series_categories"),
                self.list("get_live_streams"),
                self.list("get_vod_streams"),
                self.list("get_series"),
            )?
        };
        // This build is for US, French and Spanish TV: panels name their categories "US| NEWS", "FR| CINÉMA", "AR| …".
        if self.reduced {
            lc.retain(|_, name| keep_live_category(name));
            vc.retain(|_, name| keep_vod_category(name));
            sc.retain(|_, name| keep_vod_category(name));
            live.retain(|s| {
                str(s.get("category_id")).is_some_and(|id| lc.contains_key(&id)) && !str(s.get("name")).is_some_and(|n| n.chars().any(is_arabic))
            });
        }
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
        let movies: Vec<Movie> = vod.iter().filter_map(|s| self.movie(s, group(&vc, s))).collect();
        let shows: Vec<Series> = series.iter().filter_map(|s| self.show(s, group(&sc, s))).collect();
        // Android: only the categories kept, each asked for on its own, converted as it arrives.
        let (movies, shows) = if cfg!(target_os = "android") {
            tokio::join!(self.movies_in(&vc), self.shows_in(&sc))
        } else {
            (movies, shows)
        };
        Ok(Catalog { channels, movies, series: shows, epg_url: Some(self.with_credentials("xmltv.php")), account: Some(account) })
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

    /// `get_vod_info`: plot, cast, director, genre, duration and trailer of a film.
    pub async fn movie_info(&self, movie: &Movie) -> Result<Info> {
        let id = movie.id.strip_prefix("xt-vod-").ok_or_else(|| IptvError::BadResponse("not an Xtream film".into()))?;
        let root = self.get(Some("get_vod_info"), &[("vod_id", id)]).await?;
        Ok(parse_info(root.get("info")))
    }

    /// The `info` block of `get_series_info`.
    pub async fn series_info(&self, series: &Series) -> Result<Info> {
        let root = self.get(Some("get_series_info"), &[("series_id", &series.id)]).await?;
        Ok(parse_info(root.get("info")))
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
            (None, format!(r#"{{"user_info":{{"auth":{auth},"status":"Active","message":"Welcome to World 8K","exp_date":"1798761600","max_connections":"2","active_cons":"1","is_trial":"0"}},"server_info":{{}}}}"#)),
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

    #[test]
    fn vod_categories_keep_known_platforms_and_ufc() {
        for kept in ["NETFLIX MOVIES ⁴ᴷ", "AMAZON KIDS", "DISNEY+ MOVIES", "APPLE+ MOVIES", "EN - UFC", "EN - WWE", "SOCCER UFC", "FR - CANAL+ SÉRIES"] {
            assert!(keep_vod_category(kept), "{kept}");
        }
        for dropped in ["EN - COMEDY", "TOP MOVIES", "UNIVERSAL", "IN - NEW RELEASE", "DE - AMAZON PRIME PRODUKTION", "NL - AMAZON PRIME", "مسلسلات عربية", "ASIA MOVIES (MULTI-SUBS)", "NETFLIX ASIA", "DISCOVERY+ SERIES MENA"] {
            assert!(!keep_vod_category(dropped), "{dropped}");
        }
    }

    #[test]
    fn info_reads_the_fields_panels_spell_differently() {
        let info = serde_json::json!({
            "plot": " A heist. ", "cast": "A, B", "director": "", "genre": "Action", "releasedate": "2024-05-01",
            "duration": "01:52:10", "rating": "7.5", "youtube_trailer": "abc123", "backdrop_path": ["http://img.test/b.jpg"]
        });
        let i = parse_info(Some(&info));
        assert_eq!(i.plot.as_deref(), Some("A heist."));
        assert_eq!((i.director, i.genre.as_deref(), i.release.as_deref(), i.trailer.as_deref()), (None, Some("Action"), Some("2024-05-01"), Some("abc123")));
        assert_eq!(i.rating, Some(7.5));
        assert_eq!(i.backdrop.unwrap().as_str(), "http://img.test/b.jpg");
        assert_eq!(parse_info(None), Info::default());
    }

    #[test]
    fn titles_in_other_languages_are_dropped() {
        for kept in ["Dune", "FR - Le Parrain", "EN - The Godfather", "ES| El Padrino", "Spider-Man: No Way Home"] {
            assert!(keep_title(kept), "{kept}");
        }
        for dropped in ["DE - Marvel Zombies", "IT - Il Padrino", "NL| De Peetvader", "TR - Film", "مسلسل"] {
            assert!(!keep_title(dropped), "{dropped}");
        }
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
        let account = cat.account.unwrap();
        assert_eq!(account.message.as_deref(), Some("Welcome to World 8K"));
        assert_eq!((account.status.as_deref(), account.expires, account.max_connections, account.active_connections), (Some("Active"), Some(1_798_761_600), Some(2), Some(1)));
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
