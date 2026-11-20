use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

use nightglass_core::{Browser, Config, Http, IptvError, Media, Providers, Relay};
use nightglass_plugin_host::Host;
use wiremock::matchers::{header, header_regex, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn example() -> &'static Path {
    static BUILT: OnceLock<PathBuf> = OnceLock::new();
    BUILT.get_or_init(|| {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../plugin-example");
        let status = Command::new(env!("CARGO"))
            .args(["build", "--release", "--quiet", "--target", "wasm32-wasip2"])
            .current_dir(&dir)
            .env_remove("CARGO_TARGET_DIR")
            .env_remove("RUSTFLAGS")
            .env_remove("CARGO_ENCODED_RUSTFLAGS")
            .status()
            .expect("cargo");
        assert!(status.success(), "building the example plugin failed (rustup target add wasm32-wasip2?)");
        dir.join("target/wasm32-wasip2/release/nightglass_plugin_example.wasm")
    })
}

fn config(pairs: &[(&str, &str)]) -> Config {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[tokio::test]
async fn describes_itself() {
    let host = Host::new(&Http::new()).unwrap();
    let plugin = host.load(example()).await.unwrap();
    let info = plugin.info();
    assert_eq!(info.id, "json-list");
    assert_eq!(info.fields.iter().map(|f| f.key.as_str()).collect::<Vec<_>>(), ["url", "token", "browser"]);
}

#[tokio::test]
async fn loads_a_catalog_through_the_host() {
    let server = MockServer::start().await;
    let body = r#"{"channels": [
        {"name": "One", "url": "http://s.test/1.ts", "group": "News", "programmes": [
            {"start": 100, "end": 200, "title": "Morning"},
            {"start": 300, "end": 300, "title": "Empty"}
        ]},
        {"name": "Broken", "url": "not an address"}
    ], "epg": "http://s.test/epg.xml"}"#;
    Mock::given(path("/list.json"))
        .and(header("authorization", "Bearer t0ken"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let host = Host::new(&Http::new()).unwrap();
    let plugin = host.load(example()).await.unwrap();
    let url = format!("{}/list.json", server.uri());
    let catalog = plugin.catalog(&config(&[("url", &url), ("token", "t0ken")])).await.unwrap();
    assert_eq!(catalog.channels.len(), 1);
    assert_eq!(catalog.channels[0].group.as_deref(), Some("News"));
    assert_eq!(catalog.epg_url.unwrap().as_str(), "http://s.test/epg.xml");
    assert_eq!(catalog.channels[0].epg_id.as_deref(), Some("http://s.test/1.ts"));
    let titles: Vec<_> = catalog.programmes.iter().map(|p| (p.channel.as_str(), p.title.as_str())).collect();
    assert_eq!(titles, [("http://s.test/1.ts", "Morning")], "an entry that ends when it starts is dropped");
}

#[tokio::test]
async fn browsers_for_the_list_and_the_streams() {
    let server = MockServer::start().await;
    Mock::given(path("/list.json"))
        .and(header_regex("user-agent", "Chrome/"))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"channels": [{"name": "One", "url": "http://s.test/1.m3u8"}]}"#))
        .mount(&server)
        .await;
    let host = Host::new(&Http::new()).unwrap();
    let plugin = host.load(example()).await.unwrap();
    let url = format!("{}/list.json", server.uri());
    let chrome = config(&[("url", &url), ("browser", "Chrome")]);
    let catalog = plugin.catalog(&chrome).await.unwrap();
    let channel = &catalog.channels[0];
    let playback = plugin.stream_url(&chrome, channel).await.unwrap();
    assert_eq!(playback.url.as_str(), "http://s.test/1.m3u8");
    assert_eq!(playback.relay, Some(Relay { browser: Some(Browser::Chrome), filter: false }));
    assert_eq!(plugin.stream_url(&config(&[("url", &url)]), channel).await.unwrap().relay, None);
    assert_eq!(plugin.stream_url(&config(&[("browser", "lynx")]), channel).await.unwrap_err(), IptvError::Setting("browser".into()));
}

#[tokio::test]
async fn filters_round_trip() {
    let host = Host::new(&Http::new()).unwrap();
    let plugin = host.load(example()).await.unwrap();
    let media = Media { content_type: Some("video/mp2t".into()), body: vec![0x47; 1 << 20] };
    let url = url::Url::parse("http://s.test/seg.ts").unwrap();
    assert_eq!(plugin.filter(&Config::new(), &url, media.clone()).await.unwrap(), media);
}

#[tokio::test]
async fn errors_come_back_typed() {
    let server = MockServer::start().await;
    Mock::given(path("/list.json")).respond_with(ResponseTemplate::new(401)).mount(&server).await;
    let host = Host::new(&Http::new()).unwrap();
    let plugin = host.load(example()).await.unwrap();
    let url = format!("{}/list.json", server.uri());
    assert_eq!(plugin.catalog(&config(&[("url", &url)])).await.unwrap_err(), IptvError::AuthFailed);
    assert_eq!(plugin.catalog(&Config::new()).await.unwrap_err(), IptvError::Setting("url".into()));
    let local = plugin.catalog(&config(&[("url", "file:///etc/hosts")])).await.unwrap_err();
    assert!(matches!(local, IptvError::Network(_)), "{local:?}");
}

#[tokio::test]
async fn a_folder_of_plugins() {
    let dir = std::env::temp_dir().join(format!("nightglass-plugins-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::copy(example(), dir.join("a.wasm")).unwrap();
    std::fs::copy(example(), dir.join("b.wasm")).unwrap();
    std::fs::write(dir.join("c.wasm"), b"not wasm").unwrap();
    std::fs::write(dir.join("notes.txt"), b"ignored").unwrap();
    let host = Host::new(&Http::new()).unwrap();
    let mut providers = Providers::builtin(&Http::new());
    let status = host.load_dir(&dir, &mut providers).await;
    std::fs::remove_dir_all(&dir).unwrap();
    assert_eq!(status.iter().map(|s| s.file.as_str()).collect::<Vec<_>>(), ["a.wasm", "b.wasm", "c.wasm"]);
    assert_eq!(status[0].provider.as_deref(), Some("JSON list"));
    assert!(status[1].error.as_deref().unwrap().contains("already exists"));
    assert!(status[2].error.is_some());
    assert!(providers.get("json-list").is_ok());
}

#[tokio::test]
async fn keeps_the_list_across_launches() {
    let server = MockServer::start().await;
    Mock::given(path("/list.json"))
        .and(header("if-none-match", "\"v1\""))
        .respond_with(ResponseTemplate::new(304))
        .with_priority(1)
        .mount(&server)
        .await;
    Mock::given(path("/list.json"))
        .respond_with(ResponseTemplate::new(200).insert_header("etag", "\"v1\"").set_body_string(r#"{"channels": [{"name": "One", "url": "http://s.test/1.ts"}]}"#))
        .expect(1)
        .mount(&server)
        .await;
    let data = std::env::temp_dir().join(format!("nightglass-plugin-data-{}", std::process::id()));
    let url = format!("{}/list.json", server.uri());
    for _ in 0..2 {
        let host = Host::new(&Http::new()).unwrap().with_data_dir(data.clone());
        let plugin = host.load(example()).await.unwrap();
        let catalog = plugin.catalog(&config(&[("url", &url)])).await.unwrap();
        assert_eq!(catalog.channels[0].name, "One");
    }
    std::fs::remove_dir_all(data).unwrap();
}
