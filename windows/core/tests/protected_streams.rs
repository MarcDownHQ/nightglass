//! Real protected streams through the relay, as far as their keys allow: Axinom's public multi-DRM test vectors
//! (https://github.com/Axinom/public-test-vectors), whose keys come only from licence servers this test never asks.
//! A stand-in CDM leaves samples as they are, so what is checked is everything around the decryption: manifests,
//! boxes, sample positions, subsample maps. Set `NIGHTGLASS_NETWORK_TESTS` to run it.

use std::sync::{Arc, Mutex};

use nightglass_core::{BoxFuture, Catalog, Config, Http, KeySystem, LicenseRequest, Manifest, Playback, Provider, ProviderInfo, RelayServer, Result};
use nightglass_drm::cenc::Cdm;
use nightglass_drm::mp4::{Init, Segment, children};
use nightglass_drm::protocol::{Key, Sample};
use nightglass_drm::pssh::Pssh;
use nightglass_drm::{Exchange, Result as DrmResult};
use url::Url;

/// Holds every key a `pssh` names and decrypts nothing, checking each sample's subsamples on the way.
#[derive(Default)]
struct Unlocked {
    keys: Mutex<Vec<Key>>,
    samples: Mutex<usize>,
}

impl Cdm for Unlocked {
    fn decrypt(&self, samples: Vec<Sample>, data: Vec<u8>) -> nightglass_drm::BoxFuture<'_, DrmResult<Vec<u8>>> {
        Box::pin(async move {
            for sample in &samples {
                let covered: u64 = sample.subsamples.iter().map(|&(c, p)| u64::from(c) + u64::from(p)).sum();
                assert!(sample.subsamples.is_empty() || covered == u64::from(sample.size), "subsamples cover {covered} of {}", sample.size);
                assert_eq!(sample.iv.len(), 16);
            }
            *self.samples.lock().unwrap() += samples.len();
            Ok(data)
        })
    }

    fn license(&self, pssh: Pssh, exchange: Exchange) -> nightglass_drm::BoxFuture<'_, DrmResult<Vec<Key>>> {
        Box::pin(async move {
            exchange(nightglass_drm::protocol::MessageKind::LicenseRequest, pssh.bytes.clone()).await?;
            let keys: Vec<Key> = pssh.kids().into_iter().map(|kid| Key { kid: kid.to_vec(), status: Key::USABLE, system_code: 0 }).collect();
            self.keys.lock().unwrap().extend(keys.clone());
            Ok(keys)
        })
    }

    fn probe<'a>(&'a self, _kid: &'a [u8], _video: bool) -> nightglass_drm::BoxFuture<'a, DrmResult<()>> {
        Box::pin(async { Ok(()) })
    }

    fn keys(&self) -> Vec<Key> {
        self.keys.lock().unwrap().clone()
    }
}

struct NoServer(ProviderInfo);

impl Provider for NoServer {
    fn info(&self) -> &ProviderInfo {
        &self.0
    }
    fn catalog<'a>(&'a self, _config: &'a Config) -> BoxFuture<'a, Result<Catalog>> {
        Box::pin(async { Ok(Catalog::default()) })
    }
    fn license<'a>(&'a self, _config: &'a Config, _request: &'a LicenseRequest) -> BoxFuture<'a, Result<Vec<u8>>> {
        Box::pin(async { Ok(Vec::new()) })
    }
}

async fn get(url: &str) -> Vec<u8> {
    let r = reqwest::get(url).await.unwrap();
    assert!(r.status().is_success(), "{url}: {}", r.status());
    r.bytes().await.unwrap().to_vec()
}

fn kinds(data: &[u8]) -> Vec<String> {
    children(data, 0..data.len()).unwrap().iter().map(|b| String::from_utf8_lossy(&b.kind).into_owned()).collect()
}

