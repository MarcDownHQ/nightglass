use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use url::Url;

use crate::{IptvError, Result};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
const READ_TIMEOUT: Duration = Duration::from_secs(60);
const REDIRECTS: usize = 10;

/// The HTTP client every source shares. Cheap to clone.
#[derive(Clone)]
pub struct Http {
    client: reqwest::Client,
    browsers: Arc<Mutex<HashMap<Browser, wreq::Client>>>,
}

/// A desktop browser whose TLS and HTTP/2 handshake a request presents, for servers that turn other clients away.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Chrome,
    Edge,
    Firefox,
    Safari,
}

impl Browser {
    fn client(self) -> Result<wreq::Client> {
        use wreq_util::Emulation;
        let emulation = match self {
            Browser::Chrome => Emulation::Chrome149,
            Browser::Edge => Emulation::Edge148,
            Browser::Firefox => Emulation::Firefox151,
            Browser::Safari => Emulation::Safari26,
        };
        wreq::Client::builder()
            .emulation(emulation)
            .redirect(wreq::redirect::Policy::limited(REDIRECTS))
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .map_err(network_as_browser)
    }
}

/// Any request: what `Http::send` and `Http::open` take.
#[derive(Clone, Debug)]
pub struct Request {
    pub method: String,
    pub url: Url,
    pub headers: Vec<(String, String)>,
    pub body: Option<Vec<u8>>,
    /// Sent with this browser's handshake rather than the app's own.
    pub browser: Option<Browser>,
}

