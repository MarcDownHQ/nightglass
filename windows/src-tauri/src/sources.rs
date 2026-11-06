//! Sources hold provider passwords: the whole list lives in one file encrypted with DPAPI for the
//! current Windows user, never in plain text.

use std::path::{Path, PathBuf};

use nightglass_core::Source;
use tauri::{AppHandle, Manager};
use windows_sys::Win32::Foundation::LocalFree;
use windows_sys::Win32::Security::Cryptography::{
    CRYPT_INTEGER_BLOB, CRYPTPROTECT_UI_FORBIDDEN, CryptProtectData, CryptUnprotectData,
};

/// UI tests pass their own account so they start empty and never touch the user's sources.
pub fn file(app: &AppHandle) -> Result<PathBuf, String> {
    let account = std::env::var("NG_KEYCHAIN_ACCOUNT").unwrap_or_else(|_| "sources".into());
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{account}.bin")))
}

pub fn load(path: &Path) -> Vec<Source> {
    let Ok(sealed) = std::fs::read(path) else { return Vec::new() };
    match dpapi(&sealed, false).map(|json| serde_json::from_slice(&json)) {
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
    let sealed = dpapi(&json, true)?;
    std::fs::create_dir_all(path.parent().unwrap_or(Path::new("."))).map_err(|e| e.to_string())?;
    // Write then rename, so a crash never leaves half a file.
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, sealed).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

fn dpapi(data: &[u8], encrypt: bool) -> Result<Vec<u8>, String> {
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
