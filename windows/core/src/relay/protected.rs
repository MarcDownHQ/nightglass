//! Protected playbacks: the licence obtained before the player starts, then manifests served without protection
//! and segments served decrypted, so that the player sees an ordinary clear stream.

use std::collections::{HashMap, HashSet};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::header::{CONTENT_RANGE, CONTENT_TYPE, HeaderValue};
use hyper::{Response, StatusCode};
use nightglass_drm::cenc::{self, Cdm};
use nightglass_drm::client::{Backend, Helper};
use nightglass_drm::mp4::{self, Index, Init, Segment};
use nightglass_drm::protocol::{Key, MessageKind};
use nightglass_drm::pssh::{Pssh, WIDEVINE};
use nightglass_drm::{DrmError, Exchange, hex};
use url::Url;
use uuid::Uuid;

use super::dash::{Addressing, Manifest, Rep, Source};
use super::hls::{self, Named};
use super::{Body, RelayServer, Session, Urls, lock, status};
use crate::{BoxFuture, Config, Decryptor, Http, IptvError, LicenseKind, LicenseRequest, Media, Playback, Provider, Result, Track, widevine};

/// How long a licence server may take to answer, through the plugin.
const LICENSE_TIMEOUT: Duration = Duration::from_secs(30);

/// The CDM helper next to the app's executable.
pub fn widevine_helper() -> PathBuf {
    helper_in(std::env::current_exe().ok().as_deref().and_then(Path::parent))
}

/// `nightglass-widevine` as the app ships it (a Tauri sidecar), or `nightglass-cdm` as Cargo builds it.
fn helper_in(dir: Option<&Path>) -> PathBuf {
    let named = |name: &str| {
        let file = format!("{name}{}", std::env::consts::EXE_SUFFIX);
        dir.map_or_else(|| PathBuf::from(&file), |d| d.join(&file))
    };
    let shipped = named("nightglass-widevine");
    if shipped.exists() { shipped } else { named("nightglass-cdm") }
}

impl Decryptor for RelayServer {
    fn open<'a>(&'a self, cdm: &'a widevine::Cdm, provider: Arc<dyn Provider>, config: Config, playback: Playback) -> BoxFuture<'a, Result<Playback>> {
        Box::pin(self.open_widevine(provider, config, playback, &cdm.library))
    }
}

fn drm_error(e: DrmError) -> IptvError {
    IptvError::Drm(e.to_string())
}

impl RelayServer {
    /// `playback` played through Google's Widevine CDM at `library`, hosted by `widevine_helper()`: the licence is
    /// obtained through `provider.license` before this returns, and the playback that comes back plays clear from
    /// 127.0.0.1 (`Playback::player_url`).
    pub async fn open_widevine(&self, provider: Arc<dyn Provider>, config: Config, playback: Playback, library: &Path) -> Result<Playback> {
        let helper = Helper::start(&Backend::native(widevine_helper(), library)).await.map_err(drm_error)?;
        helper.initialize().await.map_err(drm_error)?;
        if let Some(certificate) = playback.drm.as_ref().and_then(|d| d.certificate.clone()) {
            helper.set_server_certificate(certificate).await.map_err(drm_error)?;
        }
        self.open_protected(provider, config, playback, Arc::new(helper)).await
    }

    /// `playback`, whose `drm` is set, decrypted by `cdm`.
    pub async fn open_protected(&self, provider: Arc<dyn Provider>, config: Config, playback: Playback, cdm: Arc<dyn Cdm>) -> Result<Playback> {
        let port = *self.0.port.get_or_try_init(|| self.listen()).await?;
        let drm = playback.drm.clone().ok_or_else(|| IptvError::Drm("the playback is not protected".into()))?;
        let protection = Arc::new(Protection::new(cdm, exchange(provider.clone(), config.clone(), playback.url.clone(), drm.context.clone())));
        let given = drm.pssh.as_deref().map(|p| Pssh::parse(p).unwrap_or_else(|_| Pssh::new(WIDEVINE, p)));
        let url = self.open_one(&provider, &config, &playback, playback.url.clone(), &protection, port, given.as_ref()).await?;
        let mut tracks = Vec::with_capacity(playback.tracks.len());
        for track in &playback.tracks {
            tracks.push(Track { url: self.open_one(&provider, &config, &playback, track.url.clone(), &protection, port, given.as_ref()).await?, ..track.clone() });
        }
        protection.check()?;
        Ok(Playback { url, tracks, headers: HashMap::new(), relay: None, chapters: playback.chapters, drm: None })
    }

    #[allow(clippy::too_many_arguments)]
    async fn open_one(&self, provider: &Arc<dyn Provider>, config: &Config, playback: &Playback, url: Url, protection: &Arc<Protection>, port: u16, given: Option<&Pssh>) -> Result<Url> {
        let session = Arc::new(Session {
            provider: provider.clone(),
            config: config.clone(),
            headers: playback.headers.clone().into_iter().collect(),
            relay: playback.relay.clone().unwrap_or_default(),
            urls: Mutex::default(),
            used: Mutex::new(Instant::now()),
            protected: Some(Protected { protection: protection.clone(), resources: Mutex::default() }),
        });
        let http = &self.0.http;
        let fetched = session.fetch(http, &url, None).await?;
        let text = String::from_utf8_lossy(&fetched.body);
        let (resource, extension) = match Manifest::parse(&text, &fetched.url) {
            Some(manifest) => {
                protection.remember(&manifest.reps);
                let mut pssh: Vec<Pssh> = given.cloned().into_iter().collect();
                if pssh.is_empty() {
                    pssh = manifest.pssh.clone();
                }
                if pssh.is_empty() {
                    pssh = protection.pssh_of_inits(&session, http, manifest.reps.iter().filter_map(|r| r.init.clone())).await?;
                }
                protection.license_all(&pssh).await?;
                protection.probe_video(&manifest.reps).await?;
                (Resource::Manifest(url), "mpd")
            }
            None if hls::is_playlist(&text) => {
                let mut pssh: Vec<Pssh> = given.cloned().into_iter().collect();
                pssh.extend(hls::widevine_pssh(&text));
                let mut media = (text.to_string(), fetched.url.clone());
                if let Some(variant) = hls::first_variant(&text, &fetched.url) {
                    let fetched = session.fetch(http, &variant, None).await?;
                    media = (String::from_utf8_lossy(&fetched.body).into_owned(), fetched.url);
                    pssh.extend(hls::widevine_pssh(&media.0));
                }
                hls::rewrite(&media.0, &media.1, &mut |_| media.1.clone()).map_err(drm_error)?;
                if pssh.is_empty() {
                    pssh = protection.pssh_of_inits(&session, http, hls::maps(&media.0, &media.1).into_iter()).await?;
                }
                protection.license_all(&pssh).await?;
                (Resource::Manifest(url), "m3u8")
            }
            None => (Resource::Plain(url), ""),
        };
        let token = Uuid::new_v4().simple().to_string();
        let local = session.local_resource(port, &token, &resource, extension);
        let mut sessions = lock(&self.0.sessions);
        sessions.retain(|_, s| lock(&s.used).elapsed() < super::IDLE);
        sessions.insert(token, session);
        Ok(local)
    }
}

