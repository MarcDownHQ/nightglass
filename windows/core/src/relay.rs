//! Streams the app fetches on the player's behalf, served on 127.0.0.1: for servers the player can't talk to
//! itself (a browser handshake), or whose answers need changing before it can read them (`Provider::filter`).
//! HLS playlists are rewritten so that every playlist, segment and key they name comes back through here.

use std::collections::HashMap;
use std::convert::Infallible;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::combinators::UnsyncBoxBody;
use http_body_util::{BodyExt, Full, StreamBody};
use hyper::body::Frame;
use hyper::header::{HeaderValue, CONTENT_LENGTH, CONTENT_TYPE, RANGE};
use hyper::{Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;
use tokio::sync::OnceCell;
use url::Url;
use uuid::Uuid;

use crate::{Config, Http, IptvError, Media, Playback, Provider, Relay, Request, Result, Track};

/// A session nobody asked anything of for this long is forgotten.
const IDLE: Duration = Duration::from_secs(6 * 3600);

/// Passed on from the upstream answer when the body is passed on as is.
const PASSED_ON: [&str; 5] = ["content-type", "content-length", "content-range", "accept-ranges", "cache-control"];

type Body = UnsyncBoxBody<Bytes, std::io::Error>;

/// Started with the first relayed playback; one per app. Cheap to clone.
#[derive(Clone)]
pub struct RelayServer(Arc<Inner>);

struct Inner {
    http: Http,
    port: OnceCell<u16>,
    sessions: Mutex<HashMap<String, Arc<Session>>>,
}

struct Session {
    provider: Arc<dyn Provider>,
    config: Config,
    headers: Vec<(String, String)>,
    relay: Relay,
    urls: Mutex<Urls>,
    used: Mutex<Instant>,
}

/// A live stream names new segments on every refresh: past this many addresses, those unused for `FORGOTTEN` go.
const KEPT: usize = 8192;
const FORGOTTEN: Duration = Duration::from_secs(600);

/// The upstream addresses a session has handed out, by number, and when each was last named or fetched.
#[derive(Default)]
struct Urls {
    next: usize,
    sweep_at: usize,
    by_number: HashMap<usize, (Url, Instant)>,
    numbers: HashMap<Url, usize>,
}

impl RelayServer {
    pub fn new(http: Http) -> Self {
        RelayServer(Arc::new(Inner { http, port: OnceCell::new(), sessions: Mutex::default() }))
    }

    /// `playback` with its stream and each of its tracks on 127.0.0.1.
    pub async fn relay(&self, provider: Arc<dyn Provider>, config: Config, playback: Playback) -> Result<Playback> {
        let mut tracks = Vec::with_capacity(playback.tracks.len());
        for track in &playback.tracks {
            let alone = Playback { url: track.url.clone(), tracks: Vec::new(), ..playback.clone() };
            tracks.push(Track { url: self.open(provider.clone(), config.clone(), alone).await?, ..track.clone() });
        }
        let url = self.open(provider, config, Playback { tracks: Vec::new(), ..playback }).await?;
        Ok(Playback { tracks, ..url.into() })
    }

    /// The address the player opens for `playback`, served by `provider` until it goes unused.
    pub async fn open(&self, provider: Arc<dyn Provider>, config: Config, playback: Playback) -> Result<Url> {
        let port = *self.0.port.get_or_try_init(|| self.listen()).await?;
        let token = Uuid::new_v4().simple().to_string();
        let session = Arc::new(Session {
            provider,
            config,
            headers: playback.headers.into_iter().collect(),
            relay: playback.relay.unwrap_or_default(),
            urls: Mutex::default(),
            used: Mutex::new(Instant::now()),
        });
        let local = session.local(port, &token, &playback.url);
        let mut sessions = lock(&self.0.sessions);
        sessions.retain(|_, s| lock(&s.used).elapsed() < IDLE);
        sessions.insert(token, session);
        Ok(local)
    }

    async fn listen(&self) -> Result<u16> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await.map_err(|e| IptvError::Network(format!("relay: {e}")))?;
        let port = listener.local_addr().map_err(|e| IptvError::Network(format!("relay: {e}")))?.port();
        let inner = Arc::downgrade(&self.0);
        tokio::spawn(async move {
            loop {
                let accepted = listener.accept().await;
                let Some(inner) = inner.upgrade() else { return };
                let stream = match accepted {
                    Ok((stream, _)) => stream,
                    Err(e) => {
                        log::warn!("relay: {e}");
                        drop(inner);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        continue;
                    }
                };
                let server = RelayServer(inner);
                tokio::spawn(async move {
                    let service = hyper::service::service_fn(move |request| {
                        let server = server.clone();
                        async move { Ok::<_, Infallible>(server.answer(request, port).await) }
                    });
                    let _ = hyper::server::conn::http1::Builder::new().serve_connection(TokioIo::new(stream), service).await;
                });
            }
        });
        Ok(port)
    }

    async fn answer(&self, request: hyper::Request<hyper::body::Incoming>, port: u16) -> Response<Body> {
        let mut parts = request.uri().path().trim_start_matches('/').splitn(2, '/');
        let (token, name) = (parts.next().unwrap_or_default(), parts.next().unwrap_or_default());
        let number = name.split('.').next().and_then(|n| n.parse::<usize>().ok());
        let session = lock(&self.0.sessions).get(token).cloned();
        let (Some(session), Some(number)) = (session, number) else { return status(StatusCode::NOT_FOUND) };
        let Some(upstream) = lock(&session.urls).get(number) else { return status(StatusCode::NOT_FOUND) };
        *lock(&session.used) = Instant::now();
        let range = request.headers().get(RANGE).and_then(|v| v.to_str().ok()).map(str::to_string);
        match session.relay_one(&self.0.http, upstream, range, port, token).await {
            Ok(response) => response,
            Err(e) => {
                log::warn!("relay: {e}");
                status(StatusCode::BAD_GATEWAY)
            }
        }
    }
}

