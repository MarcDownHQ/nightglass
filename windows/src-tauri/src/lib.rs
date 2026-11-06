mod mpv;
mod sources;

use std::collections::{HashMap, HashSet};
use std::time::{SystemTime, UNIX_EPOCH};

use mpv::Mpv;
use nightglass_core::{Catalog, Channel, Episode, Http, Programme, Series, Source, xmltv};
use tauri::{AppHandle, Manager, State, WindowEvent};
use url::Url;

/// The child window mpv draws into.
struct VideoHost(isize);

#[tauri::command]
fn mpv_play(mpv: State<'_, Mpv>, url: String, headers: HashMap<String, String>) -> Result<(), String> {
    // M3U playlists may ask for a user agent and a referrer per channel (`#EXTVLCOPT`).
    mpv.set("user-agent", headers.get("User-Agent").map_or("libmpv", String::as_str))?;
    mpv.set("referrer", headers.get("Referer").map_or("", String::as_str))?;
    mpv.command(&["loadfile", &url, "replace"])
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
async fn catalog(http: State<'_, Http>, source: Source) -> Result<Catalog, String> {
    source.client(&http).catalog().await.map_err(|e| e.to_string())
}

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

/// Programmes from 3 h ago to 24 h ahead, grouped by `xmltv::epg_key` and sorted. Only `keys` are kept when given.
#[tauri::command]
async fn guide(http: State<'_, Http>, url: Url, keys: Vec<String>) -> Result<HashMap<String, Vec<Programme>>, String> {
    let data = http.fetch(&url).await.map_err(|e| e.to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64);
        let wanted: HashSet<String> = keys.into_iter().collect();
        let mut out: HashMap<String, Vec<Programme>> = HashMap::new();
        for p in xmltv::parse(&data, now - 3 * 3600..=now + 86_400, None) {
            let key = xmltv::epg_key(&p.channel);
            if wanted.is_empty() || wanted.contains(&key) {
                out.entry(key).or_default().push(p);
            }
        }
        for list in out.values_mut() {
            list.sort_by_key(|p| p.start);
        }
        out
    })
    .await
    .map_err(|e| e.to_string())
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
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(tauri_plugin_log::Builder::default().level(log::LevelFilter::Info).build())?;
            }
            let window = app.get_webview_window("main").expect("main window");
            let host = mpv::video_host(window.hwnd()?.0 as isize);
            let dll = app.path().resource_dir()?.join("libmpv-2.dll");
            app.manage(Mpv::new(&dll, host)?);
            app.manage(VideoHost(host));
            app.manage(Http::new());
            let handle = app.handle().clone();
            std::thread::spawn(move || handle.state::<Mpv>().pump(&handle));
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
            stream_url,
            episodes,
            guide,
            parse_location,
            automation
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightglass");
}
