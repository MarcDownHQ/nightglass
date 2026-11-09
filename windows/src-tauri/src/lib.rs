mod cache;
#[cfg(target_os = "macos")]
mod host_macos;
mod mpv;
mod sources;
mod tiles;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use mpv::Mpv;
use nightglass_core::{Catalog, Channel, Episode, Http, Programme, Series, Source, xmltv};
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use url::Url;

/// A window's native handle, the parent of video windows: its HWND, or its content NSView on macOS.
macro_rules! native {
    ($window:expr) => {{
        #[cfg(windows)]
        let handle = $window.hwnd().map(|h| h.0 as isize);
        #[cfg(target_os = "macos")]
        let handle = $window.ns_view().map(|v| v as isize);
        handle
    }};
}

/// The child window mpv draws into.
pub struct VideoHost(pub isize);

/// The main window, parent of the video windows.
pub struct MainWindow(pub isize);

/// How far back the guide keeps programmes: the replay window of most Xtream servers.
const PAST: i64 = 3 * 86_400;

// mpv commands run off the main thread (`async`): on macOS, mpv's video output waits on the main thread, which would deadlock.
#[tauri::command(async)]
/// `options` are per-file mpv options, such as `start=93,sid=no`: they don't stick to the next file.
fn mpv_play(mpv: State<'_, Mpv>, url: String, headers: HashMap<String, String>, options: Option<String>) -> Result<(), String> {
    // M3U playlists may ask for a user agent and a referrer per channel (`#EXTVLCOPT`).
    mpv.set("user-agent", headers.get("User-Agent").map_or("libmpv", String::as_str))?;
    mpv.set("referrer", headers.get("Referer").map_or("", String::as_str))?;
    let options = options.as_deref().unwrap_or("");
    // mpv before 0.38 (the macOS build is 0.36) has no index argument: options come third there.
    mpv.command(&["loadfile", &url, "replace", "-1", options]).or_else(|_| mpv.command(&["loadfile", &url, "replace", options]))
}

#[tauri::command(async)]
fn mpv_command(mpv: State<'_, Mpv>, args: Vec<String>) -> Result<(), String> {
    mpv.command(&args.iter().map(String::as_str).collect::<Vec<_>>())
}

#[tauri::command(async)]
fn mpv_set(mpv: State<'_, Mpv>, name: String, value: String) -> Result<(), String> {
    mpv.set(&name, &value)
}

/// Where the video goes, as margins relative to the window: [left, top, right, bottom].
#[tauri::command(async)]
fn mpv_video_rect(mpv: State<'_, Mpv>, margins: [f64; 4]) -> Result<(), String> {
    for (side, value) in ["left", "top", "right", "bottom"].iter().zip(margins) {
        mpv.set(&format!("video-margin-ratio-{side}"), &value.clamp(0.0, 1.0).to_string())?;
    }
    Ok(())
}

/// Picture in picture: a small window above the others that the video moves into, leaving the main window free.
#[tauri::command]
async fn pip_open(app: AppHandle, mpv: State<'_, Mpv>) -> Result<(), String> {
    if app.get_webview_window("pip").is_some() {
        return Ok(());
    }
    let (w, h) = (480.0, 270.0);
    let mut builder = WebviewWindowBuilder::new(&app, "pip", WebviewUrl::App("index.html".into()))
        .title("Nightglass")
        .inner_size(w, h)
        .min_inner_size(240.0, 135.0)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .maximizable(false);
    // Bottom right of the main window's screen, above the taskbar.
    if let Some(m) = app.get_webview_window("main").and_then(|m| m.current_monitor().ok().flatten()) {
        let k = m.scale_factor();
        let right = f64::from(m.position().x) / k + f64::from(m.size().width) / k;
        let bottom = f64::from(m.position().y) / k + f64::from(m.size().height) / k;
        builder = builder.position(right - w - 24.0, bottom - h - 64.0);
    }
    let pip = builder.build().map_err(|e| e.to_string())?;
    // The whole PiP window is picture.
    for side in ["left", "top", "right", "bottom"] {
        mpv.set(&format!("video-margin-ratio-{side}"), "0")?;
    }
    mpv::adopt_host(app.state::<VideoHost>().0, native!(pip).map_err(|e| e.to_string())?);
    Ok(())
}

/// Closes the PiP window; the video goes back on its way out (see `run`).
#[tauri::command]
async fn pip_close(app: AppHandle) -> Result<(), String> {
    match app.get_webview_window("pip") {
        Some(pip) => pip.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
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
            let main = native!(window)?;
            let host = mpv::video_host(main);
            #[cfg(windows)]
            let dll = app.path().resource_dir()?.join("libmpv-2.dll");
            // In Contents/Frameworks, or lib/macos in dev builds: see build.rs.
            #[cfg(target_os = "macos")]
            let dll = std::path::PathBuf::from("@rpath/libmpv.dylib");
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
        .on_window_event(|window, event| match event {
            // The video window follows the size of the window it is in: the main one, or the PiP one.
            WindowEvent::Resized(size) => {
                if let Some(host) = window.try_state::<VideoHost>()
                    && native!(window).is_ok_and(|h| mpv::host_parent(host.0) == h)
                {
                    mpv::resize_host(host.0, size.width, size.height);
                }
            }
            // The video comes home before the PiP window goes: destroyed with it, mpv would lose where it draws.
            WindowEvent::CloseRequested { .. } if window.label() == "pip" => {
                mpv::adopt_host(window.state::<VideoHost>().0, window.state::<MainWindow>().0);
                let _ = window.emit_to("main", "pip-closed", ());
            }
            // The PiP window alone would keep the app running.
            WindowEvent::Destroyed if window.label() == "main" => window.app_handle().exit(0),
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            mpv_play,
            mpv_command,
            mpv_set,
            mpv_video_rect,
            pip_open,
            pip_close,
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
