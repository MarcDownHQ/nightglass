//! Real servers, opt-in. Each test returns early when its variable is missing:
//! `NG_M3U=https://iptv-org.github.io/iptv/countries/fr.m3u`, `NG_EPG=https://xmltvfr.fr/xmltv/xmltv.xml`,
//! `NG_XTREAM=http://127.0.0.1:9191 NG_XTREAM_USER=… NG_XTREAM_PASS=…`. Add `-- --nocapture` to see the counts.

use std::time::{Instant, SystemTime, UNIX_EPOCH};

use nightglass_core::{Http, M3uSource, XtreamClient, xmltv};
use url::Url;

fn now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as i64
}

#[tokio::test]
async fn iptv_org_playlist() {
    let Ok(m3u) = std::env::var("NG_M3U") else { return };
    let cat = M3uSource::new(Url::parse(&m3u).unwrap(), None, Http::new()).catalog().await.unwrap();
    println!("[NG] {m3u}: {} channels", cat.channels.len());
    assert!(cat.channels.len() > 50);
}

#[tokio::test]
async fn guide() {
    let Ok(epg) = std::env::var("NG_EPG") else { return };
    let started = Instant::now();
    let p = xmltv::load(&Url::parse(&epg).unwrap(), now() - 3 * 3600..=now() + 86_400, None, &Http::new()).await.unwrap();
    println!("[NG] {epg}: {} programmes in {:.1?}", p.len(), started.elapsed());
    assert!(xmltv::now_next(&p, now()).values().any(|e| e.now.is_some()));
}

#[tokio::test]
async fn xtream_dispatcharr() {
    let Ok(server) = std::env::var("NG_XTREAM") else { return };
    let user = std::env::var("NG_XTREAM_USER").unwrap_or_default();
    let pass = std::env::var("NG_XTREAM_PASS").unwrap_or_default();
    let http = Http::new();
    let cat = XtreamClient::new(Url::parse(&server).unwrap(), &user, &pass, http.clone()).catalog().await.unwrap();
    assert!(cat.channels.len() > 10);
    let guide = xmltv::load(&cat.epg_url.unwrap(), now()..=now() + 86_400, None, &http).await.unwrap();
    assert!(xmltv::now_next(&guide, now()).values().any(|e| e.now.is_some()));
}
