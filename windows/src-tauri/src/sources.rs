//! Sources hold provider passwords: the whole list lives in one file encrypted for the current user
//! (DPAPI on Windows; on macOS with a key kept in the login keychain), never in plain text.

use std::path::{Path, PathBuf};

use nightglass_core::Source;
use tauri::{AppHandle, Manager};
#[cfg(windows)]
use windows_sys::Win32::Foundation::LocalFree;
#[cfg(windows)]
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

/// UI tests pass their own account so they start empty and never touch the user's sources.
pub fn file(app: &AppHandle) -> Result<PathBuf, String> {
    let account = std::env::var("NG_KEYCHAIN_ACCOUNT").unwrap_or_else(|_| "sources".into());
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    #[cfg(target_os = "android")]
    sandbox::set_dir(&dir);
    Ok(dir.join(format!("{account}.bin")))
}

pub fn load(path: &Path) -> Vec<Source> {
    let Ok(sealed) = std::fs::read(path) else { return Vec::new() };
    match protect(&sealed, false).map(|json| serde_json::from_slice(&json)) {
        Ok(Ok(sources)) => sources,
        Ok(Err(e)) => {
            log::error!("{}: {e}", path.display());
            Vec::new()
        }
        Err(e) => {
            log::error!("{}: {e}", path.display());
            Vec::new()
        }
    }
}

pub fn save(path: &Path, sources: &[Source]) -> Result<(), String> {
    let json = serde_json::to_vec(sources).map_err(|e| e.to_string())?;
    let sealed = protect(&json, true)?;
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    // Write then rename, so a crash never leaves half a file.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, sealed).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Writes the sources and the page's settings to a JSON file the user picked, for another PC.
/// The file holds the passwords in clear: the UI says so before it asks where to save.
#[tauri::command]
pub fn settings_export(app: AppHandle, path: PathBuf, prefs: serde_json::Value) -> Result<(), String> {
    let sources = load(&file(&app)?);
    let doc = serde_json::json!({ "app": "Nightglass", "version": 1, "sources": sources, "prefs": prefs });
    let json = serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| format!("{}: {e}", path.display()))
}

/// Reads a file made by `settings_export`: its sources (checked) and settings.
#[tauri::command]
pub fn settings_import(path: PathBuf) -> Result<(Vec<Source>, serde_json::Value), String> {
    let data = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut doc: serde_json::Value = serde_json::from_slice(&data).map_err(|_| "Not a Nightglass settings file.".to_string())?;
    if doc.get("app").and_then(|a| a.as_str()) != Some("Nightglass") {
        return Err("Not a Nightglass settings file.".into());
    }
    let sources = serde_json::from_value(doc["sources"].take()).map_err(|e| format!("Sources: {e}"))?;
    Ok((sources, doc["prefs"].take()))
}

/// Encrypts (or decrypts) `data` so that only the current user can read it.
#[cfg(windows)]
pub fn protect(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
    let input = CRYPT_INTEGER_BLOB { cbData: data.len() as u32, pbData: data.as_ptr() as *mut u8 };
    let mut output = CRYPT_INTEGER_BLOB { cbData: 0, pbData: std::ptr::null_mut() };
    let null = std::ptr::null();
    let ok = unsafe {
        if encrypt {
            CryptProtectData(&input, null as _, null as _, null, null as _, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        } else {
            CryptUnprotectData(&input, std::ptr::null_mut(), null as _, null, null as _, CRYPTPROTECT_UI_FORBIDDEN, &mut output)
        }
    };
    if ok == 0 {
        return Err(std::io::Error::last_os_error().to_string());
    }
    let out = unsafe { std::slice::from_raw_parts(output.pbData, output.cbData as usize) }.to_vec();
    unsafe { LocalFree(output.pbData as _) };
    Ok(out)
}

#[cfg(target_os = "macos")]
pub use keychain::protect;

#[cfg(target_os = "android")]
pub use sandbox::protect;

#[cfg(target_os = "macos")]
mod keychain {
    use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
    use ring::rand::{SecureRandom, SystemRandom};
    #[cfg(not(debug_assertions))]
    use security_framework::passwords::{get_generic_password, set_generic_password};

    const SERVICE: &str = "com.marcdownhq.nightglass";
    #[cfg(not(debug_assertions))]
    const ACCOUNT: &str = "data-key";
    #[cfg(not(debug_assertions))]
    const NOT_FOUND: i32 = -25300; // errSecItemNotFound

    /// Read once per launch: each keychain read may ask the user.
    static KEY: std::sync::OnceLock<Vec<u8>> = std::sync::OnceLock::new();

    fn key() -> Result<LessSafeKey, String> {
        let bytes = match KEY.get() {
            Some(bytes) => bytes,
            None => {
                let bytes = stored_key()?;
                KEY.get_or_init(|| bytes)
            }
        };
        let key = UnboundKey::new(&CHACHA20_POLY1305, bytes).map_err(|_| "Bad data key".to_string())?;
        Ok(LessSafeKey::new(key))
    }

    fn new_key() -> Result<Vec<u8>, String> {
        let mut bytes = vec![0; 32];
        SystemRandom::new().fill(&mut bytes).map_err(|_| "No randomness".to_string())?;
        Ok(bytes)
    }

    /// The user's data key, made on first use. Any other keychain error (access denied…) fails rather than replace it.
    #[cfg(not(debug_assertions))]
    fn stored_key() -> Result<Vec<u8>, String> {
        match get_generic_password(SERVICE, ACCOUNT) {
            Ok(bytes) => Ok(bytes),
            Err(e) if e.code() == NOT_FOUND => {
                let bytes = new_key()?;
                set_generic_password(SERVICE, ACCOUNT, &bytes).map_err(|e| e.to_string())?;
                Ok(bytes)
            }
            Err(e) => Err(format!("Keychain: {e}")),
        }
    }

    /// Dev builds: every rebuild is a new unsigned binary to the keychain, which would ask for the password each
    /// time; the key lives instead in a file only the user can read, apart from a release build's data.
    #[cfg(debug_assertions)]
    fn stored_key() -> Result<Vec<u8>, String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let home = std::env::var("HOME").map_err(|e| e.to_string())?;
        let path = std::path::Path::new(&home).join("Library/Application Support").join(SERVICE).join("dev-data-key");
        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }
        let bytes = new_key()?;
        std::fs::create_dir_all(path.parent().expect("key dir")).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    /// Encrypts (or decrypts) `data` so that only the current user can read it: a random nonce, then the sealed data.
    pub fn protect(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
        let key = key()?;
        if encrypt {
            let mut nonce = [0; NONCE_LEN];
            SystemRandom::new().fill(&mut nonce).map_err(|_| "No randomness".to_string())?;
            let mut out = data.to_vec();
            key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut out).map_err(|_| "Encryption failed".to_string())?;
            return Ok([&nonce[..], &out].concat());
        }
        if data.len() < NONCE_LEN {
            return Err("Truncated data".into());
        }
        let (nonce, sealed) = data.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| "Bad nonce".to_string())?;
        let mut buf = sealed.to_vec();
        let plain = key.open_in_place(nonce, Aad::empty(), &mut buf).map_err(|_| "Decryption failed".to_string())?;
        Ok(plain.to_vec())
    }
}

