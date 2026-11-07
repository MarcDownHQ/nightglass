mod cache;
mod mpv;
mod sources;
mod tiles;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use mpv::Mpv;
use nightglass_core::{Catalog, Channel, Episode, Http, Programme, Series, Source, xmltv};
use tauri::{AppHandle, Manager, State, WindowEvent};
use url::Url;

/// The child window mpv draws into.
pub struct VideoHost(pub isize);

/// The main window, parent of the video windows.
pub struct MainWindow(pub isize);

/// How far back the guide keeps programmes: the replay window of most Xtream servers.
const PAST: i64 = 3 * 86_400;

#[tauri::command]
/// `options` are per-file mpv options, such as `start=93,sid=no`: they don't stick to the next file.
fn mpv_play(mpv: State<'_, Mpv>, url: String, headers: HashMap<String, String>, options: Option<String>) -> Result<(), String> {
    // M3U playlists may ask for a user agent and a referrer per channel (`#EXTVLCOPT`).
    mpv.set("user-agent", headers.get("User-Agent").map_or("libmpv", String::as_str))?;
    mpv.set("referrer", headers.get("Referer").map_or("", String::as_str))?;
    mpv.command(&["loadfile", &url, "replace", "-1", options.as_deref().unwrap_or("")])
}

#[tauri::command]
fn mpv_command(mpv: State<'_, Mpv>, args: Vec<String>) -> Result<(), String> {
    mpv.command(&args.iter().map(String::as_str).collect::<Vec<_>>())
}

#[tauri::command]
fn mpv_set(mpv: State<'_, Mpv>, name: String, value: String) -> Result<(), String> {
    mpv.set(&name, &value)
}

/// Where the video goes, as margins relative to the window: [left, top, right, bottom].
#[tauri::command]
fn mpv_video_rect(mpv: State<'_, Mpv>, margins: [f64; 4]) -> Result<(), String> {
    for (side, value) in ["left", "top", "right", "bottom"].iter().zip(margins) {
        mpv.set(&format!("video-margin-ratio-{side}"), &value.clamp(0.0, 1.0).to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn sources_load(app: AppHandle) -> Result<Vec<Source>, String> {
    Ok(sources::load(&sources::file(&app)?))
}

#[tauri::command]
fn sources_save(app: AppHandle, sources: Vec<Source>) -> Result<(), String> {
    sources::save(&sources::file(&app)?, &sources)
}

// shortcut: whole catalogs cross the IPC bridge as JSON; page them from Rust if a provider's tens of thousands of entries get slow.
#[tauri::command]
async fn catalog(app: AppHandle, http: State<'_, Http>, source: Source) -> Result<Catalog, String> {
    let catalog = source.client(&http).catalog().await.map_err(|e| e.to_string())?;
    let (id, copy) = (source.id.to_string(), catalog.clone());
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = cache::write(&app, "catalog", &id, &copy) {
            log::warn!("catalog cache: {e}");
        }
    });
    Ok(catalog)
}

/// The catalog the source returned last time, if any.
#[tauri::command]
async fn catalog_cached(app: AppHandle, id: String) -> Result<Option<Catalog>, String> {
    tauri::async_runtime::spawn_blocking(move || cache::read(&app, "catalog", &id)).await.map_err(|e| e.to_string())
}

/// The guide the source returned last time, without the programmes that have ended.
#[tauri::command]
async fn guide_cached(app: AppHandle, id: String) -> Result<Option<Guide>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let mut guide: Guide = cache::read(&app, "guide", &id)?;
        let now = unix_now();
        guide.retain(|_, list| {
            list.retain(|p| p.end > now - PAST);
            !list.is_empty()
        });
        Some(guide)
    })
    .await
    .map_err(|e| e.to_string())
}

#[tauri::command]
fn cache_drop(app: AppHandle, id: String) {
    cache::drop(&app, &id);
}

fn unix_now() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64)
}

type Guide = HashMap<String, Vec<Programme>>;

/// Stalker links are short-lived, so they are asked for at play time; other sources return the stream as is.
#[tauri::command]
async fn stream_url(http: State<'_, Http>, source: Source, channel: Channel) -> Result<Url, String> {
    source.client(&http).stream_url(&channel).await.map_err(|e| e.to_string())
}

/// A series' episodes, asked for when its page opens (Xtream only; other sources have none).
#[tauri::command]
async fn episodes(http: State<'_, Http>, source: Source, series: Series) -> Result<Vec<Episode>, String> {
    source.client(&http).episodes(&series).await.map_err(|e| e.to_string())
}