/// The licence exchange through `provider`, and where the plugin's own error lands when it fails: that says more
/// than "the licence failed".
fn exchange(provider: Arc<dyn Provider>, config: Config, url: Url, context: String) -> (Exchange, Arc<Mutex<Option<IptvError>>>) {
    let refused = Arc::new(Mutex::new(None));
    let slot = refused.clone();
    let exchange: Exchange = Arc::new(move |kind, challenge| {
        let (provider, config, slot) = (provider.clone(), config.clone(), slot.clone());
        let request = LicenseRequest {
            kind: match kind {
                MessageKind::LicenseRenewal => LicenseKind::Renewal,
                MessageKind::LicenseRelease => LicenseKind::Release,
                MessageKind::CertificateRequest => LicenseKind::Certificate,
                MessageKind::LicenseRequest | MessageKind::IndividualizationRequest => LicenseKind::Request,
            },
            challenge,
            url: url.clone(),
            context: context.clone(),
        };
        Box::pin(async move {
            let answer = match tokio::time::timeout(LICENSE_TIMEOUT, provider.license(&config, &request)).await {
                Ok(answer) => answer,
                Err(_) => Err(IptvError::Drm(format!("no licence after {} s", LICENSE_TIMEOUT.as_secs()))),
            };
            answer.map_err(|e| {
                let message = e.to_string();
                *lock(&slot) = Some(e);
                DrmError::License(message)
            })
        })
    });
    (exchange, refused)
}

/// What a protected session's addresses stand for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(super) enum Resource {
    /// Rewritten on every request: a live manifest changes.
    Manifest(Url),
    /// A representation's segments, which the player names relative to it.
    Base { url: Url, track: String },
    /// One segment, or the init segment, of a track.
    Segment { url: Url, range: Option<(u64, u64)>, track: String },
    /// Passed on as it is: subtitles, anything not protected.
    Plain(Url),
}

pub(super) struct Protected {
    protection: Arc<Protection>,
    resources: Mutex<Urls<Resource>>,
}

/// The keys of one playback, shared by its tracks.
pub(super) struct Protection {
    cdm: Arc<dyn Cdm>,
    exchange: Exchange,
    /// The plugin's own error, when the licence exchange failed: it says more than "the licence failed".
    refused: Arc<Mutex<Option<IptvError>>>,
    licensed: tokio::sync::Mutex<HashSet<Vec<u8>>>,
    probed: Mutex<HashSet<Vec<u8>>>,
    /// Keys the CDM only decrypts in its own decoder, with the error that says so: for every sample, or for video.
    decode_only: Mutex<HashMap<Vec<u8>, DrmError>>,
    video_refused: Mutex<HashMap<Vec<u8>, DrmError>>,
    sources: Mutex<HashMap<String, Source>>,
    inits: Mutex<HashMap<String, Arc<Init>>>,
    indexes: Mutex<HashMap<String, Index>>,
}

impl Protection {
    fn new(cdm: Arc<dyn Cdm>, (exchange, refused): (Exchange, Arc<Mutex<Option<IptvError>>>)) -> Self {
        Protection {
            cdm,
            exchange,
            refused,
            licensed: Default::default(),
            probed: Default::default(),
            decode_only: Default::default(),
            video_refused: Default::default(),
            sources: Default::default(),
            inits: Default::default(),
            indexes: Default::default(),
        }
    }

    fn error(&self, e: DrmError) -> IptvError {
        match e {
            DrmError::License(_) => lock(&self.refused).take().unwrap_or_else(|| drm_error(e)),
            e => drm_error(e),
        }
    }

    fn remember(&self, reps: &[Rep]) {
        let mut sources = lock(&self.sources);
        for rep in reps {
            if let Some(init) = &rep.init {
                sources.insert(rep.track.clone(), init.clone());
            }
        }
    }

    /// The first `pssh` the licence needs: the others only when they name keys not had yet.
    async fn license_all(&self, pssh: &[Pssh]) -> Result<()> {
        let mut widevine = pssh.iter().filter(|p| p.is_widevine());
        let Some(first) = widevine.next() else {
            return Err(drm_error(DrmError::Unsupported("no Widevine pssh in the stream".into())));
        };
        self.license(first).await?;
        for p in widevine {
            let known = self.cdm.keys();
            let kids = p.kids();
            if !kids.is_empty() && kids.iter().all(|k| known.iter().any(|known| known.kid == k)) {
                continue;
            }
            if let Err(e) = self.license(p).await {
                log::warn!("widevine: another pssh of the stream: {e}");
            }
        }
        Ok(())
    }

    async fn license(&self, pssh: &Pssh) -> Result<()> {
        let mut licensed = self.licensed.lock().await;
        if licensed.contains(&pssh.bytes) {
            return Ok(());
        }
        self.cdm.license(pssh.clone(), self.exchange.clone()).await.map_err(|e| self.error(e))?;
        licensed.insert(pssh.bytes.clone());
        drop(licensed);
        self.probe().await;
        Ok(())
    }

    /// Keys the CDM holds but only decrypts in its own decoder, found by trying each new usable key once.
    async fn probe(&self) {
        let fresh: Vec<Vec<u8>> = self.cdm.keys().into_iter().filter(Key::usable).map(|k| k.kid).filter(|k| lock(&self.probed).insert(k.clone())).collect();
        for kid in fresh {
            if let Err(e @ (DrmError::SecureDecodeRequired { .. } | DrmError::DecoderOnly { .. })) = self.cdm.probe(&kid, false).await {
                lock(&self.decode_only).insert(kid, e);
            }
        }
    }

