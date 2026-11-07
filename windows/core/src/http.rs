use std::time::Duration;

use serde_json::Value;
use url::Url;

use crate::{IptvError, Result};

/// The HTTP client every source shares. Cheap to clone.
#[derive(Clone)]
pub struct Http(reqwest::Client);

impl Default for Http {
    fn default() -> Self {
        Self::new()
    }
}

impl Http {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .user_agent(concat!("Nightglass/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(15))
            .read_timeout(Duration::from_secs(60))
            .build()
            .expect("HTTP client");
        Http(client)
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
        let mut response = self.0.get(url.clone()).send().await.map_err(network)?;
        if !response.status().is_success() {
            return Err(IptvError::Http(response.status().as_u16()));
        }
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            f(&chunk);
        }
        Ok(())
    }

    pub(crate) async fn data(&self, url: Url, headers: &[(&str, &str)]) -> Result<Vec<u8>> {
        let mut request = self.0.get(url);
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