impl Request {
    pub fn get(url: Url) -> Self {
        Request { method: "GET".into(), url, headers: Vec::new(), body: None, browser: None }
    }
}

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Http {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Nightglass/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(CONNECT_TIMEOUT)
            .read_timeout(READ_TIMEOUT)
            .build()
            .expect("HTTP client");
        Http { client, browsers: Default::default() }
    }

    /// A playlist or guide: a local file (`file:` URL) or a download.
    pub async fn fetch(&self, url: &Url) -> Result<Vec<u8>> {
        if url.scheme() == "file" {
            let path = url.to_file_path().map_err(|_| IptvError::File(url.to_string()))?;
            return std::fs::read(&path).map_err(|e| IptvError::File(format!("{}: {e}", path.display())));
        }
        self.data(url.clone(), &[]).await
    }

    /// A guide on disk, to be read without holding it in memory: a local file as is, a download
    /// written to a temporary file as it arrives (removed when the returned value is dropped).
    pub async fn to_disk(&self, url: &Url) -> Result<OnDisk> {
        if url.scheme() == "file" {
            let path = url.to_file_path().map_err(|_| IptvError::File(url.to_string()))?;
            return Ok(OnDisk { path, temporary: false });
        }
        use std::io::Write;
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        let n = COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("nightglass-{}-{n}.download", std::process::id()));
        let disk = OnDisk { path, temporary: true };
        let mut file = std::io::BufWriter::new(std::fs::File::create(&disk.path).map_err(|e| IptvError::File(e.to_string()))?);
        let mut failed = None;
        self.each_chunk(url, |chunk| {
            if failed.is_none()
                && let Err(e) = file.write_all(chunk)
            {
                failed = Some(e);
            }
        })
        .await?;
        if let Some(e) = failed {
            return Err(IptvError::File(e.to_string()));
        }
        file.flush().map_err(|e| IptvError::File(e.to_string()))?;
        Ok(disk)
    }

    /// A playlist read piece by piece as it arrives (or from disk), without holding the whole file.
    pub async fn each_chunk(&self, url: &Url, mut f: impl FnMut(&[u8])) -> Result<()> {
        if url.scheme() == "file" {
            use std::io::Read;
            let path = url.to_file_path().map_err(|_| IptvError::File(url.to_string()))?;
            let mut file = std::fs::File::open(&path).map_err(|e| IptvError::File(format!("{}: {e}", path.display())))?;
            let mut buf = vec![0; 1 << 16];
            loop {
                let n = file.read(&mut buf).map_err(|e| IptvError::File(format!("{}: {e}", path.display())))?;
                if n == 0 {
                    return Ok(());
                }
                f(&buf[..n]);
            }
        }
        let mut response = self.client.get(url.clone()).send().await.map_err(network)?;
        if !response.status().is_success() {
            return Err(IptvError::Http(response.status().as_u16()));
        }
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            f(&chunk);
        }
        Ok(())
    }

    pub const SEND_LIMIT: usize = 128 << 20;

    /// Any request, its answer returned whatever the status. http(s) only.
    pub async fn send(&self, request: Request) -> Result<Response> {
        let mut response = self.open(request).await?;
        let mut body = Vec::new();
        while let Some(chunk) = response.chunk().await? {
            if body.len() + chunk.len() > Self::SEND_LIMIT {
                return Err(IptvError::Network(format!("response larger than {} MB", Self::SEND_LIMIT >> 20)));
            }
            body.extend_from_slice(&chunk);
        }
        Ok(Response { status: response.status, headers: response.headers, body })
    }

    /// Any request, its body read as it arrives. http(s) only.
    pub async fn open(&self, request: Request) -> Result<Opened> {
        if !matches!(request.url.scheme(), "http" | "https") {
            return Err(IptvError::Network(format!("{} addresses can't be fetched", request.url.scheme())));
        }
        let invalid = || IptvError::Network(format!("invalid method {}", request.method));
        match request.browser {
            None => {
                let method = reqwest::Method::from_bytes(request.method.as_bytes()).map_err(|_| invalid())?;
                let mut builder = self.client.request(method, request.url);
                for (name, value) in &request.headers {
                    builder = builder.header(name, value);
                }
                if let Some(body) = request.body {
                    builder = builder.body(body);
                }
                let response = builder.send().await.map_err(network)?;
                Ok(Opened { status: response.status().as_u16(), headers: header_list(response.headers(), |v| v.to_str().ok()), body: Body::App(response) })
            }
            Some(browser) => {
                let client = self.browser(browser)?;
                let method = wreq::Method::from_bytes(request.method.as_bytes()).map_err(|_| invalid())?;
                let mut builder = client.request(method, request.url.as_str());
                for (name, value) in &request.headers {
                    builder = builder.header(name, value);
                }
                if let Some(body) = request.body {
                    builder = builder.body(body);
                }
                let response = builder.send().await.map_err(network_as_browser)?;
                Ok(Opened { status: response.status().as_u16(), headers: header_list(response.headers(), |v| v.to_str().ok()), body: Body::Browser(Box::pin(response.bytes_stream())) })
            }
        }
    }

    fn browser(&self, browser: Browser) -> Result<wreq::Client> {
        let mut clients = self.browsers.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(client) = clients.get(&browser) {
            return Ok(client.clone());
        }
        let client = browser.client()?;
        clients.insert(browser, client.clone());
        Ok(client)
    }

    pub(crate) async fn data(&self, url: Url, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
        let mut request = self.client.get(url);
        for (name, value) in headers {
            request = request.header(*name, *value);
        }
        let response = request.send().await.map_err(network)?;
        if !response.status().is_success() {
            return Err(IptvError::Http(response.status().as_u16()));
        }
        Ok(response.bytes().await.map_err(network)?.to_vec())
    }

    pub(crate) async fn json(&self, url: Url, headers: &[(&str, &str)]) -> Result<Value> {
        let data = self.data(url, headers).await?;
        serde_json::from_slice(&data)
            .map_err(|_| IptvError::BadResponse(String::from_utf8_lossy(&data[..data.len().min(120)]).into_owned()))
    }
}

pub struct Response {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// An answer whose body is still on its way.
pub struct Opened {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    body: Body,
}

enum Body {
    App(reqwest::Response),
    Browser(std::pin::Pin<Box<dyn futures_util::Stream<Item = wreq::Result<Bytes>> + Send>>),
}

impl Opened {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str())
    }

    /// The next piece of the body, none at its end.
    pub async fn chunk(&mut self) -> Result<Option<Bytes>> {
        match &mut self.body {
            Body::App(r) => r.chunk().await.map_err(network),
            Body::Browser(r) => futures_util::StreamExt::next(r).await.transpose().map_err(network_as_browser),
        }
    }
}

fn header_list<'a, K: ToString + 'a, V: 'a>(headers: impl IntoIterator<Item = (&'a K, &'a V)>, text: impl Fn(&V) -> Option<&str>) -> Vec<(String, String)> {
    headers.into_iter().filter_map(|(k, v)| Some((k.to_string(), text(v)?.to_string()))).collect()
}

/// A file `Http::to_disk` made or found.
pub struct OnDisk {
    pub path: std::path::PathBuf,
    temporary: bool,
}