    /// The keys of the video representations tried with a video-shaped sample, and an error when none of them can
    /// play here.
    async fn probe_video(&self, reps: &[Rep]) -> Result<()> {
        let mut kids: Vec<[u8; 16]> = reps.iter().filter(|r| r.video).filter_map(|r| r.kid).collect();
        kids.dedup();
        for kid in &kids {
            if !self.playable(kid) || lock(&self.video_refused).contains_key(&kid[..]) {
                continue;
            }
            if let Err(e @ (DrmError::SecureDecodeRequired { .. } | DrmError::DecoderOnly { .. })) = self.cdm.probe(kid, true).await {
                lock(&self.video_refused).insert(kid.to_vec(), e);
            }
        }
        let video: Vec<&Rep> = reps.iter().filter(|r| r.video && r.kid.is_some()).collect();
        if video.is_empty() || video.iter().any(|r| self.keep(r)) {
            return Ok(());
        }
        let refused = video.iter().find_map(|r| r.kid.and_then(|k| lock(&self.video_refused).get(&k[..]).or(lock(&self.decode_only).get(&k[..])).cloned()));
        match refused {
            Some(e) => Err(drm_error(e)),
            None => self.check(),
        }
    }

    fn playable(&self, kid: &[u8]) -> bool {
        self.cdm.keys().iter().any(|k| k.kid == kid && k.usable()) && !lock(&self.decode_only).contains_key(kid)
    }

    /// Whether a representation can play: its key, when the manifest names it, is one decrypted here.
    fn keep(&self, rep: &Rep) -> bool {
        rep.kid.is_none_or(|k| self.playable(&k) && !(rep.video && lock(&self.video_refused).contains_key(&k[..])))
    }

    /// An error unless some key can be decrypted here.
    fn check(&self) -> Result<()> {
        let keys = self.cdm.keys();
        if keys.iter().any(|k| self.playable(&k.kid)) {
            return Ok(());
        }
        if let Some(e) = lock(&self.decode_only).values().next() {
            return Err(drm_error(e.clone()));
        }
        let statuses: Vec<String> = keys.iter().map(|k| format!("{}: {}", hex(&k.kid), k.status_name())).collect();
        Err(drm_error(DrmError::NoUsableKey(if statuses.is_empty() { "none".into() } else { statuses.join(", ") })))
    }

    /// The Widevine `pssh` of the first of `inits` that has one.
    async fn pssh_of_inits(&self, session: &Session, http: &Http, inits: impl Iterator<Item = Source>) -> Result<Vec<Pssh>> {
        for (url, range) in inits {
            let fetched = session.fetch(http, &url, range).await?;
            let found: Vec<Pssh> = Init::parse(&fetched.body).map(|i| i.pssh).unwrap_or_default().into_iter().filter(Pssh::is_widevine).collect();
            if !found.is_empty() {
                return Ok(found);
            }
        }
        Ok(Vec::new())
    }

    async fn init(&self, session: &Session, http: &Http, track: &str) -> Result<Arc<Init>> {
        if let Some(init) = lock(&self.inits).get(track).cloned() {
            return Ok(init);
        }
        let source = lock(&self.sources).get(track).cloned();
        let Some((url, range)) = source else { return Err(drm_error(DrmError::Malformed(format!("no init segment for {track}")))) };
        let fetched = session.fetch(http, &url, range).await?;
        let init = Arc::new(Init::parse(&fetched.body).map_err(drm_error)?);
        lock(&self.inits).insert(track.to_string(), init.clone());
        Ok(init)
    }

    async fn index(&self, session: &Session, http: &Http, rep: &Rep) -> Result<Index> {
        if let Some(index) = lock(&self.indexes).get(&rep.track).cloned() {
            return Ok(index);
        }
        let Some(((start, end), _)) = rep.single else { return Err(drm_error(DrmError::Malformed("no index".into()))) };
        let fetched = session.fetch(http, &rep.base, Some((start, end))).await?;
        let index = Index::parse(&fetched.body, start).map_err(drm_error)?;
        lock(&self.indexes).insert(rep.track.clone(), index.clone());
        Ok(index)
    }

    /// `body` made clear in place, whether it is an init or a media segment of `track`. Anything else is left.
    async fn clear(&self, session: &Session, http: &Http, track: &str, body: &mut [u8], origin: u64) -> Result<()> {
        let Ok(boxes) = mp4::children(body, 0..body.len()) else { return Ok(()) };
        if boxes.iter().any(|b| &b.kind == b"moov") {
            let init = cenc::clear_init_segment(body).map_err(drm_error)?;
            lock(&self.inits).insert(track.to_string(), Arc::new(init));
            return Ok(());
        }
        if !boxes.iter().any(|b| &b.kind == b"moof") {
            return Ok(());
        }
        let init = self.init(session, http, track).await?;
        let segment = Segment::parse(body, &init, origin).map_err(drm_error)?;
        let kids = segment.kids();
        let known = self.cdm.keys();
        if kids.iter().any(|kid| !known.iter().any(|k| &k.kid == kid)) {
            for pssh in segment.pssh.iter().chain(&init.pssh).filter(|p| p.is_widevine()) {
                self.license(pssh).await?;
            }
        }
        if let Some(e) = kids.iter().find_map(|k| lock(&self.decode_only).get(k).cloned()) {
            return Err(drm_error(e));
        }
        cenc::clear_segment(body, &segment, &*self.cdm).await.map_err(|e| self.error(e))
    }
}

/// A whole upstream answer, or the byte range of it that was asked.
pub(super) struct Fetched {
    pub url: Url,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
    /// The size of the whole resource, when the server said.
    pub total: Option<u64>,
}

impl Session {
    /// `url` (a byte range of it) as a whole, through the provider's filter when the playback asks.
    pub(super) async fn fetch(&self, http: &Http, url: &Url, range: Option<(u64, u64)>) -> Result<Fetched> {
        let mut answer = self.open_upstream(http, url, range.map(|(start, end)| format!("bytes={start}-{end}"))).await?;
        if !(200..300).contains(&answer.status) {
            return Err(IptvError::Http(answer.status));
        }
        let partial = answer.status == 206;
        let total = answer.header("content-range").and_then(|v| v.rsplit('/').next()?.trim().parse().ok());
        let content_type = answer.header("content-type").map(str::to_string);
        let mut body = Vec::new();
        while let Some(chunk) = answer.chunk().await? {
            if body.len() + chunk.len() > Http::SEND_LIMIT {
                return Err(IptvError::Network(format!("{url}: larger than {} MB", Http::SEND_LIMIT >> 20)));
            }
            body.extend_from_slice(&chunk);
        }
        if let Some((start, end)) = range.filter(|_| !partial) {
            let end = (end as usize).min(body.len().saturating_sub(1));
            body = body.get(start as usize..=end).map(<[u8]>::to_vec).unwrap_or_default();
        }
        let mut media = Media { content_type, body };
        if self.relay.filter {
            media = self.provider.filter(&self.config, &answer.url, media).await?;
        }
        Ok(Fetched { url: answer.url, content_type: media.content_type, body: media.body, total })
    }

