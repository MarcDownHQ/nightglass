//! Google's Widevine CDM, downloaded from Google's component updater at the user's request and kept, one version at a
//! time, in a folder of the app's data. Never bundled: the package is checked against Google's pinned Widevine key
//! before anything of it is kept.

use std::io::{Cursor, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use ring::digest::{Context, SHA256, digest};
use ring::signature::{RSA_PKCS1_1024_8192_SHA256_FOR_LEGACY_USE_ONLY, UnparsedPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use url::Url;

use crate::{Http, IptvError, Request, Result};

const UPDATE_SERVICE: &str = "https://update.googleapis.com/service/update2/json";
const APP_ID: &str = "oimompecagnajdejgnnjijobebaeigek";
/// SHA-256 of the Widevine component's signing key (its DER SubjectPublicKeyInfo); `APP_ID` is its first half.
const WIDEVINE_KEY: [u8; 32] = [
    0xe8, 0xce, 0xcf, 0x42, 0x06, 0xd0, 0x93, 0x49, 0x6d, 0xd9, 0x89, 0xe1, 0x41, 0x04, 0x86, 0x4a, 0x8f, 0xbd, 0x86, 0x12, 0xb9, 0x58, 0x9b, 0xfb, 0x4f, 0xbb, 0x1b, 0xa9, 0xd3, 0x85, 0x37, 0xef,
];
const CHROME_VERSION: &str = "145.0.0.0";
const MAX_PACKAGE: u64 = 512 << 20;
const MAX_HEADER: usize = 1 << 20;
const LIBRARY_SIZE: std::ops::RangeInclusive<u64> = 1 << 10..=128 << 20;
/// The CDM interface the host speaks.
const INTERFACE: &str = "10";
const RECEIPT: &str = "cdm.json";

/// An installed CDM.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Cdm {
    pub version: String,
    /// The library to load: `libwidevinecdm.so`, `widevinecdm.dll` or `libwidevinecdm.dylib`, with its `.sig` beside it
    /// on Windows and macOS.
    pub library: PathBuf,
    /// Of the library, in hex.
    pub sha256: String,
}

impl Cdm {
    /// Whether the library on disk is still the one that was installed.
    pub fn verify(&self) -> Result<()> {
        let bytes = std::fs::read(&self.library).map_err(|e| IptvError::File(format!("{}: {e}", self.library.display())))?;
        if hex(digest(&SHA256, &bytes).as_ref()) != self.sha256 {
            return Err(IptvError::Drm("the Widevine CDM changed on disk: turn protected content off and on again".into()));
        }
        Ok(())
    }
}

/// How far an install has got.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", tag = "stage")]
pub enum Progress {
    /// Asking Google which version to get.
    Checking,
    Downloading { received: u64, total: u64 },
    Verifying,
}

/// The receipt as kept: the library relative to the folder, so that the folder can move.
#[derive(Serialize, Deserialize)]
struct Receipt {
    version: String,
    library: PathBuf,
    sha256: String,
}

/// The CDM installed in `dir`, if any.
pub fn installed(dir: &Path) -> Option<Cdm> {
    let receipt: Receipt = serde_json::from_slice(&std::fs::read(dir.join(RECEIPT)).ok()?).ok()?;
    let library = dir.join(&receipt.library);
    library.is_file().then_some(Cdm { version: receipt.version, library, sha256: receipt.sha256 })
}

/// Forgets the CDM: `dir` and all it holds.
pub fn remove(dir: &Path) -> Result<()> {
    match std::fs::remove_dir_all(dir) {
        Err(e) if e.kind() != std::io::ErrorKind::NotFound => Err(IptvError::File(format!("{}: {e}", dir.display()))),
        _ => Ok(()),
    }
}

/// What Google offers for this computer.
struct Offer {
    version: String,
    url: Url,
    size: u64,
    sha256: String,
}

/// Downloads the current CDM for this computer into `dir`, checks it, and keeps it in place of any other version.
/// `cancel` stops the download between two pieces.
pub async fn install(http: &Http, dir: &Path, cancel: &AtomicBool, progress: impl Fn(Progress)) -> Result<Cdm> {
    let (os, arch) = platform().ok_or_else(|| IptvError::Drm(format!("Google offers no Widevine CDM for {} {}", std::env::consts::OS, std::env::consts::ARCH)))?;
    progress(Progress::Checking);
    let offer = offer(http, os, arch).await?;
    std::fs::create_dir_all(dir).map_err(|e| IptvError::File(format!("{}: {e}", dir.display())))?;
    let crx = download(http, &offer, &dir.join(".download.crx3"), cancel, &progress).await;
    let crx = crx.inspect_err(|_| _ = std::fs::remove_file(dir.join(".download.crx3")))?;
    progress(Progress::Verifying);
    let dir = dir.to_path_buf();
    let done = tokio::task::spawn_blocking(move || {
        let kept = unpack(&dir, &offer.version, &crx, &WIDEVINE_KEY);
        _ = std::fs::remove_file(dir.join(".download.crx3"));
        kept
    });
    done.await.map_err(|e| IptvError::Drm(e.to_string()))?
}

/// Google's names for this computer's system and processor.
fn platform() -> Option<(&'static str, &'static str)> {
    let os = match std::env::consts::OS {
        "linux" => "linux",
        "windows" => "win",
        "macos" => "mac",
        _ => return None,
    };
    let arch = match std::env::consts::ARCH {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "x86",
        _ => return None,
    };
    Some((os, arch))
}

fn library_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "widevinecdm.dll",
        "macos" => "libwidevinecdm.dylib",
        _ => "libwidevinecdm.so",
    }
}