/// The first video representation's init and first segment, through the relay.
async fn first_video(manifest: &str, base: &Url) -> (Vec<u8>, Vec<u8>) {
    let video = &manifest[manifest.find("video/mp4").expect("a video set")..];
    if let Some(at) = video.find("<SegmentURL media=\"") {
        let init = &video[video.find("sourceURL=\"").unwrap() + 11..];
        let media = &video[at + 19..];
        return (get(&init[..init.find('"').unwrap()]).await, get(&media[..media.find('"').unwrap()]).await);
    }
    let local = &video[video.find("<BaseURL>").unwrap() + 9..];
    let local = Url::parse(&local[..local.find("</BaseURL>").unwrap()]).unwrap();
    let template = &video[video.find("<SegmentTemplate").unwrap()..];
    let attr = |name: &str| {
        let value = &template[template.find(&format!("{name}=\"")).unwrap() + name.len() + 2..];
        value[..value.find('"').unwrap()].to_string()
    };
    let id = &video[video.find("<Representation id=\"").unwrap() + 20..];
    let id = &id[..id.find('"').unwrap()];
    let number = attr("startNumber");
    let init = attr("initialization").replace("$RepresentationID$", id);
    let media = attr("media").replace("$RepresentationID$", id).replace("$Number$", &number);
    let _ = base;
    (get(local.join(&init).unwrap().as_str()).await, get(local.join(&media).unwrap().as_str()).await)
}

#[tokio::test]
async fn public_protected_streams_come_through_whole() {
    if std::env::var_os("NIGHTGLASS_NETWORK_TESTS").is_none() {
        eprintln!("NIGHTGLASS_NETWORK_TESTS unset: skipped");
        return;
    }
    let provider: Arc<dyn Provider> = Arc::new(NoServer(ProviderInfo { id: "t".into(), name: "T".into(), fields: Vec::new(), resolves_streams: true, presets: Vec::new() }));
    for manifest in ["Dash/protected_dash_1080p_h264_singlekey", "Cmaf/protected_1080p_h264_cbcs"] {
        let url = Url::parse(&format!("https://media.axprod.net/TestVectors/{manifest}/manifest.mpd")).unwrap();
        let mut playback = Playback::from(url.clone());
        playback.drm = Some(nightglass_core::Drm { key_system: KeySystem::Widevine, manifest: Manifest::Dash, certificate: None, pssh: None, context: String::new() });
        let cdm = Arc::new(Unlocked::default());
        let relay = RelayServer::new(Http::new());
        let opened = relay.open_protected(provider.clone(), Config::new(), playback, cdm.clone()).await.unwrap();
        let rewritten = String::from_utf8(get(opened.url.as_str()).await).unwrap();
        assert!(!rewritten.contains("ContentProtection") && !rewritten.contains("axprod"), "{manifest}");
        let (init, segment) = first_video(&rewritten, &url).await;
        assert!(Init::parse(&init).is_ok_and(|i| !i.is_protected() && i.pssh.is_empty()), "{manifest}: {:?}", kinds(&init));
        assert!(!kinds(&segment).is_empty());
        let clear = Init::parse(&init).unwrap();
        assert!(Segment::parse(&segment, &clear, 0).unwrap().encrypted.is_empty(), "{manifest}: nothing left encrypted");
        assert!(*cdm.samples.lock().unwrap() > 0, "{manifest}: samples went to the CDM");
        if std::env::var_os("NIGHTGLASS_FFPROBE").is_some() {
            let local = opened.url.to_string();
            let probe = tokio::task::spawn_blocking(move || std::process::Command::new("ffprobe").args(["-v", "error", "-show_entries", "stream=codec_name,width", "-of", "csv", &local]).output().unwrap()).await.unwrap();
            eprintln!("{manifest}: ffprobe {} {} {}", probe.status, String::from_utf8_lossy(&probe.stdout).replace('\n', " "), String::from_utf8_lossy(&probe.stderr).lines().filter(|l| !l.contains("h264 @")).take(8).collect::<Vec<_>>().join(" | "));
        }
        eprintln!("{manifest}: {} samples, init {:?}, segment {:?}", cdm.samples.lock().unwrap(), kinds(&init), kinds(&segment));
    }
}