impl Session {
    async fn relay_one(&self, http: &Http, upstream: Url, range: Option<String>, port: u16, token: &str) -> Result<Response<Body>> {
        let mut headers = self.headers.clone();
        if let Some(range) = range.filter(|_| !self.relay.filter) {
            headers.push(("Range".into(), range));
        }
        let request = Request { headers, browser: self.relay.browser, ..Request::get(upstream.clone()) };
        let mut answer = http.open(request.clone()).await?;
        if matches!(answer.status, 403 | 502 | 503) {
            answer = http.open(request).await?;
        }
        let content_type = answer.header("content-type").map(str::to_string);
        if !(200..300).contains(&answer.status) {
            return Ok(passed_on(answer, None));
        }
        let mut first = None;
        if !self.relay.filter && !is_playlist(&answer.url, content_type.as_deref()) {
            first = answer.chunk().await?;
            if !first.as_deref().is_some_and(starts_playlist) {
                return Ok(passed_on(answer, first));
            }
        }
        let mut body = first.map(Vec::from).unwrap_or_default();
        while let Some(chunk) = answer.chunk().await? {
            if body.len() + chunk.len() > Http::SEND_LIMIT {
                return Err(IptvError::Network(format!("{upstream}: larger than {} MB", Http::SEND_LIMIT >> 20)));
            }
            body.extend_from_slice(&chunk);
        }
        let mut media = Media { content_type, body };
        if self.relay.filter {
            media = self.provider.filter(&self.config, &answer.url, media).await?;
        }
        if is_playlist(&answer.url, media.content_type.as_deref()) || starts_playlist(&media.body) {
            let text = String::from_utf8_lossy(&media.body);
            media = Media { content_type: Some("application/vnd.apple.mpegurl".into()), body: self.rewrite(text.trim_start_matches('\u{feff}'), &answer.url, port, token).into_bytes() };
        }
        let mut response = Response::new(Full::new(Bytes::from(media.body)).map_err(|e| match e {}).boxed_unsync());
        if let Some(value) = media.content_type.and_then(|ct| HeaderValue::from_str(&ct).ok()) {
            response.headers_mut().insert(CONTENT_TYPE, value);
        }
        Ok(response)
    }

    /// Every address the playlist names, made to come back through the relay.
    fn rewrite(&self, playlist: &str, base: &Url, port: u16, token: &str) -> String {
        let mut out = String::with_capacity(playlist.len() * 2);
        for line in playlist.lines() {
            let line = line.trim_end_matches('\r');
            if line.starts_with('#') {
                out.push_str(&self.rewrite_attributes(line, base, port, token));
            } else if line.trim().is_empty() {
                out.push_str(line);
            } else {
                match base.join(line.trim()) {
                    Ok(url) => out.push_str(self.local(port, token, &url).as_str()),
                    Err(_) => out.push_str(line),
                }
            }
            out.push('\n');
        }
        out
    }