    /// The relay's address for `resource`: a base the player names segments under, or one resource.
    fn local_resource(&self, port: u16, token: &str, resource: &Resource, extension: &str) -> Url {
        let Some(protected) = &self.protected else { return self.local(port, token, &Url::parse("http://invalid/").expect("URL")) };
        let number = lock(&protected.resources).number(resource);
        let address = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
        let name = match resource {
            Resource::Base { .. } => format!("p{number}/"),
            _ if extension.is_empty() => format!("p{number}"),
            _ => format!("p{number}.{extension}"),
        };
        Url::parse(&format!("http://{address}/{token}/{name}")).expect("relay address")
    }
}

/// The answer to `name` (with its `query`) in a protected session.
#[allow(clippy::too_many_arguments)]
pub(super) async fn answer(http: &Http, session: &Session, protected: &Protected, name: &str, query: Option<&str>, range: Option<String>, port: u16, token: &str) -> Response<Body> {
    let (head, rest) = name.split_once('/').map_or((name, None), |(head, rest)| (head, Some(rest)));
    let number = head.strip_prefix('p').and_then(|h| h.split('.').next()).and_then(|n| n.parse::<usize>().ok());
    let Some(resource) = number.and_then(|n| lock(&protected.resources).get(n)) else { return status(StatusCode::NOT_FOUND) };
    let answered = match resource {
        Resource::Manifest(url) => manifest(http, session, protected, &url, port, token).await,
        Resource::Plain(url) => session.relay_one(http, url, range, port, token).await,
        Resource::Base { url, track } => {
            let rest = rest.unwrap_or_default();
            let relative = match query {
                Some(query) => format!("{rest}?{query}"),
                None => rest.to_string(),
            };
            match url.join(&relative) {
                Ok(upstream) if !rest.is_empty() => segment(http, session, protected, upstream, None, &track, range).await,
                _ => Ok(status(StatusCode::NOT_FOUND)),
            }
        }
        Resource::Segment { url, range: own, track } => segment(http, session, protected, url, own, &track, range).await,
    };
    match answered {
        Ok(response) => response,
        Err(IptvError::Http(code)) => status(StatusCode::from_u16(code).unwrap_or(StatusCode::BAD_GATEWAY)),
        Err(e) => {
            log::warn!("relay: protected stream: {e}");
            status(StatusCode::BAD_GATEWAY)
        }
    }
}

/// The track of an HLS init segment.
fn track_key((url, range): &Source) -> String {
    match range {
        Some((start, end)) => format!("{url}#{start}-{end}"),
        None => url.to_string(),
    }
}

/// The extension players guess formats from, when it is plain.
fn extension(url: &Url) -> String {
    let name = url.path_segments().and_then(|mut s| s.next_back()).unwrap_or_default();
    name.rsplit_once('.').map(|(_, ext)| ext).filter(|ext| (1..=5).contains(&ext.len()) && ext.chars().all(|c| c.is_ascii_alphanumeric())).unwrap_or_default().to_string()
}

async fn manifest(http: &Http, session: &Session, protected: &Protected, url: &Url, port: u16, token: &str) -> Result<Response<Body>> {
    let fetched = session.fetch(http, url, None).await?;
    let text = String::from_utf8_lossy(&fetched.body);
    let protection = &protected.protection;
    if hls::is_playlist(&text) {
        let body = hls::rewrite(&text, &fetched.url, &mut |named| match named {
            Named::Playlist(url) => session.local_resource(port, token, &Resource::Manifest(url), "m3u8"),
            Named::Plain(url) => {
                let extension = extension(&url);
                session.local_resource(port, token, &Resource::Plain(url), &extension)
            }
            Named::Segment { url, range, track } => {
                let key = track_key(&track);
                lock(&protection.sources).insert(key.clone(), track);
                let extension = extension(&url);
                session.local_resource(port, token, &Resource::Segment { url, range, track: key }, &extension)
            }
        })
        .map_err(drm_error)?;
        return Ok(bytes_response(StatusCode::OK, body.into_bytes(), Some("application/vnd.apple.mpegurl"), None));
    }
    let Some(manifest) = Manifest::parse(&text, &fetched.url) else { return Err(IptvError::BadResponse(format!("{url}: neither DASH nor HLS"))) };
    protection.remember(&manifest.reps);
    let mut indexes = HashMap::new();
    for rep in manifest.reps.iter().filter(|r| r.single.is_some()) {
        indexes.insert(rep.track.clone(), protection.index(session, http, rep).await?);
    }
    let body = manifest.rewrite(
        |rep| protection.keep(rep),
        |rep| match (indexes.remove(&rep.track), rep.single) {
            (Some(index), Some(((index_start, _), init))) => {
                let segment = |range| Resource::Segment { url: rep.base.clone(), range: Some(range), track: rep.track.clone() };
                let init = session.local_resource(port, token, &segment(init.unwrap_or((0, index_start.saturating_sub(1)))), "mp4");
                let media = index.references.iter().map(|(r, _)| session.local_resource(port, token, &segment((r.start, r.end - 1)), "m4s")).collect();
                Addressing::List { index, init, media }
            }
            _ if rep.whole => {
                let extension = extension(&rep.base);
                Addressing::Base(session.local_resource(port, token, &Resource::Segment { url: rep.base.clone(), range: None, track: rep.track.clone() }, &extension))
            }
            _ => Addressing::Base(session.local_resource(port, token, &Resource::Base { url: rep.base.clone(), track: rep.track.clone() }, "")),
        },
    );
    Ok(bytes_response(StatusCode::OK, body.into_bytes(), Some("application/dash+xml"), None))
}

