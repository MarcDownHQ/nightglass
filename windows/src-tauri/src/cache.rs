//! The last catalog and guide each source returned, so the screens fill at launch and the network
//! only refreshes them. Encrypted like the sources: Xtream stream URLs carry the password.

use std::path::PathBuf;

use serde::Serialize;
use serde::de::DeserializeOwned;
use tauri::{AppHandle, Manager};

use crate::sources::protect;

fn path(app: &AppHandle, kind: &str, id: &str) -> Result<PathBuf, String> {
    // Source ids are UUIDs; anything else must not become a path.
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
        return Err(format!("Invalid cache id: {id}"));
    }
    let dir = app.path().app_cache_dir().map_err(|e| e.to_string())?;
    Ok(dir.join(format!("{kind}-{id}.bin")))
}

pub fn read<T: DeserializeOwned>(app: &AppHandle, kind: &str, id: &str) -> Option<T> {
    let sealed = std::fs::read(path(app, kind, id).ok()?).ok()?;
    let json = protect(&sealed, false).ok()?;
    serde_json::from_slice(&json).ok()
}

pub fn write<T: Serialize>(app: &AppHandle, kind: &str, id: &str, value: &T) -> Result<(), String> {
    let path = path(app, kind, id)?;
    let sealed = protect(&serde_json::to_vec(value).map_err(|e| e.to_string())?, true)?;
    std::fs::create_dir_all(path.parent().expect("cache dir")).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, sealed).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, &path).map_err(|e| e.to_string())
}

pub fn drop(app: &AppHandle, id: &str) {
    for kind in ["catalog", "guide"] {
        if let Ok(p) = path(app, kind, id) {
            let _ = std::fs::remove_file(p);
        }
    }
}
