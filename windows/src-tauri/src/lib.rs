mod cache;
#[cfg(target_os = "linux")]
mod host_linux;
#[cfg(target_os = "macos")]
mod host_macos;
mod mpv;
mod sources;
mod tiles;
mod widevine;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use mpv::Mpv;
use nightglass_core::{Catalog, Channel, Chapter, Episode, Http, IptvError, Movie, Playback, Programme, Prompt, Provider, ProviderInfo, Providers, RelayServer, Series, Source, Unavailable, chapters_in_order, xmltv};
use nightglass_plugin_host::PluginStatus;
use tauri::async_runtime::RwLock;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use url::Url;
use widevine::Widevine;

/// A window's native handle, the parent of video windows: its HWND, its content NSView on macOS, or its video
/// layer on Linux.
macro_rules! native {
    ($window:expr) => {{
        #[cfg(windows)]
        let handle = $window.hwnd().map(|h| h.0 as isize);
        #[cfg(target_os = "macos")]
        let handle = $window.ns_view().map(|v| v as isize);
        #[cfg(target_os = "linux")]
        let handle = $window.default_vbox().and_then(|b| mpv::video_layer(&b).ok_or(tauri::Error::WindowNotFound));
        handle
    }};
}

/// The child window mpv draws into.
pub struct VideoHost(pub isize);

/// The main window, parent of the video windows.
pub struct MainWindow(pub isize);

/// How far back the guide keeps programmes: the replay window of most Xtream servers.
const PAST: i64 = 3 * 86_400;