async fn offer(http: &Http, os: &str, arch: &str) -> Result<Offer> {
    let body = json!({"request": {
        "protocol": "3.1",
        "acceptformat": "crx3",
        "@updater": "chrome",
        "@os": os,
        "arch": arch,
        "prodversion": CHROME_VERSION,
        "updaterversion": CHROME_VERSION,
        "prodchannel": "stable",
        "os": {"platform": os, "arch": arch},
        "app": [{"appid": APP_ID, "version": "0.0.0.0", "enabled": true, "updatecheck": {}}],
    }});
    let request = Request {
        method: "POST".into(),
        url: Url::parse(UPDATE_SERVICE).expect("a valid address"),
        headers: vec![("content-type".into(), "application/json".into())],
        body: Some(body.to_string().into_bytes()),
        browser: None,
    };
    let response = http.send(request).await?;
    if !(200..300).contains(&response.status) {
        return Err(IptvError::Http(response.status));
    }
    parse_offer(&response.body)
}

fn parse_offer(body: &[u8]) -> Result<Offer> {
    let bad = |what: &str| IptvError::BadResponse(format!("Google's update service: {what}"));
    let text = std::str::from_utf8(body).map_err(|_| bad("not text"))?;
    let json = if text.starts_with(")]}'") { text.split_once('\n').map_or("", |(_, rest)| rest) } else { text };
    let answer: Value = serde_json::from_str(json).map_err(|_| bad("not JSON"))?;
    let check = &answer["response"]["app"][0]["updatecheck"];
    if check["status"] != "ok" {
        return Err(bad(&format!("no package ({})", check["status"].as_str().unwrap_or("no status"))));
    }
    let version = check["manifest"]["version"].as_str().filter(|v| is_version(v)).ok_or_else(|| bad("no version"))?;
    let package = &check["manifest"]["packages"]["package"][0];
    let name = package["name"].as_str().filter(|n| !n.is_empty() && !n.contains(['/', '\\', '?', '#'])).ok_or_else(|| bad("no package name"))?;
    let size = package["size"].as_u64().filter(|s| *s <= MAX_PACKAGE).ok_or_else(|| bad("no package size"))?;
    let sha256 = package["hash_sha256"].as_str().filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit())).ok_or_else(|| bad("no package hash"))?;
    let bases: Vec<Url> = check["urls"]["url"].as_array().into_iter().flatten().filter_map(|u| Url::parse(u["codebase"].as_str()?).ok()).filter(|u| u.scheme() == "https").collect();
    let base = bases.iter().find(|u| u.host_str() == Some("dl.google.com")).or(bases.first()).ok_or_else(|| bad("no https address"))?;
    let url = base.join(name).map_err(|_| bad("no download address"))?;
    Ok(Offer { version: version.into(), url, size, sha256: sha256.to_ascii_lowercase() })
}

