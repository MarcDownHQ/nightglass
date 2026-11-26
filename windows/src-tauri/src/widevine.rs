//! The "protected content" setting: on while Google's Widevine CDM is installed in the app's data. Only the page's
//! setting installs it, never a plugin.

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use nightglass_core::widevine::{self, Cdm, Progress};
use nightglass_core::{Config, Decryptor, Http, Playback, Provider, Result as CoreResult};
use tauri::async_runtime::Mutex;
use tauri::{AppHandle, Emitter, State};

pub struct Widevine {
    dir: PathBuf,
    cancel: AtomicBool,
    /// Held by an install, and by a removal so that it waits for the install it cancels.
    busy: Mutex<()>,
    decryptor: Arc<dyn Decryptor>,
}

impl Widevine {
    pub fn new(dir: PathBuf, decryptor: Arc<dyn Decryptor>) -> Self {
        Widevine { dir, cancel: AtomicBool::new(false), busy: Mutex::new(()), decryptor }
    }

    /// The CDM, when protected content is on.
    pub fn cdm(&self) -> Option<Cdm> {
        widevine::installed(&self.dir)
    }

    /// `playback`, whose `drm` is set, handed to the CDM host.
    pub async fn open(&self, cdm: &Cdm, provider: Arc<dyn Provider>, config: Config, playback: Playback) -> CoreResult<Playback> {
        let checked = cdm.clone();
        tauri::async_runtime::spawn_blocking(move || checked.verify()).await.map_err(|e| nightglass_core::IptvError::Drm(e.to_string()))??;
        self.decryptor.open(cdm, provider, config, playback).await
    }
}

/// The installed CDM's version; none while protected content is off.
#[tauri::command]
pub fn widevine_status(widevine: State<'_, Widevine>) -> Option<String> {
    widevine.cdm().map(|c| c.version)
}

/// Downloads the CDM from Google, reporting `widevine-progress` events on the way; its version once installed.
#[tauri::command]
pub async fn widevine_install(app: AppHandle, http: State<'_, Http>, widevine: State<'_, Widevine>) -> Result<String, String> {
    let _busy = widevine.busy.lock().await;
    widevine.cancel.store(false, Ordering::Relaxed);
    let progress = |p: Progress| {
        let _ = app.emit("widevine-progress", p);
    };
    let cdm = widevine::install(&http, &widevine.dir, &widevine.cancel, progress).await.map_err(|e| e.to_string())?;
    Ok(cdm.version)
}

/// Turns protected content off: a download under way stops, and the CDM leaves the disk.
#[tauri::command]
pub async fn widevine_remove(widevine: State<'_, Widevine>) -> Result<(), String> {
    widevine.cancel.store(true, Ordering::Relaxed);
    let _busy = widevine.busy.lock().await;
    let dir = widevine.dir.clone();
    tauri::async_runtime::spawn_blocking(move || widevine::remove(&dir)).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())
}