impl Drop for OnDisk {
    fn drop(&mut self) {
        if self.temporary {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// The error chain, without the URL: Xtream URLs carry the password.
fn network(e: reqwest::Error) -> IptvError {
    let mut causes = Vec::new();
    let mut next = std::error::Error::source(&e);
    while let Some(cause) = next {
        causes.push(cause.to_string());
        next = cause.source();
    }
    let mut message = e.without_url().to_string();
    for cause in causes {
        message.push_str(": ");
        message.push_str(&cause);
    }
    IptvError::Network(message)
}

fn network_as_browser(e: wreq::Error) -> IptvError {
    let mut causes = Vec::new();
    let mut next = std::error::Error::source(&e);
    while let Some(cause) = next {
        causes.push(cause.to_string());
        next = cause.source();
    }
    let mut message = e.without_uri().to_string();
    for cause in causes {
        message.push_str(": ");
        message.push_str(&cause);
    }
    IptvError::Network(message)
}

// Panels send numbers as strings and strings as numbers; read both.
pub(crate) fn str(v: Option<&Value>) -> Option<String> {
    match v? {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => Some(n.to_string()),
        Value::Bool(b) => Some(if *b { "1" } else { "0" }.to_string()),
        _ => None,
    }
}

pub(crate) fn int(v: Option<&Value>) -> Option<i64> {
    let s = str(v)?;
    s.parse().ok().or_else(|| s.parse::<f64>().ok().map(|d| d as i64))
}

pub(crate) fn double(v: Option<&Value>) -> Option<f64> {
    str(v)?.parse().ok()
}

pub(crate) fn url(v: Option<&Value>) -> Option<Url> {
    Url::parse(str(v)?.trim()).ok()
}

/// The objects of a JSON array; anything else (`{}`, `null`) is an empty list.
pub(crate) fn objects(v: Value) -> Vec<serde_json::Map<String, Value>> {
    match v {
        Value::Array(items) => items
            .into_iter()
            .filter_map(|item| match item {
                Value::Object(o) => Some(o),
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use wiremock::matchers::{body_string, header, method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    #[tokio::test]
    async fn send_returns_any_status() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api"))
            .and(header("x-key", "k"))
            .and(body_string("hi"))
            .respond_with(ResponseTemplate::new(418).insert_header("x-reply", "r").set_body_string("teapot"))
            .mount(&server)
            .await;
        let url = Url::parse(&format!("{}/api", server.uri())).unwrap();
        let r = Http::new()
            .send(Request { method: "POST".into(), headers: vec![("x-key".into(), "k".into())], body: Some(b"hi".to_vec()), ..Request::get(url) })
            .await
            .unwrap();
        assert_eq!(r.status, 418);
        assert_eq!(r.body, b"teapot");
        assert!(r.headers.contains(&("x-reply".into(), "r".into())));
    }

    #[tokio::test]
    async fn browser_requests_present_the_browser() {
        let server = MockServer::start().await;
        Mock::given(method("GET")).and(path("/ua")).respond_with(|r: &wiremock::Request| {
            ResponseTemplate::new(200).set_body_string(r.headers.get("user-agent").and_then(|v| v.to_str().ok()).unwrap_or_default())
        })
        .mount(&server)
        .await;
        let url = Url::parse(&format!("{}/ua", server.uri())).unwrap();
        let http = Http::new();
        let app = http.send(Request::get(url.clone())).await.unwrap();
        assert!(String::from_utf8(app.body).unwrap().starts_with("Nightglass/"));
        let chrome = http.send(Request { browser: Some(Browser::Chrome), ..Request::get(url.clone()) }).await.unwrap();
        assert!(String::from_utf8(chrome.body).unwrap().contains("Chrome/"));
        let own = http.send(Request { browser: Some(Browser::Firefox), headers: vec![("User-Agent".into(), "mine".into())], ..Request::get(url) }).await.unwrap();
        assert_eq!(own.body, b"mine");
    }

    #[tokio::test]
    async fn browser_requests_follow_redirects() {
        let server = MockServer::start().await;
        Mock::given(path("/a")).respond_with(ResponseTemplate::new(302).insert_header("location", "/b")).mount(&server).await;
        Mock::given(path("/b")).respond_with(ResponseTemplate::new(200).set_body_string("b")).mount(&server).await;
        let url = Url::parse(&format!("{}/a", server.uri())).unwrap();
        let r = Http::new().send(Request { browser: Some(Browser::Safari), ..Request::get(url) }).await.unwrap();
        assert_eq!((r.status, r.body.as_slice()), (200, b"b".as_slice()));
    }

    #[tokio::test]
    async fn send_refuses_local_files() {
        let err = Http::new().send(Request::get(Url::parse("file:///etc/passwd").unwrap())).await.err();
        assert!(matches!(err, Some(IptvError::Network(_))));
    }
}