/// Digits and dots: a version names a folder.
fn is_version(v: &str) -> bool {
    !v.is_empty() && v.len() <= 32 && v.bytes().all(|b| b.is_ascii_digit() || b == b'.') && !v.starts_with('.')
}

async fn download(http: &Http, offer: &Offer, path: &Path, cancel: &AtomicBool, progress: &impl Fn(Progress)) -> Result<Vec<u8>> {
    let mut response = http.open(Request::get(offer.url.clone())).await?;
    if response.url.scheme() != "https" {
        return Err(IptvError::Drm("the download left https".into()));
    }
    if !(200..300).contains(&response.status) {
        return Err(IptvError::Http(response.status));
    }
    if response.header("content-length").and_then(|l| l.parse::<u64>().ok()).is_some_and(|l| l != offer.size) {
        return Err(IptvError::Drm("the package is not the size Google announced".into()));
    }
    let mut file = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| IptvError::File(format!("{}: {e}", path.display())))?);
    let mut hash = Context::new(&SHA256);
    let mut received = 0u64;
    progress(Progress::Downloading { received, total: offer.size });
    while let Some(chunk) = response.chunk().await? {
        if cancel.load(Ordering::Relaxed) {
            return Err(IptvError::Drm("download cancelled".into()));
        }
        received += chunk.len() as u64;
        if received > offer.size {
            return Err(IptvError::Drm("the package is larger than Google announced".into()));
        }
        hash.update(&chunk);
        file.write_all(&chunk).map_err(|e| IptvError::File(e.to_string()))?;
        progress(Progress::Downloading { received, total: offer.size });
    }
    file.flush().map_err(|e| IptvError::File(e.to_string()))?;
    drop(file);
    if received != offer.size || hex(hash.finish().as_ref()) != offer.sha256 {
        return Err(IptvError::Drm("the package is not the one Google announced".into()));
    }
    let mut bytes = Vec::with_capacity(offer.size as usize);
    std::fs::File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| IptvError::File(e.to_string()))?;
    Ok(bytes)
}