/// A segment of `track`, decrypted. `own` is its byte range upstream; a player's `range` with both ends, on a
/// segment without one, is taken for it (a segment list's `mediaRange`), any other is cut from the clear segment.
#[allow(clippy::too_many_arguments)]
async fn segment(http: &Http, session: &Session, protected: &Protected, url: Url, own: Option<(u64, u64)>, track: &str, range: Option<String>) -> Result<Response<Body>> {
    let asked = range.as_deref().and_then(parse_range);
    let (upstream, cut) = match (own, asked) {
        (Some(own), asked) => (Some(own), asked),
        (None, Some((start, Some(end)))) => (Some((start, end)), None),
        (None, asked) => (None, asked),
    };
    let mut fetched = session.fetch(http, &url, upstream).await?;
    protected.protection.clear(session, http, track, &mut fetched.body, upstream.map_or(0, |r| r.0)).await?;
    let content_type = fetched.content_type.as_deref();
    if let (None, Some((start, _))) = (own, upstream) {
        let end = start + (fetched.body.len() as u64).saturating_sub(1);
        let total = fetched.total.map_or("*".to_string(), |t| t.to_string());
        return Ok(bytes_response(StatusCode::PARTIAL_CONTENT, fetched.body, content_type, Some(format!("bytes {start}-{end}/{total}"))));
    }
    Ok(match cut {
        None | Some((0, None)) => bytes_response(StatusCode::OK, fetched.body, content_type, None),
        Some((start, end)) => cut_response(fetched.body, start, end, content_type),
    })
}

/// `bytes=a-b` or `bytes=a-`.
fn parse_range(header: &str) -> Option<(u64, Option<u64>)> {
    let (start, end) = header.trim().strip_prefix("bytes=")?.split_once('-')?;
    Some((start.trim().parse().ok()?, if end.trim().is_empty() { None } else { Some(end.trim().parse().ok()?) }))
}

fn cut_response(body: Vec<u8>, start: u64, end: Option<u64>, content_type: Option<&str>) -> Response<Body> {
    let total = body.len() as u64;
    let end = end.unwrap_or(u64::MAX).min(total.saturating_sub(1));
    if start > end || start >= total {
        return status(StatusCode::RANGE_NOT_SATISFIABLE);
    }
    let part = body[start as usize..=end as usize].to_vec();
    bytes_response(StatusCode::PARTIAL_CONTENT, part, content_type, Some(format!("bytes {start}-{end}/{total}")))
}

fn bytes_response(code: StatusCode, body: Vec<u8>, content_type: Option<&str>, content_range: Option<String>) -> Response<Body> {
    let mut response = Response::new(Full::new(Bytes::from(body)).map_err(|e| match e {}).boxed_unsync());
    *response.status_mut() = code;
    if let Some(value) = content_type.and_then(|ct| HeaderValue::from_str(ct).ok()) {
        response.headers_mut().insert(CONTENT_TYPE, value);
    }
    if let Some(value) = content_range.and_then(|cr| HeaderValue::from_str(&cr).ok()) {
        response.headers_mut().insert(CONTENT_RANGE, value);
    }
    response
}

#[cfg(test)]
mod tests {
    use nightglass_drm::cenc::ClearKeys;
    use nightglass_drm::testing::*;
    use wiremock::matchers::path;
    use wiremock::{Mock, MockServer, Request as Asked, Respond, ResponseTemplate};

    use super::*;
    use crate::{Catalog, KeySystem, Manifest as Kind, ProviderInfo};

    const KID: [u8; 16] = [5; 16];
    const KEY: [u8; 16] = [0x42; 16];
    /// A Clear Key licence for `KID`, `KEY`.
    const LICENSE: &str = r#"{"keys":[{"kty":"oct","kid":"BQUFBQUFBQUFBQUFBQUFBQ","k":"QkJCQkJCQkJCQkJCQkJCQg"}]}"#;

    /// Answers licence requests as its licence server would, and remembers them.
    struct Licensed {
        info: ProviderInfo,
        answer: std::result::Result<String, IptvError>,
        asked: Mutex<Vec<LicenseRequest>>,
    }

