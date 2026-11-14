#[cfg(mobile)]
mod android;
mod cache;
#[cfg(target_os = "macos")]
mod host_macos;
#[cfg(desktop)]
mod mpv;
mod sources;
#[cfg(desktop)]
mod tiles;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(desktop)]
use mpv::Mpv;
use nightglass_core::{Catalog, Channel, Episode, Http, Info, Movie, Programme, Series, Source, xmltv};
#[cfg(desktop)]
use tauri::{Emitter, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri::{AppHandle, Manager, State};
use url::Url;

// Multi-view needs several decoders drawing into windows of their own: not on a TV. The commands exist so the page's calls resolve.
#[cfg(mobile)]
mod tiles {
    #[tauri::command]
    pub fn tile_play() -> Result<(), String> {
        Err("Multi-view is not available on Android TV.".into())
    }
    #[tauri::command]
    pub fn tile_rect() -> Result<(), String> {
        Ok(())
    }
    #[tauri::command]
    pub fn tile_mute() -> Result<(), String> {
        Ok(())
    }
    #[tauri::command]
    pub fn tile_close() -> Result<(), String> {
        Ok(())
    }
    #[tauri::command]
    pub fn tiles_mode() {}
}

/// A window's native handle, the parent of video windows: its HWND, or its content NSView on macOS.
#[cfg(desktop)]
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
#[cfg(desktop)]
pub struct VideoHost(pub isize);

/// The main window, parent of the video windows.
#[cfg(desktop)]
pub struct MainWindow(pub isize);

/// How far back the guide keeps programmes: the replay window of most Xtream servers.
const PAST: i64 = 3 * 86_400;

// mpv commands run off the main thread (`async`): on macOS, mpv's video output waits on the main thread, which would deadlock.
#[cfg(desktop)]
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

#[cfg(desktop)]
#[tauri::command(async)]
fn mpv_command(mpv: State<'_, Mpv>, args: Vec<String>) -> Result<(), String> {
    mpv.command(&args.iter().map(String::as_str).collect::<Vec<_>>())
}

#[cfg(desktop)]
#[tauri::command(async)]
fn mpv_set(mpv: State<'_, Mpv>, name: String, value: String) -> Result<(), String> {
    mpv.set(&name, &value)
}

/// Where the video goes in the main window: x, y, width, height in physical pixels. The video window
/// itself moves, mpv's picture stays as it is: nothing to redraw, so it keeps up with a scrolling page.
#[cfg(desktop)]
#[tauri::command]
fn video_place(host: State<'_, VideoHost>, main: State<'_, MainWindow>, rect: [i32; 4]) {
    // In the PiP window it fills that window instead.
    if mpv::host_parent(host.0) == main.0 {
        mpv::move_host(host.0, rect);
    }
}

/// Picture in picture: a small window above the others that the video moves into, leaving the main window free.
#[cfg(desktop)]
#[tauri::command]
async fn pip_open(app: AppHandle) -> Result<(), String> {
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
    mpv::adopt_host(app.state::<VideoHost>().0, native!(pip).map_err(|e| e.to_string())?);
    Ok(())
}

/// Closes the PiP window; the video goes back on its way out (see `run`).
#[cfg(desktop)]
#[tauri::command]
async fn pip_close(app: AppHandle) -> Result<(), String> {
    match app.get_webview_window("pip") {
        Some(pip) => pip.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

#[cfg(mobile)]
use android::{mpv_command, mpv_play, mpv_set, pip_close, pip_open, video_place};

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
async fn catalog(app: AppHandle, http: State<'_, Http>, source: Source, reduced: Option<bool>) -> Result<Catalog, String> {
    let client = reduced.map_or(source.client(&http), |on| source.client(&http).reduced(on));
    let catalog = client.catalog().await.map_err(|e| e.to_string())?;
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

/// What the provider says about a film (Xtream only; other sources have nothing more than the list).
#[tauri::command]
async fn movie_info(http: State<'_, Http>, source: Source, movie: Movie) -> Result<Info, String> {
    source.client(&http).movie_info(&movie).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn series_info(http: State<'_, Http>, source: Source, series: Series) -> Result<Info, String> {
    source.client(&http).series_info(&series).await.map_err(|e| e.to_string())
}

/// Opens an address in the system's own app: on a TV, a YouTube trailer opens the YouTube app.
#[cfg(mobile)]
use android::open_url;
#[cfg(desktop)]
#[tauri::command]
fn open_url(_url: String) -> Result<(), String> {
    Err("Not available here.".into())
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

#[cfg(desktop)]
fn desktop_setup(app: &mut tauri::App) -> Result<(), Box<dyn std::error::Error>> {
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
    let handle = app.handle().clone();
    std::thread::spawn(move || handle.state::<Mpv>().pump(&handle, "mpv", None));
    Ok(())
}

#[cfg(desktop)]
fn window_event(window: &tauri::Window, event: &WindowEvent) {
    match event {
        // In the PiP window, the video window fills it. In the main one the page places it (`video_place`).
        WindowEvent::Resized(size) if window.label() == "pip" => {
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
    }
}

#[cfg(mobile)]
fn window_event(_: &tauri::Window, _: &tauri::WindowEvent) {}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(desktop)]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(mobile)]
    let builder = builder.plugin(android::plugin());
    builder
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            app.manage(Http::new());
            #[cfg(desktop)]
            desktop_setup(app)?;
            Ok(())
        })
        .on_window_event(window_event)
        .invoke_handler(tauri::generate_handler![
            mpv_play,
            mpv_command,
            mpv_set,
            video_place,
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
            movie_info,
            series_info,
            open_url,
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