/// Programmes from 3 days ago (for replay) to 24 h ahead, grouped by `xmltv::epg_key` and sorted. Only `keys` are kept when given.
/// `cache` is the source id to keep the guide under for the next launch.
#[tauri::command]
async fn guide(app: AppHandle, http: State<'_, Http>, url: Url, keys: Vec<String>, cache: Option<String>) -> Result<Guide, String> {
    // Guides run to hundreds of MB: written to disk as they download, then read from there.
    let file = http.to_disk(&url).await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let now = unix_now();
        let wanted: HashSet<String> = keys.into_iter().collect();
        let mut out: Guide = HashMap::new();
        let programmes = xmltv::parse_file(&file.path, now - PAST..=now + 86_400, None).map_err(|e| e.to_string())?;
        for p in programmes {
            let key = xmltv::epg_key(&p.channel);
            if wanted.is_empty() || wanted.contains(&key) {
                out.entry(key).or_default().push(p);
            }
        }
        for list in out.values_mut() {
            list.sort_by_key(|p| p.start);
        }
        if let Some(id) = cache {
            if let Err(e) = cache::write(&app, "guide", &id, &out) {
                log::warn!("guide cache: {e}");
            }
            // The guide's own channel list, to match a channel by hand.
            if let Err(e) = xmltv::channels_file(&file.path).map_err(|e| e.to_string()).and_then(|c| cache::write(&app, "epgchannels", &id, &c)) {
                log::warn!("guide channels cache: {e}");
            }
        }
        Ok(out)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// The channels the source's guide describes, (id, name), as last downloaded.
#[tauri::command]
async fn guide_channels(app: AppHandle, id: String) -> Result<Vec<(String, String)>, String> {
    tauri::async_runtime::spawn_blocking(move || cache::read(&app, "epgchannels", &id).unwrap_or_default())
        .await
        .map_err(|e| e.to_string())
}

/// A past programme, from the servers that keep them (Xtream `tv_archive`).
#[tauri::command]
async fn catchup_url(http: State<'_, Http>, source: Source, channel: Channel, start: i64, end: i64) -> Result<Url, String> {
    source.client(&http).catchup_url(&channel, start, end).await.map_err(|e| e.to_string())
}

/// Where a recording goes: `Videos\Nightglass\<name>.ts`, the folder made if needed.
#[tauri::command]
fn record_path(app: AppHandle, name: String) -> Result<String, String> {
    let dir = app.path().video_dir().map_err(|e| e.to_string())?.join("Nightglass");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = name.chars().map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect();
    Ok(dir.join(format!("{}.ts", safe.trim())).to_string_lossy().into_owned())
}

/// What the user typed as a playlist, guide or portal address: a URL, or a Windows path to a local file.
#[tauri::command]
fn parse_location(input: String) -> Result<Url, String> {
    let input = input.trim();
    let path = std::path::Path::new(input);
    if path.is_absolute() {
        return Url::from_file_path(path).map_err(|_| format!("Invalid path: {input}"));
    }
    match Url::parse(input) {
        Ok(u) if matches!(u.scheme(), "http" | "https" | "file") => Ok(u),
        _ => Err("Invalid URL".into()),
    }
}

/// Debug-only test hooks, read from the environment (see `src/lib/automation.ts`).
#[tauri::command]
fn automation() -> HashMap<String, String> {
    if !cfg!(debug_assertions) {
        return HashMap::new();
    }
    std::env::vars().filter(|(k, _)| k.starts_with("NG_")).collect()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            let window = app.get_webview_window("main").expect("main window");
            let main = window.hwnd()?.0 as isize;
            let host = mpv::video_host(main);
            let dll = app.path().resource_dir()?.join("libmpv-2.dll");
            app.manage(Mpv::new(&dll, host)?);
            app.manage(VideoHost(host));
            app.manage(MainWindow(main));
            app.manage(tiles::MpvDll(dll));
            app.manage(tiles::Tiles::default());
            app.manage(Http::new());
            let handle = app.handle().clone();
            std::thread::spawn(move || handle.state::<Mpv>().pump(&handle, "mpv", None));
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::Resized(size) = event
                && let Some(host) = window.try_state::<VideoHost>()
            {
                mpv::resize_host(host.0, size.width, size.height);
            }
        })
        .invoke_handler(tauri::generate_handler![
            mpv_play,
            mpv_command,
            mpv_set,
            mpv_video_rect,
            sources_load,
            sources_save,
            catalog,
            catalog_cached,
            guide_cached,
            cache_drop,
            stream_url,
            episodes,
            guide,
            guide_channels,
            catchup_url,
            record_path,
            sources::settings_export,
            sources::settings_import,
            tiles::tile_play,
            tiles::tile_rect,
            tiles::tile_mute,
            tiles::tile_close,
            tiles::tiles_mode,
            parse_location,
            automation
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightglass");
}