    impl Provider for Licensed {
        fn info(&self) -> &ProviderInfo {
            &self.info
        }
        fn catalog<'a>(&'a self, _config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
            Box::pin(async { Ok(Catalog::default()) })
        }
        fn license<'a>(&'a self, _config: &'a Config, request: &'a LicenseRequest) -> BoxFuture<'a, Result<Vec<u8>>> {
            lock(&self.asked).push(request.clone());
            Box::pin(async move { self.answer.clone().map(String::into_bytes) })
        }
    }

    fn licensed(answer: std::result::Result<&str, IptvError>) -> Arc<Licensed> {
        let info = ProviderInfo { id: "l".into(), name: "L".into(), fields: Vec::new(), resolves_streams: true, presets: Vec::new() };
        Arc::new(Licensed { info, answer: answer.map(str::to_string), asked: Mutex::default() })
    }

    fn track() -> TrackSpec {
        TrackSpec { id: 1, audio: false, scheme: *b"cenc", crypto: crypto(KID, 8, None, (0, 0)) }
    }

    fn plain() -> Vec<SampleSpec> {
        vec![SampleSpec { data: (0..90).collect(), iv: vec![1; 8], subsamples: vec![(10, 80)] }, SampleSpec { data: vec![7; 33], iv: vec![2; 8], subsamples: vec![] }]
    }

    fn mdat(data: &[u8]) -> Vec<u8> {
        data[data.windows(4).position(|w| w == b"mdat").unwrap()..].to_vec()
    }

    fn playback(server: &MockServer, path: &str) -> Playback {
        let mut playback = Playback::from(Url::parse(&format!("{}{path}", server.uri())).unwrap());
        playback.drm = Some(crate::Drm { key_system: KeySystem::Widevine, manifest: Kind::Dash, certificate: None, pssh: None, context: "ctx".into() });
        playback
    }

    async fn get(url: &str, range: Option<&str>) -> (u16, Option<String>, Vec<u8>) {
        let mut request = reqwest::Client::new().get(url);
        if let Some(range) = range {
            request = request.header("Range", range);
        }
        let r = request.send().await.unwrap();
        let content_range = r.headers().get("content-range").map(|v| v.to_str().unwrap().to_string());
        (r.status().as_u16(), content_range, r.bytes().await.unwrap().to_vec())
    }

    fn between<'a>(text: &'a str, from: &str, to: &str) -> &'a str {
        let start = text.find(from).unwrap() + from.len();
        &text[start..start + text[start..].find(to).unwrap()]
    }

    #[tokio::test]
    async fn a_dash_stream_plays_clear_through_the_relay() {
        let server = MockServer::start().await;
        let mpd = r#"<MPD type="static"><Period><AdaptationSet><ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" value="cenc" default_KID="05050505-0505-0505-0505-050505050505"/><SegmentTemplate initialization="$RepresentationID$/init.mp4" media="$RepresentationID$/$Number$.m4s" duration="2" timescale="1"/><Representation id="v" bandwidth="1"/></AdaptationSet></Period></MPD>"#;
        let init = init_segment(&[track()], &[Pssh::new(WIDEVINE, &[[0x12, 16].as_slice(), &KID].concat())]);
        let segment = media_segment(&track(), &encrypt(&track(), &KEY, &plain()), Aux::Senc, &[]);
        Mock::given(path("/s/manifest.mpd")).respond_with(ResponseTemplate::new(200).set_body_raw(mpd, "application/dash+xml")).mount(&server).await;
        Mock::given(path("/s/v/init.mp4")).respond_with(ResponseTemplate::new(200).set_body_raw(init.clone(), "video/mp4")).mount(&server).await;
        Mock::given(path("/s/v/1.m4s")).respond_with(ResponseTemplate::new(200).set_body_raw(segment.clone(), "video/mp4")).mount(&server).await;

        let provider = licensed(Ok(LICENSE));
        let relay = RelayServer::new(Http::new());
        let opened = relay.open_protected(provider.clone(), Config::new(), playback(&server, "/s/manifest.mpd"), Arc::new(ClearKeys::default())).await.unwrap();
        assert_eq!((opened.url.host_str(), opened.drm.is_none()), (Some("127.0.0.1"), true));
        let asked = lock(&provider.asked).clone();
        assert_eq!(asked.len(), 1, "the licence is had before the player starts");
        assert_eq!((asked[0].kind, asked[0].context.as_str(), asked[0].url.path()), (LicenseKind::Request, "ctx", "/s/manifest.mpd"));
        assert!(String::from_utf8_lossy(&asked[0].challenge).contains("BQUFBQUFBQUFBQUFBQUFBQ"), "the pssh came from the init segment");

        let (status, _, body) = get(opened.url.as_str(), None).await;
        let manifest = String::from_utf8(body).unwrap();
        assert_eq!(status, 200);
        assert!(!manifest.contains("ContentProtection"), "{manifest}");
        let base = between(&manifest, "<BaseURL>", "</BaseURL>").to_string();
        assert!(base.starts_with("http://127.0.0.1:") && base.ends_with('/'), "{base}");

        let (status, _, clear_init) = get(&format!("{base}v/init.mp4"), Some("bytes=0-")).await;
        assert_eq!((status, clear_init.len()), (200, init.len()));
        assert!(find(&clear_init, b"avc1") && !find(&clear_init, b"encv") && !find(&clear_init, b"pssh"));
        let (status, _, clear) = get(&format!("{base}v/1.m4s"), None).await;
        assert_eq!(status, 200);
        assert_eq!(mdat(&clear), mdat(&media_segment(&track(), &plain(), Aux::Senc, &[])));
        assert!(!find(&clear, b"senc"));
        let (status, content_range, part) = get(&format!("{base}v/1.m4s"), Some("bytes=4-")).await;
        assert_eq!((status, content_range, part), (206, Some(format!("bytes 4-{}/{}", clear.len() - 1, clear.len())), clear[4..].to_vec()));
        assert_eq!(get(&format!("{base}v/9.m4s"), None).await.0, 404, "upstream 404 is passed on");
        assert_eq!(lock(&provider.asked).len(), 1);
    }

    /// Byte ranges of one file, as a server that honours `Range` answers them.
    struct Ranged(Vec<u8>);

    impl Respond for Ranged {
        fn respond(&self, request: &Asked) -> ResponseTemplate {
            let range = request.headers.get("range").and_then(|v| v.to_str().ok()).and_then(parse_range);
            match range {
                Some((start, end)) => {
                    let end = end.unwrap_or(self.0.len() as u64 - 1);
                    ResponseTemplate::new(206).insert_header("content-range", format!("bytes {start}-{end}/{}", self.0.len()).as_str()).set_body_raw(self.0[start as usize..=end as usize].to_vec(), "video/mp4")
                }
                None => ResponseTemplate::new(200).set_body_raw(self.0.clone(), "video/mp4"),
            }
        }
    }

    #[tokio::test]
    async fn a_single_file_representation_becomes_a_list_of_segments() {
        let server = MockServer::start().await;
        let init = init_segment(&[track()], &[]);
        let segment = media_segment(&track(), &encrypt(&track(), &KEY, &plain()), Aux::Senc, &[]);
        let mut sidx = vec![0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0x03, 0xe8, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1];
        sidx.extend_from_slice(&(segment.len() as u32).to_be_bytes());
        sidx.extend_from_slice(&[0, 0, 0x07, 0xd0, 0x90, 0, 0, 0]);
        let sidx = bx(b"sidx", &sidx);
        let file = [init.clone(), sidx.clone(), segment.clone()].concat();
        let (index_start, index_end) = (init.len(), init.len() + sidx.len() - 1);
        let mpd = format!(
            r#"<MPD><Period><AdaptationSet><ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"><pssh>{}</pssh></ContentProtection><Representation id="a"><BaseURL>a.mp4</BaseURL><SegmentBase indexRange="{index_start}-{index_end}"><Initialization range="0-{}"/></SegmentBase></Representation></AdaptationSet></Period></MPD>"#,
            "AAAAMnBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAABISEAUFBQUFBQUFBQUFBQUFBQU=",
            init.len() - 1
        );
        Mock::given(path("/m.mpd")).respond_with(ResponseTemplate::new(200).set_body_string(mpd)).mount(&server).await;
        Mock::given(path("/a.mp4")).respond_with(Ranged(file)).mount(&server).await;

        let relay = RelayServer::new(Http::new());
        let opened = relay.open_protected(licensed(Ok(LICENSE)), Config::new(), playback(&server, "/m.mpd"), Arc::new(ClearKeys::default())).await.unwrap();
        let manifest = String::from_utf8(get(opened.url.as_str(), None).await.2).unwrap();
        assert!(manifest.contains(r#"<S t="0" d="2000"/>"#) && !manifest.contains("SegmentBase"), "{manifest}");
        let init_url = between(&manifest, r#"<Initialization sourceURL=""#, "\"").to_string();
        let media_url = between(&manifest, r#"<SegmentURL media=""#, "\"").to_string();
        let clear_init = get(&init_url, None).await.2;
        assert_eq!(clear_init.len(), init.len());
        assert!(!find(&clear_init, b"encv"));
        let clear = get(&media_url, Some("bytes=0-")).await.2;
        assert_eq!(clear.len(), segment.len());
        assert_eq!(mdat(&clear), mdat(&media_segment(&track(), &plain(), Aux::Senc, &[])));
    }

    #[tokio::test]
    async fn a_segment_list_range_goes_upstream_and_comes_back_clear() {
        let server = MockServer::start().await;
        let init = init_segment(&[track()], &[Pssh::new(WIDEVINE, &[[0x12, 16].as_slice(), &KID].concat())]);
        let segment = media_segment(&track(), &encrypt(&track(), &KEY, &plain()), Aux::SaizSaio, &[]);
        let file = [init.clone(), segment.clone()].concat();
        let (start, end) = (init.len(), file.len() - 1);
        let mpd = format!(
            r#"<MPD><Period><AdaptationSet><Representation id="v"><BaseURL>v.mp4</BaseURL><SegmentList duration="2"><Initialization range="0-{}"/><SegmentURL mediaRange="{start}-{end}"/></SegmentList></Representation></AdaptationSet></Period></MPD>"#,
            init.len() - 1
        );
        Mock::given(path("/m.mpd")).respond_with(ResponseTemplate::new(200).set_body_string(mpd)).mount(&server).await;
        Mock::given(path("/v.mp4")).respond_with(Ranged(file.clone())).mount(&server).await;
        let relay = RelayServer::new(Http::new());
        let opened = relay.open_protected(licensed(Ok(LICENSE)), Config::new(), playback(&server, "/m.mpd"), Arc::new(ClearKeys::default())).await.unwrap();
        let manifest = String::from_utf8(get(opened.url.as_str(), None).await.2).unwrap();
        let base = between(&manifest, "<BaseURL>", "</BaseURL>").to_string();
        let (status, content_range, clear) = get(&format!("{base}v.mp4"), Some(&format!("bytes={start}-{end}"))).await;
        assert_eq!((status, content_range), (206, Some(format!("bytes {start}-{end}/{}", file.len()))));
        assert_eq!(mdat(&clear), mdat(&media_segment(&track(), &plain(), Aux::SaizSaio, &[])));
    }

    #[tokio::test]
    async fn licence_failures_reach_the_caller() {
        let server = MockServer::start().await;
        let mpd = r#"<MPD><Period><AdaptationSet><ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"><pssh>AAAAMnBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAABISEAUFBQUFBQUFBQUFBQUFBQU=</pssh></ContentProtection><Representation id="v"/></AdaptationSet></Period></MPD>"#;
        Mock::given(path("/m.mpd")).respond_with(ResponseTemplate::new(200).set_body_string(mpd)).mount(&server).await;
        let relay = RelayServer::new(Http::new());
        let refused = relay.open_protected(licensed(Err(IptvError::Http(403))), Config::new(), playback(&server, "/m.mpd"), Arc::new(ClearKeys::default())).await;
        assert_eq!(refused.unwrap_err(), IptvError::Http(403), "the plugin's own error");
        let other_key = r#"{"keys":[{"kty":"oct","kid":"BgYGBgYGBgYGBgYGBgYGBg","k":"QkJCQkJCQkJCQkJCQkJCQg"}]}"#;
        let opened = relay.open_protected(licensed(Ok(other_key)), Config::new(), playback(&server, "/m.mpd"), Arc::new(ClearKeys::default())).await.unwrap();
        let mpd = String::from_utf8(get(opened.url.as_str(), None).await.2).unwrap();
        assert!(mpd.contains(r#"<Representation id="v">"#), "no default_KID: nothing to leave out");
        let unprotected = relay.open_protected(licensed(Ok(LICENSE)), Config::new(), Playback::from(opened.url.clone()), Arc::new(ClearKeys::default())).await;
        assert!(matches!(unprotected, Err(IptvError::Drm(_))));
    }

    #[tokio::test]
    async fn an_hls_stream_plays_clear_through_the_relay() {
        let server = MockServer::start().await;
        let cbcs = TrackSpec { id: 1, audio: false, scheme: *b"cbcs", crypto: crypto(KID, 0, Some(vec![9; 16]), (1, 9)) };
        let plain: Vec<SampleSpec> = (0..2).map(|i| SampleSpec { data: vec![i + 1; 200], iv: vec![], subsamples: vec![(5, 160), (35, 0)] }).collect();
        let init = init_segment(std::slice::from_ref(&cbcs), &[]);
        let segment = media_segment(&cbcs, &encrypt(&cbcs, &KEY, &plain), Aux::Senc, &[]);
        let key = r#"METHOD=SAMPLE-AES,URI="data:text/plain;base64,AAAAMnBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAABISEAUFBQUFBQUFBQUFBQUFBQU=",KEYFORMAT="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed""#;
        let master = format!("#EXTM3U\n#EXT-X-SESSION-KEY:{key}\n#EXT-X-STREAM-INF:BANDWIDTH=1\nv/media.m3u8\n");
        let media = format!("#EXTM3U\n#EXT-X-TARGETDURATION:2\n#EXT-X-KEY:{key}\n#EXT-X-MAP:URI=\"init.mp4\"\n#EXTINF:2,\n1.m4s\n#EXT-X-ENDLIST\n");
        Mock::given(path("/h/master.m3u8")).respond_with(ResponseTemplate::new(200).set_body_raw(master, "application/vnd.apple.mpegurl")).mount(&server).await;
        Mock::given(path("/h/v/media.m3u8")).respond_with(ResponseTemplate::new(200).set_body_raw(media, "application/vnd.apple.mpegurl")).mount(&server).await;
        Mock::given(path("/h/v/init.mp4")).respond_with(ResponseTemplate::new(200).set_body_raw(init.clone(), "video/mp4")).mount(&server).await;
        Mock::given(path("/h/v/1.m4s")).respond_with(ResponseTemplate::new(200).set_body_raw(segment, "video/mp4")).mount(&server).await;

        let relay = RelayServer::new(Http::new());
        let opened = relay.open_protected(licensed(Ok(LICENSE)), Config::new(), playback(&server, "/h/master.m3u8"), Arc::new(ClearKeys::default())).await.unwrap();
        assert!(opened.url.path().ends_with(".m3u8"));
        let master = String::from_utf8(get(opened.url.as_str(), None).await.2).unwrap();
        assert!(!master.contains("KEY"), "{master}");
        let variant = master.lines().find(|l| l.starts_with("http")).unwrap().to_string();
        let media = String::from_utf8(get(&variant, None).await.2).unwrap();
        assert!(!media.contains("EXT-X-KEY"), "{media}");
        let init_url = between(&media, "#EXT-X-MAP:URI=\"", "\"").to_string();
        let segment_url = media.lines().find(|l| l.starts_with("http")).unwrap().to_string();
        assert!(!find(&get(&init_url, None).await.2, b"encv"));
        let clear = get(&segment_url, None).await.2;
        assert_eq!(mdat(&clear), mdat(&media_segment(&cbcs, &plain, Aux::Senc, &[])));
    }

    #[tokio::test]
    async fn sample_aes_transport_streams_are_refused_before_playing() {
        let server = MockServer::start().await;
        let media = "#EXTM3U\n#EXT-X-KEY:METHOD=SAMPLE-AES,URI=\"skd://k\",KEYFORMAT=\"com.apple.streamingkeydelivery\"\n#EXTINF:2,\n1.ts\n";
        Mock::given(path("/ts.m3u8")).respond_with(ResponseTemplate::new(200).set_body_string(media)).mount(&server).await;
        let relay = RelayServer::new(Http::new());
        let refused = relay.open_protected(licensed(Ok(LICENSE)), Config::new(), playback(&server, "/ts.m3u8"), Arc::new(ClearKeys::default())).await;
        assert_eq!(refused.unwrap_err(), IptvError::Drm(DrmError::Unsupported("HLS in MPEG-TS with SAMPLE-AES".into()).to_string()));
    }

    #[test]
    fn the_helper_is_found_beside_the_app() {
        let dir = std::env::temp_dir().join(format!("nightglass-helper-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let suffix = std::env::consts::EXE_SUFFIX;
        assert_eq!(helper_in(Some(&dir)), dir.join(format!("nightglass-cdm{suffix}")), "as Cargo builds it");
        std::fs::write(dir.join(format!("nightglass-widevine{suffix}")), b"").unwrap();
        assert_eq!(helper_in(Some(&dir)), dir.join(format!("nightglass-widevine{suffix}")), "as the bundle ships it");
        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// Clear Key, but refusing video-shaped samples for some keys, as Google's desktop CDM does.
    struct DecoderOnly(ClearKeys, Vec<[u8; 16]>);

    impl Cdm for DecoderOnly {
        fn decrypt(&self, samples: Vec<nightglass_drm::protocol::Sample>, data: Vec<u8>) -> BoxFuture<'_, std::result::Result<Vec<u8>, DrmError>> {
            self.0.decrypt(samples, data)
        }
        fn license(&self, pssh: Pssh, exchange: Exchange) -> BoxFuture<'_, std::result::Result<Vec<Key>, DrmError>> {
            self.0.license(pssh, exchange)
        }
        fn probe<'a>(&'a self, kid: &'a [u8], video: bool) -> BoxFuture<'a, std::result::Result<(), DrmError>> {
            Box::pin(async move {
                match self.1.iter().any(|k| k == kid) && video {
                    true => Err(DrmError::DecoderOnly { kid: hex(kid) }),
                    false => self.0.probe(kid, video).await,
                }
            })
        }
        fn keys(&self) -> Vec<Key> {
            self.0.keys()
        }
    }

    #[tokio::test]
    async fn video_widevine_only_decodes_itself_is_refused_before_playing() {
        let server = MockServer::start().await;
        let pssh = "AAAAMnBzc2gAAAAA7e+LqXnWSs6jyCfc1R0h7QAAABISEAUFBQUFBQUFBQUFBQUFBQU=";
        let set = |kind: &str, kid: &str, id: &str| format!(r#"<AdaptationSet contentType="{kind}"><ContentProtection schemeIdUri="urn:mpeg:dash:mp4protection:2011" default_KID="{kid}"/><ContentProtection schemeIdUri="urn:uuid:edef8ba9-79d6-4ace-a3c8-27dcd51d21ed"><pssh>{pssh}</pssh></ContentProtection><SegmentTemplate media="$Number$.m4s" initialization="i.mp4" duration="2"/><Representation id="{id}"/></AdaptationSet>"#);
        let five = "05050505-0505-0505-0505-050505050505";
        let six = "06060606-0606-0606-0606-060606060606";
        let one_key = format!("<MPD><Period>{}{}</Period></MPD>", set("video", five, "v"), set("audio", five, "a"));
        let two_keys = format!("<MPD><Period>{}{}{}</Period></MPD>", set("video", five, "hd"), set("video", six, "sd"), set("audio", five, "a"));
        Mock::given(path("/one.mpd")).respond_with(ResponseTemplate::new(200).set_body_string(one_key)).mount(&server).await;
        Mock::given(path("/two.mpd")).respond_with(ResponseTemplate::new(200).set_body_string(two_keys)).mount(&server).await;
        let both = r#"{"keys":[{"kty":"oct","kid":"BQUFBQUFBQUFBQUFBQUFBQ","k":"QkJCQkJCQkJCQkJCQkJCQg"},{"kty":"oct","kid":"BgYGBgYGBgYGBgYGBgYGBg","k":"QkJCQkJCQkJCQkJCQkJCQg"}]}"#;
        let relay = RelayServer::new(Http::new());
        let refused = relay.open_protected(licensed(Ok(LICENSE)), Config::new(), playback(&server, "/one.mpd"), Arc::new(DecoderOnly(ClearKeys::default(), vec![KID]))).await;
        assert_eq!(refused.unwrap_err(), IptvError::Drm(DrmError::DecoderOnly { kid: hex(&KID) }.to_string()));
        let opened = relay.open_protected(licensed(Ok(both)), Config::new(), playback(&server, "/two.mpd"), Arc::new(DecoderOnly(ClearKeys::default(), vec![KID]))).await.unwrap();
        let mpd = String::from_utf8(get(opened.url.as_str(), None).await.2).unwrap();
        assert!(!mpd.contains(r#"id="hd""#) && mpd.contains(r#"id="sd""#) && mpd.contains(r#"id="a""#), "the video its key can't decrypt goes, the audio with the same key stays: {mpd}");
    }
}
