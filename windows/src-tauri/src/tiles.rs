//! Multi-view: up to four extra mpv instances, each drawing into its own child window placed where
//! the page leaves a hole for it. The main player's window is hidden meanwhile.

use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Manager, State};

use crate::mpv::{self, Mpv};
use crate::{MainWindow, VideoHost};

pub const MAX: usize = 4;

struct Tile {
    mpv: Arc<Mpv>,
    host: isize,
}

#[derive(Default)]
pub struct Tiles(Mutex<HashMap<usize, Tile>>);

/// Where libmpv-2.dll is, for the tiles' instances.
pub struct MpvDll(pub PathBuf);

/// Plays `url` in tile `index`, made on first use, at `rect` (x, y, width, height in physical pixels).
#[tauri::command]
pub fn tile_play(
    app: AppHandle,
    tiles: State<'_, Tiles>,
    index: usize,
    url: String,
    headers: HashMap<String, String>,
    rect: [i32; 4],
    muted: bool,
) -> Result<(), String> {
    if index >= MAX {
        return Err(format!("Tile {index} out of range"));
    }
    let mut tiles = tiles.0.lock().map_err(|e| e.to_string())?;
    let tile = match tiles.entry(index) {
        Entry::Occupied(e) => e.into_mut(),
        Entry::Vacant(slot) => {
            let host = mpv::tile_host(app.state::<MainWindow>().0, rect);
            let instance = Arc::new(Mpv::new(&app.state::<MpvDll>().0, host)?);
            // Four streams at once: a small back buffer each.
            instance.set("demuxer-max-back-bytes", "16MiB")?;
            let (pump, handle) = (instance.clone(), app.clone());
            std::thread::spawn(move || pump.pump(&handle, "tile", Some(index)));
            slot.insert(Tile { mpv: instance, host })
        }
    };
    mpv::move_host(tile.host, rect);
    tile.mpv.set("user-agent", headers.get("User-Agent").map_or("libmpv", String::as_str))?;
    tile.mpv.set("referrer", headers.get("Referer").map_or("", String::as_str))?;
    tile.mpv.set("mute", if muted { "yes" } else { "no" })?;
    tile.mpv.command(&["loadfile", &url, "replace"])
}

#[tauri::command]
pub fn tile_rect(tiles: State<'_, Tiles>, index: usize, rect: [i32; 4]) -> Result<(), String> {
    if let Some(tile) = tiles.0.lock().map_err(|e| e.to_string())?.get(&index) {
        mpv::move_host(tile.host, rect);
    }
    Ok(())
}

/// Only one tile is heard at a time.
#[tauri::command]
pub fn tile_mute(tiles: State<'_, Tiles>, index: usize, muted: bool) -> Result<(), String> {
    if let Some(tile) = tiles.0.lock().map_err(|e| e.to_string())?.get(&index) {
        tile.mpv.set("mute", if muted { "yes" } else { "no" })?;
    }
    Ok(())
}

/// Stops a tile and frees its mpv instance and window; `None` closes them all.
#[tauri::command]
pub fn tile_close(tiles: State<'_, Tiles>, index: Option<usize>) -> Result<(), String> {
    let mut tiles = tiles.0.lock().map_err(|e| e.to_string())?;
    let indexes: Vec<usize> = match index {
        Some(i) => vec![i],
        None => tiles.keys().copied().collect(),
    };
    for i in indexes {
        if let Some(tile) = tiles.remove(&i) {
            // The pump thread sees the shutdown and drops its handle, which destroys the instance.
            let _ = tile.mpv.command(&["quit"]);
            mpv::destroy_host(tile.host);
        }
    }
    Ok(())
}

/// Multi-view on: the main player's window steps aside so the tiles show.
#[tauri::command]
pub fn tiles_mode(host: State<'_, VideoHost>, on: bool) {
    mpv::show_host(host.0, !on);
}