    /// `URI="…"` in a tag (`EXT-X-KEY`, `EXT-X-MAP`, `EXT-X-MEDIA`…).
    fn rewrite_attributes(&self, line: &str, base: &Url, port: u16, token: &str) -> String {
        const MARK: &str = "URI=\"";
        let mut out = String::new();
        let mut rest = line;
        while let Some(start) = rest.find(MARK) {
            let (before, after) = rest.split_at(start + MARK.len());
            out.push_str(before);
            let Some(end) = after.find('"') else {
                rest = after;
                break;
            };
            match base.join(&after[..end]) {
                Ok(url) if matches!(url.scheme(), "http" | "https") => out.push_str(self.local(port, token, &url).as_str()),
                _ => out.push_str(&after[..end]),
            }
            rest = &after[end..];
        }
        out.push_str(rest);
        out
    }

    /// The relay's address for `url`, numbered in this session. The extension stays: players guess formats from it.
    fn local(&self, port: u16, token: &str, url: &Url) -> Url {
        let number = lock(&self.urls).number(url);
        let extension = url
            .path_segments()
            .and_then(|mut s| s.next_back())
            .and_then(|name| name.rsplit_once('.'))
            .map(|(_, ext)| ext)
            .filter(|ext| (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric()));
        let name = extension.map_or(number.to_string(), |ext| format!("{number}.{ext}"));
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        Url::parse(&format!("http://{address}/{token}/{name}")).expect("relay address")
    }
}

impl Urls {
    fn number(&mut self, url: &Url) -> usize {
        if let Some(&n) = self.numbers.get(url) {
            self.get(n);
            return n;
        }
        if self.by_number.len() >= self.sweep_at.max(KEPT) {
            self.by_number.retain(|_, (_, used)| used.elapsed() < FORGOTTEN);
            let kept = &self.by_number;
            self.numbers.retain(|_, n| kept.contains_key(n));
            self.sweep_at = self.by_number.len() * 2;
        }
        let n = self.next;
        self.next += 1;
        self.by_number.insert(n, (url.clone(), Instant::now()));
        self.numbers.insert(url.clone(), n);
        n
    }

    fn get(&mut self, number: usize) -> Option<Url> {
        let (url, used) = self.by_number.get_mut(&number)?;
        *used = Instant::now();
        Some(url.clone())
    }
}

/// A playlist served under a name or a type that does not say so.
fn starts_playlist(body: &[u8]) -> bool {
    body.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(body).starts_with(b"#EXTM3U")
}

fn is_playlist(url: &Url, content_type: Option<&str>) -> bool {
    let path = url.path().to_ascii_lowercase();
    content_type.is_some_and(|ct| ct.to_ascii_lowercase().contains("mpegurl")) || path.ends_with(".m3u8") || path.ends_with(".m3u")
}

/// The upstream answer as it comes: status, the headers a player needs, and the body as it arrives.
fn passed_on(answer: crate::Opened, first: Option<Bytes>) -> Response<Body> {
    let mut response = Response::builder().status(answer.status);
    for (name, value) in &answer.headers {
        if PASSED_ON.contains(&name.to_ascii_lowercase().as_str()) {
            response = response.header(name, value);
        }
    }
    let first = futures_util::stream::iter(first.map(|chunk| Ok(Frame::data(chunk))));
    let rest = futures_util::stream::unfold(Some(answer.into_body()), |body| async move {
        let mut body = body?;
        match body.chunk().await {
            Ok(Some(chunk)) => Some((Ok(Frame::data(chunk)), Some(body))),
            Ok(None) => None,
            Err(e) => Some((Err(std::io::Error::other(e.to_string())), None)),
        }
    });
    let chunks = futures_util::StreamExt::chain(first, rest);
    response.body(BodyExt::boxed_unsync(StreamBody::new(chunks))).unwrap_or_else(|_| status(StatusCode::BAD_GATEWAY))
}

fn status(code: StatusCode) -> Response<Body> {
    let mut response = Response::new(Full::new(Bytes::new()).map_err(|e| match e {}).boxed_unsync());
    *response.status_mut() = code;
    response.headers_mut().insert(CONTENT_LENGTH, HeaderValue::from_static("0"));
    response
}