/// Android: no keychain a plain app can use without a UI; the data key sits in the app's private storage (readable
/// by this app only), and the file it seals is useless without it, e.g. in a backup of the sources file alone.
#[cfg(target_os = "android")]
mod sandbox {
    use std::path::{Path, PathBuf};
    use std::sync::{Mutex, OnceLock};

    use ring::aead::{Aad, CHACHA20_POLY1305, LessSafeKey, NONCE_LEN, Nonce, UnboundKey};
    use ring::rand::{SecureRandom, SystemRandom};

    static DIR: Mutex<Option<PathBuf>> = Mutex::new(None);
    static KEY: OnceLock<Vec<u8>> = OnceLock::new();

    pub fn set_dir(dir: &Path) {
        *DIR.lock().expect("dir lock") = Some(dir.to_path_buf());
    }

    fn stored_key() -> Result<Vec<u8>, String> {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;

        let dir = DIR.lock().expect("dir lock").clone().ok_or("No data folder")?;
        let path = dir.join("data-key");
        if let Ok(bytes) = std::fs::read(&path) {
            return Ok(bytes);
        }
        let mut bytes = vec![0; 32];
        SystemRandom::new().fill(&mut bytes).map_err(|_| "No randomness".to_string())?;
        std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
        let mut file = std::fs::OpenOptions::new().write(true).create_new(true).mode(0o600).open(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        file.write_all(&bytes).map_err(|e| e.to_string())?;
        Ok(bytes)
    }

    fn key() -> Result<LessSafeKey, String> {
        let bytes = match KEY.get() {
            Some(bytes) => bytes,
            None => {
                let bytes = stored_key()?;
                KEY.get_or_init(|| bytes)
            }
        };
        let key = UnboundKey::new(&CHACHA20_POLY1305, bytes).map_err(|_| "Bad data key".to_string())?;
        Ok(LessSafeKey::new(key))
    }

    /// Encrypts (or decrypts) `data`: a random nonce, then the sealed data.
    pub fn protect(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
        let key = key()?;
        if encrypt {
            let mut nonce = [0; NONCE_LEN];
            SystemRandom::new().fill(&mut nonce).map_err(|_| "No randomness".to_string())?;
            let mut out = data.to_vec();
            key.seal_in_place_append_tag(Nonce::assume_unique_for_key(nonce), Aad::empty(), &mut out).map_err(|_| "Encryption failed".to_string())?;
            return Ok([&nonce[..], &out].concat());
        }
        if data.len() < NONCE_LEN {
            return Err("Truncated data".into());
        }
        let (nonce, sealed) = data.split_at(NONCE_LEN);
        let nonce = Nonce::try_assume_unique_for_key(nonce).map_err(|_| "Bad nonce".to_string())?;
        let mut buf = sealed.to_vec();
        let plain = key.open_in_place(nonce, Aad::empty(), &mut buf).map_err(|_| "Decryption failed".to_string())?;
        Ok(plain.to_vec())
    }
}

#[cfg(test)]
mod tests {
    use nightglass_core::SourceKind;

    use super::*;

    #[test]
    fn round_trip_is_encrypted() {
        let path = std::env::temp_dir().join(format!("nightglass-test-{}.bin", std::process::id()));
        let sources = vec![Source::new(
            "Panel",
            SourceKind::Xtream { server: "http://panel.test".parse().unwrap(), username: "u".into(), password: "s3cret".into() },
        )];
        save(&path, &sources).unwrap();
        let raw = std::fs::read(&path).unwrap();
        assert!(!raw.windows(6).any(|w| w == b"s3cret"));
        assert_eq!(load(&path), sources);
        std::fs::remove_file(path).unwrap();
    }
}