// mpv commands run off the main thread (`async`): on macOS and Linux, mpv's video output waits on the main thread, which would deadlock.
#[tauri::command(async)]
/// `options` are per-file mpv options, such as `start=93,sid=no`: they don't stick to the next file.
fn mpv_play(mpv: State<'_, Mpv>, url: String, headers: HashMap<String, String>, options: Option<String>) -> Result<(), String> {
    mpv.headers(&headers)?;
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

/// Where the video goes in the main window: x, y, width, height in physical pixels. The video window
/// itself moves, mpv's picture stays as it is: nothing to redraw, so it keeps up with a scrolling page.
#[tauri::command]
fn video_place(host: State<'_, VideoHost>, main: State<'_, MainWindow>, rect: [i32; 4]) {
    // In the PiP window it fills that window instead.
    if mpv::host_parent(host.0) == main.0 {
        mpv::move_host(host.0, rect);
    }
}

/// Picture in picture: a small window above the others that the video moves into, leaving the main window free.
#[tauri::command]
async fn pip_open(app: AppHandle) -> Result<(), String> {
    if app.get_webview_window("pip").is_some() {
        return Ok(());
    }
    // Linux: the window gets its video, and with it its GL context, before GTK first paints it. Otherwise WebKit's
    // first GL draw lands in the middle of a software-drawn frame, which compositors with explicit sync reject.
    #[cfg(target_os = "linux")]
    {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = app.clone();
        app.run_on_main_thread(move || {
            let _ = tx.send(open_pip(&handle));
        })
        .map_err(|e| e.to_string())?;
        rx.recv().map_err(|e| e.to_string())?
    }
    #[cfg(not(target_os = "linux"))]
    open_pip(&app)
}

fn open_pip(app: &AppHandle) -> Result<(), String> {
    let (w, h) = (480.0, 270.0);
    let mut builder = WebviewWindowBuilder::new(app, "pip", WebviewUrl::App("index.html".into()))
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
#[tauri::command]
async fn pip_close(app: AppHandle) -> Result<(), String> {
    match app.get_webview_window("pip") {
        Some(pip) => pip.close().map_err(|e| e.to_string()),
        None => Ok(()),
    }
}

/// Write-locked while the plugins load, so commands wait for them rather than miss them.
struct Registry {
    dir: std::path::PathBuf,
    loaded: Arc<RwLock<Loaded>>,
}

struct Loaded {
    providers: Providers,
    plugins: Vec<PluginStatus>,
}

impl Registry {
    async fn get(&self, id: &str) -> Result<Arc<dyn Provider>, String> {
        self.loaded.read().await.providers.get(id).cloned().map_err(|e| e.to_string())
    }
}

/// The plugins folder and what became of each file in it at launch.
#[derive(serde::Serialize)]
struct Plugins {
    dir: std::path::PathBuf,
    plugins: Vec<PluginStatus>,
}

#[tauri::command]
async fn plugins(registry: State<'_, Registry>) -> Result<Plugins, String> {
    Ok(Plugins { dir: registry.dir.clone(), plugins: registry.loaded.read().await.plugins.clone() })
}

/// The source types the add form offers, and what each one asks for.
#[tauri::command]
async fn providers(registry: State<'_, Registry>) -> Result<Vec<ProviderInfo>, String> {
    Ok(registry.loaded.read().await.providers.infos())
}

#[tauri::command]
fn sources_load(app: AppHandle) -> Result<Vec<Source>, String> {
    sources::load(&sources::file(&app)?)
}

#[tauri::command]
fn sources_save(app: AppHandle, sources: Vec<Source>) -> Result<(), String> {
    sources::save(&sources::file(&app)?, &sources)
}

/// A command's error as the page gets it: a message, `{ prompt }` when a plugin waits for the user, or
/// `{ protected: "off" }` when an encrypted stream waits for protected content to be allowed.
#[derive(serde::Serialize)]
#[serde(untagged)]
enum Failure {
    Message(String),
    ActionRequired { prompt: Box<Prompt> },
    ProtectedOff { protected: &'static str },
}

impl From<String> for Failure {
    fn from(message: String) -> Self {
        Failure::Message(message)
    }
}

impl From<IptvError> for Failure {
    fn from(e: IptvError) -> Self {
        match e {
            IptvError::ActionRequired(prompt) => Failure::ActionRequired { prompt },
            e => Failure::Message(e.to_string()),
        }
    }
}

// shortcut: whole catalogs cross the IPC bridge as JSON; page them from Rust if a provider's tens of thousands of entries get slow.
#[tauri::command]
async fn catalog(app: AppHandle, registry: State<'_, Registry>, source: Source) -> Result<Catalog, Failure> {
    let catalog = registry.get(&source.provider).await?.catalog(&source.config).await?;
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

/// For providers whose links are short-lived (Stalker), asked at play time; the others return the stream as is.
#[tauri::command]
async fn stream_url(registry: State<'_, Registry>, relay: State<'_, RelayServer>, widevine: State<'_, Widevine>, source: Source, channel: Channel) -> Result<PlayerInput, Failure> {
    let provider = registry.get(&source.provider).await?;
    let playback = provider.stream_url(&source.config, &channel).await?;
    playable(&relay, &widevine, provider, source, &channel.headers, playback).await
}

/// What mpv opens: one address, the playback's tracks joined into it. `link` is the stream's own address, to copy.
#[derive(serde::Serialize)]
struct PlayerInput {
    url: String,
    headers: HashMap<String, String>,
    link: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    chapters: Vec<Chapter>,
}

/// The item's own headers under those the provider gave; a playback the app fetches itself moves to 127.0.0.1, an
/// encrypted one through the CDM once protected content is on.
async fn playable(relay: &RelayServer, widevine: &Widevine, provider: Arc<dyn Provider>, source: Source, headers: &HashMap<String, String>, mut playback: Playback) -> Result<PlayerInput, Failure> {
    for (name, value) in headers {
        if !playback.headers.keys().any(|k| k.eq_ignore_ascii_case(name)) {
            playback.headers.insert(name.clone(), value.clone());
        }
    }
    let link = playback.url.to_string();
    let chapters = chapters_in_order(std::mem::take(&mut playback.chapters));
    if playback.drm.is_some() {
        let cdm = widevine.cdm().ok_or(Failure::ProtectedOff { protected: "off" })?;
        playback = widevine.open(&cdm, provider, source.config, playback).await?;
    } else if playback.relay.is_some() {
        playback = relay.relay(provider, source.config, playback).await.map_err(|e| e.to_string())?;
    }
    Ok(PlayerInput { url: playback.player_url(), headers: playback.headers, link, chapters })
}

#[tauri::command]
async fn movie_url(registry: State<'_, Registry>, relay: State<'_, RelayServer>, widevine: State<'_, Widevine>, source: Source, movie: Movie) -> Result<PlayerInput, Failure> {
    let provider = registry.get(&source.provider).await?;
    let playback = provider.movie_url(&source.config, &movie).await?;
    playable(&relay, &widevine, provider, source, &HashMap::new(), playback).await
}

#[tauri::command]
async fn episode_url(registry: State<'_, Registry>, relay: State<'_, RelayServer>, widevine: State<'_, Widevine>, source: Source, series: Series, episode: Episode) -> Result<PlayerInput, Failure> {
    let provider = registry.get(&source.provider).await?;
    let playback = provider.episode_url(&source.config, &series, &episode).await?;
    playable(&relay, &widevine, provider, source, &HashMap::new(), playback).await
}

/// A series' episodes, asked for when its page opens.
#[tauri::command]
async fn episodes(registry: State<'_, Registry>, source: Source, series: Series) -> Result<Vec<Episode>, Failure> {
    Ok(registry.get(&source.provider).await?.episodes(&source.config, &series).await?)
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

/// A past programme, from the servers that keep them.
#[tauri::command]
async fn catchup_url(registry: State<'_, Registry>, relay: State<'_, RelayServer>, widevine: State<'_, Widevine>, source: Source, channel: Channel, start: i64, end: i64) -> Result<PlayerInput, Failure> {
    let provider = registry.get(&source.provider).await?;
    let playback = provider.catchup_url(&source.config, &channel, start, end).await?;
    playable(&relay, &widevine, provider, source, &channel.headers, playback).await
}

/// A link a plugin gave, in the system browser: http(s) only.
#[tauri::command]
fn open_link(url: Url) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("not a web address: {url}"));
    }
    #[cfg(windows)]
    let (program, before) = ("rundll32", Some("url.dll,FileProtocolHandler"));
    #[cfg(target_os = "macos")]
    let (program, before) = ("open", None::<&str>);
    #[cfg(target_os = "linux")]
    let (program, before) = ("xdg-open", None::<&str>);
    let mut child = std::process::Command::new(program).args(before).arg(url.as_str()).spawn().map_err(|e| e.to_string())?;
    std::thread::spawn(move || child.wait());
    Ok(())
}

/// Where a recording goes: `Videos\Nightglass\<name>.ts`, the folder made if needed.
#[tauri::command]
fn record_path(app: AppHandle, name: String) -> Result<String, String> {
    let dir = app.path().video_dir().map_err(|e| e.to_string())?.join("Nightglass");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let safe: String = name.chars().map(|c| if c.is_control() || r#"<>:"/\|?*"#.contains(c) { '_' } else { c }).collect();
    Ok(dir.join(format!("{}.ts", safe.trim())).to_string_lossy().into_owned())
}

/// What the user typed as a playlist, guide or portal address: a URL, or (with `files`) a path to a local file.
#[tauri::command]
fn parse_location(input: String, files: bool) -> Result<Url, String> {
    let input = input.trim();
    let path = std::path::Path::new(input);
    if files && path.is_absolute() {
        return Url::from_file_path(path).map_err(|_| format!("Invalid path: {input}"));
    }
    match Url::parse(input) {
        Ok(u) if matches!(u.scheme(), "http" | "https") || (files && u.scheme() == "file") => Ok(u),
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

fn load_plugins(dir: std::path::PathBuf, data: std::path::PathBuf, http: &Http) -> Registry {
    let loaded = Arc::new(RwLock::new(Loaded { providers: Providers::builtin(http), plugins: Vec::new() }));
    let mut guard = loaded.clone().try_write_owned().expect("unshared lock");
    let (http, folder) = (http.clone(), dir.clone());
    tauri::async_runtime::spawn(async move {
        if let Err(e) = std::fs::create_dir_all(&folder) {
            log::warn!("{}: {e}", folder.display());
        }
        match nightglass_plugin_host::Host::new(&http) {
            Ok(host) => {
                let host = host.with_data_dir(data);
                let Loaded { providers, plugins } = &mut *guard;
                *plugins = host.load_dir(&folder, providers).await;
            }
            Err(e) => log::error!("plugin host: {e:#}"),
        }
    });
    Registry { dir, loaded }
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
            #[cfg(target_os = "linux")]
            let dll = std::path::PathBuf::from("libmpv.so.2");
            app.manage(Mpv::new(&dll, host)?);
            app.manage(VideoHost(host));
            app.manage(MainWindow(main));
            app.manage(tiles::MpvDll(dll));
            app.manage(tiles::Tiles::default());
            let http = Http::new();
            let app_data = app.path().app_data_dir()?;
            app.manage(load_plugins(app_data.join("plugins"), app_data.join("plugin-data"), &http));
            app.manage(RelayServer::new(http.clone()));
            app.manage(Widevine::new(app_data.join("widevine"), Arc::new(Unavailable)));
            app.manage(http);
            let handle = app.handle().clone();
            std::thread::spawn(move || handle.state::<Mpv>().pump(&handle, "mpv", None));
            Ok(())
        })
        .on_window_event(|window, event| match event {
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
        })
        .invoke_handler(tauri::generate_handler![
            mpv_play,
            mpv_command,
            mpv_set,
            video_place,
            pip_open,
            pip_close,
            providers,
            plugins,
            sources_load,
            sources_save,
            catalog,
            catalog_cached,
            guide_cached,
            cache_drop,
            stream_url,
            movie_url,
            episode_url,
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
            open_link,
            automation,
            widevine::widevine_status,
            widevine::widevine_install,
            widevine::widevine_remove
        ])
        .run(tauri::generate_context!())
        .expect("error while running Nightglass");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_prompt_reaches_the_page_as_an_object_other_errors_as_text() {
        let prompt = Prompt { message: "Sign in".into(), url: None, code: Some("WDJB-MJHT".into()), retry_in: Some(5) };
        let json = serde_json::to_value(Failure::from(IptvError::ActionRequired(Box::new(prompt)))).unwrap();
        assert_eq!(json, serde_json::json!({"prompt": {"message": "Sign in", "url": null, "code": "WDJB-MJHT", "retryIn": 5}}));
        assert_eq!(serde_json::to_value(Failure::from(IptvError::AuthFailed)).unwrap(), "The server refused the credentials.");
    }

    #[test]
    fn protected_content_off_reaches_the_page_as_an_object() {
        assert_eq!(serde_json::to_value(Failure::ProtectedOff { protected: "off" }).unwrap(), serde_json::json!({"protected": "off"}));
        assert_eq!(serde_json::to_value(Failure::from(IptvError::Drm("not implemented yet".into()))).unwrap(), "Protected playback failed: not implemented yet");
    }
}