/// Checks the package against `key` (the SHA-256 of its signing key), then keeps its library, the library's signature,
/// its manifest and Google's licence in `dir/<version>`, and drops every other version.
fn unpack(dir: &Path, version: &str, crx: &[u8], key: &[u8; 32]) -> Result<Cdm> {
    let zip = verify_crx3(crx, key)?;
    let mut archive = zip::ZipArchive::new(Cursor::new(zip)).map_err(|e| IptvError::Drm(format!("unreadable package: {e}")))?;
    let manifest: Value = serde_json::from_slice(&entry(&mut archive, "manifest.json", 1 << 20)?).map_err(|_| IptvError::Drm("unreadable package manifest".into()))?;
    if manifest["version"] != version {
        return Err(IptvError::Drm("the package is not the version Google announced".into()));
    }
    let interfaces = manifest["x-cdm-interface-versions"].as_str().unwrap_or_default();
    if !interfaces.split(',').any(|v| v.trim() == INTERFACE) {
        return Err(IptvError::Drm(format!("this CDM speaks interface {interfaces}, not {INTERFACE}")));
    }
    let name = library_name();
    let names: Vec<String> = archive.file_names().filter_map(|n| Some(n.ok()?.into_owned())).collect();
    let library_path = names.iter().find(|n| n.starts_with("_platform_specific/") && n.rsplit('/').next() == Some(name));
    let library_path = library_path.ok_or_else(|| IptvError::Drm(format!("no {name} in the package")))?;
    let library = entry(&mut archive, library_path, *LIBRARY_SIZE.end())?;
    if !LIBRARY_SIZE.contains(&(library.len() as u64)) {
        return Err(IptvError::Drm(format!("{name} is not the size of a CDM")));
    }
    let signature_path = format!("{library_path}.sig");
    let signature = names.contains(&signature_path).then(|| entry(&mut archive, &signature_path, 1 << 20)).transpose()?;
    let licence = entry(&mut archive, "LICENSE", 1 << 20).ok();

    let file = |e: std::io::Error| IptvError::File(e.to_string());
    let partial = dir.join(format!(".{version}.partial"));
    _ = std::fs::remove_dir_all(&partial);
    std::fs::create_dir_all(&partial).map_err(file)?;
    std::fs::write(partial.join(name), &library).map_err(file)?;
    if let Some(signature) = signature {
        std::fs::write(partial.join(format!("{name}.sig")), signature).map_err(file)?;
    }
    std::fs::write(partial.join("manifest.json"), serde_json::to_vec_pretty(&manifest).unwrap_or_default()).map_err(file)?;
    if let Some(licence) = licence {
        std::fs::write(partial.join("LICENSE"), licence).map_err(file)?;
    }
    let target = dir.join(version);
    _ = std::fs::remove_dir_all(&target);
    std::fs::rename(&partial, &target).map_err(file)?;

    let receipt = Receipt { version: version.into(), library: Path::new(version).join(name), sha256: hex(digest(&SHA256, &library).as_ref()) };
    let temporary = dir.join(format!("{RECEIPT}.partial"));
    std::fs::write(&temporary, serde_json::to_vec_pretty(&receipt).unwrap_or_default()).map_err(file)?;
    std::fs::rename(&temporary, dir.join(RECEIPT)).map_err(file)?;

    for old in std::fs::read_dir(dir).map_err(file)?.filter_map(|e| e.ok()) {
        if old.file_name() != *version && old.path().is_dir() {
            _ = std::fs::remove_dir_all(old.path());
        }
    }
    installed(dir).ok_or_else(|| IptvError::Drm("the CDM was not kept".into()))
}

fn entry(archive: &mut zip::ZipArchive<Cursor<&[u8]>>, name: &str, limit: u64) -> Result<Vec<u8>> {
    let file = archive.by_name(name).map_err(|_| IptvError::Drm(format!("no {name} in the package")))?;
    let mut bytes = Vec::new();
    file.take(limit + 1).read_to_end(&mut bytes).map_err(|e| IptvError::Drm(format!("{name}: {e}")))?;
    if bytes.len() as u64 > limit {
        return Err(IptvError::Drm(format!("{name} is too large")));
    }
    Ok(bytes)
}