fn lock<T>(m: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{header, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;
    use crate::{BoxFuture, Catalog, ProviderInfo};

    /// Upper-cases segments, leaves the rest.
    struct Shouting(ProviderInfo);

    impl Provider for Shouting {
        fn info(&self) -> &ProviderInfo {
            &self.0
        }
        fn catalog<'a>(&'a self, _config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
            Box::pin(async { Ok(Catalog::default()) })
        }
        fn filter<'a>(&'a self, _config: &'a Config, url: &'a Url, media: Media) -> BoxFuture<'a, Result<Media>> {
            Box::pin(async move {
                if !url.path().ends_with(".ts") {
                    return Ok(media);
                }
                Ok(Media { content_type: Some("video/mp2t".into()), body: media.body.to_ascii_uppercase() })
            })
        }
    }

    fn provider() -> Arc<dyn Provider> {
        Arc::new(Shouting(ProviderInfo { id: "s".into(), name: "S".into(), fields: Vec::new(), resolves_streams: true, presets: Vec::new() }))
    }

    /// The relay stops with the last of its handles: keep the first one.
    async fn open(server: &MockServer, path: &str, relay: Relay) -> (RelayServer, Url) {
        let relay_server = RelayServer::new(Http::new());
        let playback = Playback { url: Url::parse(&format!("{}{path}", server.uri())).unwrap(), headers: [("X-Key".into(), "k".into())].into(), relay: Some(relay), tracks: Vec::new(), chapters: Vec::new(), drm: None };
        let local = relay_server.open(provider(), Config::new(), playback).await.unwrap();
        (relay_server, local)
    }

    async fn get(url: &str, range: Option<&str>) -> (u16, Option<String>, String) {
        let mut request = reqwest::Client::new().get(url);
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        let r = request.send().await.unwrap();
        let ct = r.headers().get("content-type").map(|v| v.to_str().unwrap().to_string());
        (r.status().as_u16(), ct, r.text().await.unwrap())
    }

    fn addresses(playlist: &str) -> Vec<String> {
        playlist.lines().filter(|l| !l.starts_with('#') && !l.is_empty()).map(str::to_string).collect()
    }

    #[tokio::test]
    async fn relays_an_hls_stream() {
        let server = MockServer::start().await;
        let master = "#EXTM3U\n#EXT-X-STREAM-INF:BANDWIDTH=1\ntracks/mono.m3u8\n";
        let media = "#EXTM3U\n#EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\",IV=0x1\n#EXTINF:5,\n/seg/a.ts?t=1\n#EXTINF:5,\nb.ts\n";
        Mock::given(path("/live/index.m3u8")).and(header("x-key", "k")).respond_with(ResponseTemplate::new(200).set_body_raw(master, "text/plain")).mount(&server).await;
        Mock::given(path("/live/tracks/mono.m3u8")).respond_with(ResponseTemplate::new(200).set_body_raw(media, "application/vnd.apple.mpegurl")).mount(&server).await;
        Mock::given(path("/seg/a.ts")).respond_with(ResponseTemplate::new(200).set_body_raw("a-bytes", "image/webp")).mount(&server).await;
        Mock::given(path("/live/tracks/key.bin")).respond_with(ResponseTemplate::new(200).set_body_string("key")).mount(&server).await;

        let (_relay, local) = open(&server, "/live/index.m3u8", Relay { browser: None, filter: true }).await;
        assert_eq!(local.host_str(), Some("127.0.0.1"));
        assert!(local.path().ends_with("/0.m3u8"));
        let (status, ct, body) = get(local.as_str(), None).await;
        assert_eq!((status, ct.as_deref()), (200, Some("application/vnd.apple.mpegurl")));
        let tracks = &addresses(&body)[0];
        assert!(tracks.starts_with(&format!("http://127.0.0.1:{}/", local.port().unwrap())) && tracks.ends_with("/1.m3u8"), "{body}");

        let (_, _, body) = get(tracks, None).await;
        let key = body.lines().find(|l| l.starts_with("#EXT-X-KEY")).unwrap();
        assert!(key.starts_with("#EXT-X-KEY:METHOD=AES-128,URI=\"http://127.0.0.1:") && key.ends_with("/2.bin\",IV=0x1"), "{key}");
        let segments = addresses(&body);
        assert!(segments[0].ends_with("/3.ts") && segments[1].ends_with("/4.ts"), "{body}");

        assert_eq!(get(&segments[0], None).await, (200, Some("video/mp2t".into()), "A-BYTES".into()));
        let key_url = &key[key.find("http").unwrap()..key.find("\",IV").unwrap()];
        assert_eq!(get(key_url, None).await.2, "key");
        assert_eq!(get(&segments[1], None).await.0, 404, "upstream 404 is passed on");
    }

    #[tokio::test]
    async fn a_refused_answer_is_asked_once_more() {
        let server = MockServer::start().await;
        Mock::given(path("/live/index.m3u8")).respond_with(ResponseTemplate::new(403)).up_to_n_times(1).with_priority(1).mount(&server).await;
        Mock::given(path("/live/index.m3u8")).respond_with(ResponseTemplate::new(200).set_body_raw("#EXTM3U\n#EXTINF:5,\na.ts\n", "application/vnd.apple.mpegurl")).mount(&server).await;
        Mock::given(path("/live/busy.m3u8")).respond_with(ResponseTemplate::new(503)).up_to_n_times(1).with_priority(1).mount(&server).await;
        Mock::given(path("/live/busy.m3u8")).respond_with(ResponseTemplate::new(200).set_body_raw("#EXTM3U\n#EXTINF:5,\na.ts\n", "application/vnd.apple.mpegurl")).mount(&server).await;
        Mock::given(path("/live/gone.m3u8")).respond_with(ResponseTemplate::new(404)).expect(1).mount(&server).await;
        Mock::given(path("/live/broken.m3u8")).respond_with(ResponseTemplate::new(500)).expect(1).mount(&server).await;

        let (_relay, local) = open(&server, "/live/index.m3u8", Relay { browser: None, filter: false }).await;
        assert_eq!(get(local.as_str(), None).await.0, 200);
        let (_relay, local) = open(&server, "/live/busy.m3u8", Relay { browser: None, filter: false }).await;
        assert_eq!(get(local.as_str(), None).await.0, 200);
        let (_relay, local) = open(&server, "/live/gone.m3u8", Relay { browser: None, filter: false }).await;
        assert_eq!(get(local.as_str(), None).await.0, 404, "only a refusal or a busy server is asked again");
        let (_relay, local) = open(&server, "/live/broken.m3u8", Relay { browser: None, filter: false }).await;
        assert_eq!(get(local.as_str(), None).await.0, 500);
    }

    #[tokio::test]
    async fn playlists_resolve_against_where_redirects_lead() {
        let server = MockServer::start().await;
        Mock::given(path("/a/index.m3u8")).respond_with(ResponseTemplate::new(302).insert_header("location", "/b/index.m3u8")).mount(&server).await;
        Mock::given(path("/b/index.m3u8")).respond_with(ResponseTemplate::new(200).set_body_string("#EXTM3U\nx.ts\n")).mount(&server).await;
        Mock::given(path("/b/x.ts")).respond_with(ResponseTemplate::new(200).set_body_string("bx")).mount(&server).await;
        let (_relay, local) = open(&server, "/a/index.m3u8", Relay::default()).await;
        let (_, _, body) = get(local.as_str(), None).await;
        assert_eq!(get(&addresses(&body)[0], None).await.2, "bx");
    }

    #[tokio::test]
    async fn other_answers_pass_on_as_they_come() {
        let server = MockServer::start().await;
        Mock::given(path("/film.mp4"))
            .and(header("range", "bytes=0-3"))
            .respond_with(ResponseTemplate::new(206).insert_header("content-range", "bytes 0-3/10").set_body_raw("film", "video/mp4"))
            .mount(&server)
            .await;
        let (_relay, local) = open(&server, "/film.mp4", Relay::default()).await;
        assert!(local.path().ends_with("/0.mp4"));
        let r = reqwest::Client::new().get(local).header("Range", "bytes=0-3").send().await.unwrap();
        assert_eq!(r.status(), 206);
        assert_eq!(r.headers()["content-range"], "bytes 0-3/10");
        assert_eq!(r.headers()["content-type"], "video/mp4");
        assert_eq!(r.text().await.unwrap(), "film");
    }

    #[tokio::test]
    async fn playlists_are_found_by_their_first_line() {
        let server = MockServer::start().await;
        let media = "\u{feff}#EXTM3U\n#EXTINF:5,\nseg-1.ts\n";
        Mock::given(path("/play.php")).respond_with(ResponseTemplate::new(200).set_body_raw(media, "application/octet-stream")).mount(&server).await;
        Mock::given(path("/seg-1.ts")).respond_with(ResponseTemplate::new(200).set_body_raw("ts", "video/mp2t")).mount(&server).await;
        let (_relay, local) = open(&server, "/play.php", Relay::default()).await;
        let (status, ct, body) = get(local.as_str(), None).await;
        assert_eq!((status, ct.as_deref()), (200, Some("application/vnd.apple.mpegurl")));
        let segment = &addresses(&body)[0];
        assert!(segment.ends_with("/1.ts"), "{body}");
        assert_eq!(get(segment, None).await.2, "ts");
    }

    #[tokio::test]
    async fn tracks_are_relayed_each_on_its_own_address() {
        let server = MockServer::start().await;
        for (name, body) in [("/video.mp4", "video"), ("/fr.m4a", "audio")] {
            Mock::given(path(name)).and(header("x-key", "k")).respond_with(ResponseTemplate::new(200).set_body_string(body)).mount(&server).await;
        }
        let relay_server = RelayServer::new(Http::new());
        let mut playback = Playback::from(Url::parse(&format!("{}/video.mp4", server.uri())).unwrap());
        playback.headers = [("X-Key".into(), "k".into())].into();
        playback.relay = Some(Relay::default());
        playback.tracks = vec![Track { url: Url::parse(&format!("{}/fr.m4a", server.uri())).unwrap(), language: Some("fr".into()), title: Some("Français".into()) }];
        let relayed = relay_server.relay(provider(), Config::new(), playback).await.unwrap();
        assert_eq!((relayed.relay, relayed.headers.len()), (None, 0));
        assert_eq!(get(relayed.url.as_str(), None).await.2, "video");
        let track = &relayed.tracks[0];
        assert_eq!((track.language.as_deref(), track.title.as_deref()), (Some("fr"), Some("Français")));
        assert_eq!(track.url.host_str(), Some("127.0.0.1"));
        assert_eq!(get(track.url.as_str(), None).await.2, "audio");
    }

    #[tokio::test]
    async fn unknown_addresses_are_not_found() {
        let server = MockServer::start().await;
        let (_relay, local) = open(&server, "/x.m3u8", Relay::default()).await;
        let port = local.port().unwrap();
        assert_eq!(get(&format!("http://127.0.0.1:{port}/nope/0.m3u8"), None).await.0, 404);
        assert_eq!(get(local.join("7.ts").unwrap().as_str(), None).await.0, 404);
    }

    #[test]
    fn extensions_survive_only_when_plain() {
        let session = Session {
            provider: provider(),
            config: Config::new(),
            headers: Vec::new(),
            relay: Relay::default(),
            urls: Mutex::default(),
            used: Mutex::new(Instant::now()),
        };
        let local = |s: &str| session.local(1, "t", &Url::parse(s).unwrap()).path().to_string();
        assert_eq!(local("http://x.test/a/seg.ts?x=1"), "/t/0.ts");
        assert_eq!(local("http://x.test/a/"), "/t/1");
        assert_eq!(local("http://x.test/a/b.weird-ext"), "/t/2");
        assert_eq!(local("http://x.test/a/seg.ts?x=1"), "/t/0.ts", "the same address keeps its number");
    }

    #[test]
    fn old_addresses_go_once_there_are_many() {
        let mut urls = Urls::default();
        let url = |i: usize| Url::parse(&format!("http://x.test/{i}.ts")).unwrap();
        for i in 0..KEPT {
            assert_eq!(urls.number(&url(i)), i);
        }
        let long_ago = Instant::now().checked_sub(FORGOTTEN * 2).unwrap();
        for (_, used) in urls.by_number.values_mut() {
            *used = long_ago;
        }
        assert_eq!(urls.get(7), Some(url(7)), "fetching an address keeps it");
        assert_eq!(urls.number(&url(KEPT)), KEPT);
        assert_eq!(urls.by_number.len(), 2);
        assert_eq!(urls.get(0), None);
        assert_eq!(urls.number(&url(7)), 7);
        assert_eq!(urls.number(&url(0)), KEPT + 1, "a forgotten address gets a new number");
    }
}
