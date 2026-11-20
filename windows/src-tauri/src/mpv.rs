//! libmpv, loaded at runtime (libmpv-2.dll, libmpv.2.dylib, libmpv.so.2). mpv draws into a child window (an NSView
//! on macOS, see `host_macos`; a GtkGLArea on Linux, see `host_linux`) kept under the transparent webview, so the UI can sit on top of the video.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;

use libloading::Library;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
#[cfg(target_os = "linux")]
pub use crate::host_linux::*;
#[cfg(target_os = "macos")]
pub use crate::host_macos::*;
#[cfg(windows)]
use windows_sys::Win32::Foundation::RECT;
#[cfg(windows)]
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DestroyWindow, GetClientRect, GetParent, HWND_BOTTOM, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SWP_NOZORDER, SetParent, SetWindowPos, ShowWindow, WS_CHILD, WS_EX_NOPARENTNOTIFY, WS_VISIBLE,
};

type Handle = *mut c_void;

#[repr(C)]
struct Event {
    event_id: c_int,
    error: c_int,
    reply_userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct EventProperty {
    name: *const c_char,
    format: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct EventEndFile {
    reason: c_int,
    error: c_int,
}

#[repr(C)]
struct EventLogMessage {
    prefix: *const c_char,
    level: *const c_char,
    text: *const c_char,
}

const FORMAT_STRING: c_int = 1;
const FORMAT_FLAG: c_int = 3;
const FORMAT_INT64: c_int = 4;
const FORMAT_DOUBLE: c_int = 5;

/// The video track, followed for the renderer (see `host_linux`), not the UI.
#[cfg(target_os = "linux")]
const VID: u64 = u64::MAX;

/// Properties the UI follows, with the format mpv reports them in.
const OBSERVED: &[(&str, c_int)] = &[
    ("pause", FORMAT_FLAG),
    ("paused-for-cache", FORMAT_FLAG),
    ("time-pos", FORMAT_DOUBLE),
    ("duration", FORMAT_DOUBLE),
    ("width", FORMAT_INT64),
    ("height", FORMAT_INT64),
    // End of what the cache holds: on live TV, how far the picture can catch up after a pause.
    ("demuxer-cache-time", FORMAT_DOUBLE),
    // Node properties read as a string come back as JSON.
    ("track-list", FORMAT_STRING),
    // Only its oldest seekable second goes on, as `cache-start`: the start of the buffer bar.
    ("demuxer-cache-state", FORMAT_STRING),
];

struct Api {
    create: unsafe extern "C" fn() -> Handle,
    initialize: unsafe extern "C" fn(Handle) -> c_int,
    set_option_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    set_property_string: unsafe extern "C" fn(Handle, *const c_char, *const c_char) -> c_int,
    command: unsafe extern "C" fn(Handle, *mut *const c_char) -> c_int,
    observe_property: unsafe extern "C" fn(Handle, u64, *const c_char, c_int) -> c_int,
    request_log_messages: unsafe extern "C" fn(Handle, *const c_char) -> c_int,
    wait_event: unsafe extern "C" fn(Handle, f64) -> *mut Event,
    error_string: unsafe extern "C" fn(c_int) -> *const c_char,
    terminate_destroy: unsafe extern "C" fn(Handle),
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    render: Option<RenderFns>,
    _lib: Library,
}

fn symbol<T: Copy>(lib: &Library, name: &str) -> Result<T, String> {
    let bytes = format!("{name}\0");
    unsafe { lib.get::<T>(bytes.as_bytes()).map(|s| *s).map_err(|e| format!("{name}: {e}")) }
}

impl Api {
    fn load(dll: &Path) -> Result<Self, String> {
        let lib = unsafe { Library::new(dll) }.map_err(|e| format!("{}: {e}", dll.display()))?;
        Ok(Api {
            create: symbol(&lib, "mpv_create")?,
            initialize: symbol(&lib, "mpv_initialize")?,
            set_option_string: symbol(&lib, "mpv_set_option_string")?,
            set_property_string: symbol(&lib, "mpv_set_property_string")?,
            command: symbol(&lib, "mpv_command")?,
            observe_property: symbol(&lib, "mpv_observe_property")?,
            request_log_messages: symbol(&lib, "mpv_request_log_messages")?,
            wait_event: symbol(&lib, "mpv_wait_event")?,
            error_string: symbol(&lib, "mpv_error_string")?,
            terminate_destroy: symbol(&lib, "mpv_terminate_destroy")?,
            #[cfg(target_os = "macos")]
            render: Some(RenderFns {
                create: symbol(&lib, "mpv_render_context_create")?,
                render: symbol(&lib, "mpv_render_context_render")?,
                update: symbol(&lib, "mpv_render_context_update")?,
                free: symbol(&lib, "mpv_render_context_free")?,
            }),
            #[cfg(target_os = "linux")]
            render: Some(RenderFns {
                create: symbol(&lib, "mpv_render_context_create")?,
                render: symbol(&lib, "mpv_render_context_render")?,
                update: symbol(&lib, "mpv_render_context_update")?,
                free: symbol(&lib, "mpv_render_context_free")?,
                command_async: symbol(&lib, "mpv_command_async")?,
            }),
            _lib: lib,
        })
    }
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

/// An mpv instance: the main player lives until the process exits, multi-view tiles come and go.
pub struct Mpv {
    api: Api,
    handle: Handle,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    renderer: Option<std::sync::Arc<Renderer>>,
}

// The libmpv client API is thread-safe.
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Mpv {
    pub fn new(dll: &Path, wid: isize) -> Result<Self, String> {
        #[cfg_attr(windows, allow(unused_mut))]
        let mut api = Api::load(dll)?;
        #[cfg(target_os = "linux")]
        numeric_locale();
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("mpv_create failed".into());
        }
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        let render = api.render.take();
        #[cfg_attr(windows, allow(unused_mut))]
        let mut mpv = Mpv {
            api,
            handle,
            #[cfg(any(target_os = "macos", target_os = "linux"))]
            renderer: None,
        };
        let wid = wid.to_string();
        // macOS and Linux: no `wid` there; the render API draws into the host view's layer, or the host area.
        let (wid_option, vo) = if cfg!(any(target_os = "macos", target_os = "linux")) { ("vo", "libmpv") } else { ("wid", wid.as_str()) };
        let options = [
            (wid_option, vo),
            // Ignore the user's own mpv.conf and scripts: the app owns every setting.
            ("config", "no"),
            ("load-scripts", "no"),
            ("osc", "no"),
            ("ytdl", "no"),
            ("terminal", "no"),
            ("osd-level", "0"),
            ("input-default-bindings", "no"),
            ("input-vo-keyboard", "no"),
            ("input-cursor", "no"),
            ("cursor-autohide", "no"),
            // Keep the video window alive between streams, so it never pops above the webview.
            ("force-window", "yes"),
            ("idle", "yes"),
            #[cfg(windows)]
            ("vo", "gpu-next"),
            ("hwdec", "auto-safe"),
            ("background-color", "#000000"),
            ("network-timeout", "20"),
            // A back buffer for live TV: pause, rewind and catch up within it.
            ("cache", "yes"),
            ("demuxer-max-bytes", "150MiB"),
            ("demuxer-max-back-bytes", "300MiB"),
        ];
        for (name, value) in options {
            if let Err(e) = mpv.check(unsafe { (mpv.api.set_option_string)(handle, c(name).as_ptr(), c(value).as_ptr()) }) {
                log::warn!("mpv option {name}={value}: {e}");
            }
        }
        mpv.check(unsafe { (mpv.api.initialize)(handle) })?;
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if let Some(fns) = render {
            mpv.renderer = Some(attach(wid.parse().unwrap_or(0), handle, fns)?);
        }
        unsafe { (mpv.api.request_log_messages)(handle, c("warn").as_ptr()) };
        for (i, (name, format)) in OBSERVED.iter().enumerate() {
            unsafe { (mpv.api.observe_property)(handle, i as u64, c(name).as_ptr(), *format) };
        }
        #[cfg(target_os = "linux")]
        unsafe {
            (mpv.api.observe_property)(handle, VID, c("vid").as_ptr(), FORMAT_STRING)
        };
        Ok(mpv)
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| c(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).chain([std::ptr::null()]).collect();
        self.check(unsafe { (self.api.command)(self.handle, ptrs.as_mut_ptr()) })
    }

    /// The headers of the next file. mpv has its own options for the user agent and the referrer.
    pub fn headers(&self, headers: &std::collections::HashMap<String, String>) -> Result<(), String> {
        let own = |name: &str| headers.iter().find(|(k, _)| k.eq_ignore_ascii_case(name)).map(|(_, v)| v.as_str());
        self.set("user-agent", own("User-Agent").unwrap_or("libmpv"))?;
        self.set("referrer", own("Referer").unwrap_or(""))?;
        self.command(&["change-list", "http-header-fields", "clr", ""])?;
        for (name, value) in headers.iter().filter(|(k, _)| !k.eq_ignore_ascii_case("User-Agent") && !k.eq_ignore_ascii_case("Referer")) {
            // `append` adds one item as is: a value may hold commas.
            self.command(&["change-list", "http-header-fields", "append", &format!("{name}: {value}")])?;
        }
        Ok(())
    }

    pub fn set(&self, name: &str, value: &str) -> Result<(), String> {
        self.check(unsafe { (self.api.set_property_string)(self.handle, c(name).as_ptr(), c(value).as_ptr()) })
    }

    fn check(&self, code: c_int) -> Result<(), String> {
        if code >= 0 {
            return Ok(());
        }
        Err(unsafe { CStr::from_ptr((self.api.error_string)(code)) }.to_string_lossy().into_owned())
    }

    /// Forwards mpv events to the webview as `event_name` events, until mpv shuts down. A tile's carry its index.
    pub fn pump(&self, app: &AppHandle, event_name: &str, tile: Option<usize>) {
        let mut last_time = f64::NEG_INFINITY;
        let mut last_cache = f64::NEG_INFINITY;
        let mut last_start = f64::NEG_INFINITY;
        loop {
            let event = unsafe { &*(self.api.wait_event)(self.handle, -1.0) };
            let payload = match event.event_id {
                1 => break,
                2 => {
                    let m = unsafe { &*(event.data as *const EventLogMessage) };
                    let text = unsafe { CStr::from_ptr(m.text) }.to_string_lossy();
                    let prefix = unsafe { CStr::from_ptr(m.prefix) }.to_string_lossy();
                    log::warn!("mpv [{prefix}] {}", text.trim_end());
                    continue;
                }
                7 => {
                    let end = unsafe { &*(event.data as *const EventEndFile) };
                    let reason = match end.reason {
                        0 => "eof",
                        2 => "stop",
                        3 => "quit",
                        4 => "error",
                        _ => "other",
                    };
                    let error = (end.reason == 4).then(|| unsafe { CStr::from_ptr((self.api.error_string)(end.error)) }.to_string_lossy().into_owned());
                    json!({ "event": "end-file", "reason": reason, "error": error })
                }
                8 => json!({ "event": "file-loaded" }),
                21 => json!({ "event": "playback-restart" }),
                #[cfg(target_os = "linux")]
                22 if event.reply_userdata == VID => {
                    if let Some(renderer) = &self.renderer {
                        renderer.track_changed(match property(event).1 {
                            Value::Null => None,
                            Value::String(track) => Some(track),
                            track => Some(track.to_string()),
                        });
                    }
                    continue;
                }
                22 => {
                    let (name, value) = property(event);
                    // time-pos changes every frame; a few updates a second are enough for a clock.
                    if name == "time-pos" {
                        let t = value.as_f64().unwrap_or(0.0);
                        if (t - last_time).abs() < 0.25 {
                            continue;
                        }
                        last_time = t;
                    }
                    if name == "demuxer-cache-time" {
                        let t = value.as_f64().unwrap_or(0.0);
                        if (t - last_cache).abs() < 1.0 {
                            continue;
                        }
                        last_cache = t;
                    }
                    if name == "demuxer-cache-state" {
                        let Some(start) = cache_start(&value) else { continue };
                        if (start - last_start).abs() < 1.0 {
                            continue;
                        }
                        last_start = start;
                        json!({ "event": "property", "name": "cache-start", "value": start })
                    } else {
                        json!({ "event": "property", "name": name, "value": value })
                    }
                }
                _ => continue,
            };
            let payload = match (tile, payload) {
                (Some(i), Value::Object(mut o)) => {
                    o.insert("tile".into(), json!(i));
                    Value::Object(o)
                }
                (_, p) => p,
            };
            if let Err(e) = app.emit(event_name, payload) {
                log::warn!("mpv event: {e}");
            }
        }
    }
}

impl Drop for Mpv {
    fn drop(&mut self) {
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        if let Some(renderer) = &self.renderer {
            renderer.free();
        }
        unsafe { (self.api.terminate_destroy)(self.handle) };
    }
}

fn property(event: &Event) -> (String, Value) {
    let p = unsafe { &*(event.data as *const EventProperty) };
    let name = unsafe { CStr::from_ptr(p.name) }.to_string_lossy().into_owned();
    // Format NONE means the property is unavailable (no file, no video…).
    let value = unsafe {
        match p.format {
            FORMAT_FLAG => json!(*(p.data as *const c_int) != 0),
            FORMAT_INT64 => json!(*(p.data as *const i64)),
            FORMAT_DOUBLE => json!(*(p.data as *const f64)),
            FORMAT_STRING => {
                let s = CStr::from_ptr(*(p.data as *const *const c_char)).to_string_lossy();
                serde_json::from_str(&s).unwrap_or_else(|_| json!(s))
            }
            _ => Value::Null,
        }
    };
    (name, value)
}

/// Where the seekable range that reaches furthest (the live edge, after a seek outside the cache) starts.
fn cache_start(state: &Value) -> Option<f64> {
    state["seekable-ranges"]
        .as_array()?
        .iter()
        .filter_map(|r| Some((r["start"].as_f64()?, r["end"].as_f64()?)))
        .max_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(start, _)| start)
}

/// A bare child window at the bottom of the main window's z-order, for mpv to draw into.
#[cfg(windows)]
pub fn video_host(parent: isize) -> isize {
    let class: Vec<u16> = "STATIC\0".encode_utf16().collect();
    unsafe {
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(parent as _, &mut r);
        let hwnd = CreateWindowExW(
            WS_EX_NOPARENTNOTIFY,
            class.as_ptr(),
            std::ptr::null(),
            WS_CHILD | WS_VISIBLE,
            0,
            0,
            r.right,
            r.bottom,
            parent as _,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null(),
        );
        SetWindowPos(hwnd, HWND_BOTTOM, 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        hwnd as isize
    }
}

/// A child window for a multi-view tile, at `rect` (x, y, width, height in physical pixels).
#[cfg(windows)]
pub fn tile_host(parent: isize, rect: [i32; 4]) -> isize {
    let host = video_host(parent);
    move_host(host, rect);
    host
}

#[cfg(windows)]
pub fn move_host(host: isize, [x, y, w, h]: [i32; 4]) {
    unsafe { SetWindowPos(host as _, std::ptr::null_mut(), x, y, w.max(1), h.max(1), SWP_NOZORDER | SWP_NOACTIVATE) };
}

#[cfg(windows)]
pub fn show_host(host: isize, visible: bool) {
    unsafe { ShowWindow(host as _, if visible { SW_SHOWNA } else { SW_HIDE }) };
}

#[cfg(windows)]
pub fn destroy_host(host: isize) {
    unsafe { DestroyWindow(host as _) };
}

/// Moves the video window into `parent` (the main window, or the PiP one), filling it, under its webview.
#[cfg(windows)]
pub fn adopt_host(host: isize, parent: isize) {
    unsafe {
        SetParent(host as _, parent as _);
        let mut r: RECT = std::mem::zeroed();
        GetClientRect(parent as _, &mut r);
        SetWindowPos(host as _, HWND_BOTTOM, 0, 0, r.right, r.bottom, SWP_NOACTIVATE);
    }
}

/// The window the video window is in now.
#[cfg(windows)]
pub fn host_parent(host: isize) -> isize {
    unsafe { GetParent(host as _) as isize }
}

/// Keeps the video window the size of its window's client area.
#[cfg(windows)]
pub fn resize_host(host: isize, width: u32, height: u32) {
    unsafe {
        SetWindowPos(host as _, std::ptr::null_mut(), 0, 0, width as i32, height as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_start_is_that_of_the_range_reaching_furthest() {
        let state = json!({ "seekable-ranges": [{ "start": 5.0, "end": 40.0 }, { "start": 90.0, "end": 120.0 }] });
        assert_eq!(cache_start(&state), Some(90.0));
        assert_eq!(cache_start(&json!({ "seekable-ranges": [] })), None);
        assert_eq!(cache_start(&json!(null)), None);
    }
}