/// The ZIP archive of a CRX3 package, once its header proves it was signed with `key` (the SHA-256 of the signing
/// key's SubjectPublicKeyInfo) and names the component that key stands for.
fn verify_crx3<'a>(crx: &'a [u8], key: &[u8; 32]) -> Result<&'a [u8]> {
    let bad = |what: &str| IptvError::Drm(format!("the package is not a signed Widevine CDM: {what}"));
    if crx.len() < 12 || &crx[..4] != b"Cr24" || u32::from_le_bytes(crx[4..8].try_into().unwrap()) != 3 {
        return Err(bad("not a CRX3 package"));
    }
    let header_len = u32::from_le_bytes(crx[8..12].try_into().unwrap()) as usize;
    if header_len > MAX_HEADER || crx.len() < 12 + header_len {
        return Err(bad("header out of bounds"));
    }
    let (header, zip) = crx[12..].split_at(header_len);
    let mut rsa_proofs = Vec::new();
    let mut signed_data: Option<&[u8]> = None;
    for (field, value) in protobuf(header).ok_or_else(|| bad("unreadable header"))? {
        match field {
            2 => rsa_proofs.push(value),
            10000 => signed_data = Some(value),
            _ => {}
        }
    }
    let signed_data = signed_data.ok_or_else(|| bad("no signed data"))?;
    let crx_id = protobuf(signed_data).ok_or_else(|| bad("unreadable signed data"))?.into_iter().find(|(f, _)| *f == 1).map(|(_, v)| v);
    if crx_id != Some(&key[..16]) {
        return Err(bad("not the Widevine component"));
    }
    let mut message = Vec::with_capacity(20 + signed_data.len() + zip.len());
    message.extend_from_slice(b"CRX3 SignedData\0");
    message.extend_from_slice(&(signed_data.len() as u32).to_le_bytes());
    message.extend_from_slice(signed_data);
    message.extend_from_slice(zip);
    for proof in rsa_proofs {
        let fields = protobuf(proof).ok_or_else(|| bad("unreadable proof"))?;
        let public_key = fields.iter().find(|(f, _)| *f == 1).map(|(_, v)| *v);
        let signature = fields.iter().find(|(f, _)| *f == 2).map(|(_, v)| *v);
        let (Some(public_key), Some(signature)) = (public_key, signature) else { continue };
        if digest(&SHA256, public_key).as_ref() != key {
            continue;
        }
        let rsa_key = spki_key(public_key).ok_or_else(|| bad("unreadable key"))?;
        return UnparsedPublicKey::new(&RSA_PKCS1_1024_8192_SHA256_FOR_LEGACY_USE_ONLY, rsa_key).verify(&message, signature).map(|()| zip).map_err(|_| bad("wrong signature"));
    }
    Err(bad("not signed with the Widevine key"))
}

/// The length-delimited fields of a protobuf message, with their numbers; none when it doesn't parse.
fn protobuf(mut bytes: &[u8]) -> Option<Vec<(u64, &[u8])>> {
    fn varint(bytes: &mut &[u8]) -> Option<u64> {
        let mut value = 0u64;
        for shift in (0..64).step_by(7) {
            let (&byte, rest) = bytes.split_first()?;
            *bytes = rest;
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
        }
        None
    }
    let mut fields = Vec::new();
    while !bytes.is_empty() {
        let key = varint(&mut bytes)?;
        match key & 7 {
            0 => _ = varint(&mut bytes)?,
            1 => bytes = bytes.get(8..)?,
            5 => bytes = bytes.get(4..)?,
            2 => {
                let len = usize::try_from(varint(&mut bytes)?).ok()?;
                fields.push((key >> 3, bytes.get(..len)?));
                bytes = &bytes[len..];
            }
            _ => return None,
        }
    }
    Some(fields)
}

/// The key inside a DER SubjectPublicKeyInfo: the contents of its BIT STRING.
fn spki_key(spki: &[u8]) -> Option<&[u8]> {
    fn element(bytes: &[u8]) -> Option<(u8, &[u8], &[u8])> {
        let (&tag, rest) = bytes.split_first()?;
        let (&first, rest) = rest.split_first()?;
        let (len, rest) = if first & 0x80 == 0 {
            (usize::from(first), rest)
        } else {
            let n = usize::from(first & 0x7f);
            if n == 0 || n > 4 {
                return None;
            }
            (rest.get(..n)?.iter().fold(0usize, |l, b| l << 8 | usize::from(*b)), &rest[n..])
        };
        Some((tag, rest.get(..len)?, &rest[len..]))
    }
    let (0x30, info, _) = element(spki)? else { return None };
    let (0x30, _, rest) = element(info)? else { return None };
    let (0x03, bits, _) = element(rest)? else { return None };
    let (&0, key) = bits.split_first()? else { return None };
    Some(key)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
