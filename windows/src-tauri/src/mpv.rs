//! libmpv, loaded at runtime from libmpv-2.dll. mpv draws into a child window kept under the
//! transparent webview, so the UI can sit on top of the video.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::path::Path;

use libloading::Library;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter};
use windows_sys::Win32::Foundation::RECT;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, GetClientRect, HWND_BOTTOM, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
    SetWindowPos, WS_CHILD, WS_EX_NOPARENTNOTIFY, WS_VISIBLE,
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

/// Properties the UI follows, with the format mpv reports them in.
const OBSERVED: &[(&str, c_int)] = &[
    ("pause", FORMAT_FLAG),
    ("paused-for-cache", FORMAT_FLAG),
    ("time-pos", FORMAT_DOUBLE),
    ("duration", FORMAT_DOUBLE),
    ("width", FORMAT_INT64),
    ("height", FORMAT_INT64),
    // Node properties read as a string come back as JSON.
    ("track-list", FORMAT_STRING),
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
            _lib: lib,
        })
    }
}

fn c(s: &str) -> CString {
    CString::new(s).unwrap_or_default()
}

/// One mpv instance for the whole app. It lives until the process exits.
pub struct Mpv {
    api: Api,
    handle: Handle,
}

// The libmpv client API is thread-safe.
unsafe impl Send for Mpv {}
unsafe impl Sync for Mpv {}

impl Mpv {
    pub fn new(dll: &Path, wid: isize) -> Result<Self, String> {
        let api = Api::load(dll)?;
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("mpv_create failed".into());
        }
        let mpv = Mpv { api, handle };
        let wid = wid.to_string();
        let options = [
            ("wid", wid.as_str()),
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
            ("vo", "gpu-next"),
            ("hwdec", "auto-safe"),
            ("background-color", "#000000"),
            ("network-timeout", "20"),
        ];
        for (name, value) in options {
            if let Err(e) = mpv.check(unsafe { (mpv.api.set_option_string)(handle, c(name).as_ptr(), c(value).as_ptr()) }) {
                log::warn!("mpv option {name}={value}: {e}");
            }
        }
        mpv.check(unsafe { (mpv.api.initialize)(handle) })?;
        unsafe { (mpv.api.request_log_messages)(handle, c("warn").as_ptr()) };
        for (i, (name, format)) in OBSERVED.iter().enumerate() {
            unsafe { (mpv.api.observe_property)(handle, i as u64, c(name).as_ptr(), *format) };
        }
        Ok(mpv)
    }

    pub fn command(&self, args: &[&str]) -> Result<(), String> {
        let owned: Vec<CString> = args.iter().map(|a| c(a)).collect();
        let mut ptrs: Vec<*const c_char> = owned.iter().map(|a| a.as_ptr()).chain([std::ptr::null()]).collect();
        self.check(unsafe { (self.api.command)(self.handle, ptrs.as_mut_ptr()) })
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

    /// Forwards mpv events to the webview as `mpv` events, until mpv shuts down.
    pub fn pump(&self, app: &AppHandle) {
        let mut last_time = f64::NEG_INFINITY;
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
                    json!({ "event": "property", "name": name, "value": value })
                }
                _ => continue,
            };
            if let Err(e) = app.emit("mpv", payload) {
                log::warn!("mpv event: {e}");
            }
        }
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

/// A bare child window at the bottom of the main window's z-order, for mpv to draw into.
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

/// Keeps the video window the size of the main window's client area.
pub fn resize_host(host: isize, width: u32, height: u32) {
    unsafe {
        SetWindowPos(host as _, std::ptr::null_mut(), 0, 0, width as i32, height as i32, SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE);
    }
}
