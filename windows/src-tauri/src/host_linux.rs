//! Linux video windows: GtkGLAreas in a GtkFixed laid under the webview (a GtkOverlay replaces the window's
//! webview box), mpv drawing into them through its render API. Same functions as the Win32 ones in `mpv.rs`;
//! a host is a GtkGLArea pointer holding its own reference, a parent the GtkFixed of a window. GTK wants the
//! main thread: each function runs there, waiting for it if called from elsewhere.

use std::ffi::{CStr, CString, c_char, c_int, c_void};
use std::sync::{Arc, Mutex, OnceLock, mpsc};

use gtk::glib::translate::{FromGlibPtrFull, FromGlibPtrNone, ToGlibPtr};
use gtk::glib::{self, ObjectType};
use gtk::prelude::*;
use libloading::Library;

fn on_main<R: Send + 'static>(f: impl FnOnce() -> R + Send + 'static) -> R {
    if gtk::is_initialized_main_thread() {
        return f();
    }
    let (tx, rx) = mpsc::channel();
    glib::MainContext::default().invoke(move || {
        let _ = tx.send(f());
    });
    rx.recv().expect("GTK main loop gone")
}

fn area(ptr: isize) -> gtk::GLArea {
    unsafe { gtk::GLArea::from_glib_none(ptr as *mut gtk::ffi::GtkGLArea) }
}

fn fixed(ptr: isize) -> gtk::Fixed {
    unsafe { gtk::Fixed::from_glib_none(ptr as *mut gtk::ffi::GtkFixed) }
}

/// The video layer of the window whose webview sits in `webview_box`, made on first use; none once the window is gone.
pub fn video_layer(webview_box: &gtk::Box) -> Option<isize> {
    let webview_box = webview_box.as_ptr() as isize;
    on_main(move || {
        let webview_box = unsafe { gtk::Box::from_glib_none(webview_box as *mut gtk::ffi::GtkBox) };
        let parent = webview_box.parent()?.downcast::<gtk::Container>().ok()?;
        if let Ok(overlay) = parent.clone().downcast::<gtk::Overlay>()
            && let Some(layer) = overlay.children().into_iter().find_map(|c| c.downcast::<gtk::Fixed>().ok())
        {
            return Some(layer.as_ptr() as isize);
        }
        parent.remove(&webview_box);
        let overlay = gtk::Overlay::new();
        let layer = gtk::Fixed::new();
        // The overlay takes its size from this empty box, not from the layer: a tile placed past the window's edge
        // would otherwise grow the window.
        overlay.add(&gtk::Box::new(gtk::Orientation::Vertical, 0));
        overlay.add_overlay(&layer);
        overlay.add_overlay(&webview_box);
        parent.add(&overlay);
        overlay.show_all();
        Some(layer.as_ptr() as isize)
    })
}

/// The size of the window `layer` is in, in GTK's units, known before it is first allocated.
fn window_size(layer: &gtk::Fixed) -> (i32, i32) {
    layer.toplevel().and_then(|w| w.downcast::<gtk::Window>().ok()).map_or((1, 1), |w| w.size())
}

/// An area filling `parent` (a window's video layer), under its webview.
pub fn video_host(parent: isize) -> isize {
    on_main(move || {
        let layer = fixed(parent);
        let host = gtk::GLArea::new();
        host.set_auto_render(false);
        let (w, h) = window_size(&layer);
        host.set_size_request(w, h);
        layer.put(&host, 0, 0);
        host.show();
        // Ours until `destroy_host`.
        ToGlibPtr::<*mut gtk::ffi::GtkGLArea>::to_glib_full(&host) as isize
    })
}

/// An area for a multi-view tile, at `rect` (x, y, width, height in physical pixels, from the top left).
pub fn tile_host(parent: isize, rect: [i32; 4]) -> isize {
    let host = video_host(parent);
    move_host(host, rect);
    host
}

pub fn move_host(host: isize, [x, y, w, h]: [i32; 4]) {
    on_main(move || {
        let host = area(host);
        let Some(layer) = host.parent().and_then(|p| p.downcast::<gtk::Fixed>().ok()) else { return };
        let k = host.scale_factor().max(1);
        layer.move_(&host, x / k, y / k);
        host.set_size_request((w / k).max(1), (h / k).max(1));
    });
}

pub fn show_host(host: isize, visible: bool) {
    on_main(move || area(host).set_visible(visible));
}

pub fn destroy_host(host: isize) {
    on_main(move || {
        let host = unsafe { gtk::GLArea::from_glib_full(host as *mut gtk::ffi::GtkGLArea) };
        if let Some(layer) = host.parent().and_then(|p| p.downcast::<gtk::Container>().ok()) {
            layer.remove(&host);
        }
    });
}

/// Moves the area into `parent` (the main window's video layer, or the PiP one's), filling it, under its webview.
pub fn adopt_host(host: isize, parent: isize) {
    on_main(move || {
        let (host, layer) = (area(host), fixed(parent));
        if let Some(old) = host.parent().and_then(|p| p.downcast::<gtk::Container>().ok()) {
            old.remove(&host);
        }
        let (w, h) = window_size(&layer);
        host.set_size_request(w, h);
        layer.put(&host, 0, 0);
        host.show();
        host.realize();
    });
}

/// The video layer the area is in now.
pub fn host_parent(host: isize) -> isize {
    on_main(move || area(host).parent().map_or(0, |p| p.as_ptr() as isize))
}

/// Keeps the area the size of its window (`width` and `height` in physical pixels).
pub fn resize_host(host: isize, width: u32, height: u32) {
    on_main(move || {
        let host = area(host);
        let k = host.scale_factor().max(1) as u32;
        host.set_size_request((width / k).max(1) as i32, (height / k).max(1) as i32);
    });
}

// mpv only embeds through `wid` on X11, and drawing in its own X window would cover the webview: it draws
// through its render API instead, into the area's GL context.

/// The render API functions, loaded from libmpv with the rest.
pub struct RenderFns {
    pub create: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut Param) -> c_int,
    pub render: unsafe extern "C" fn(*mut c_void, *mut Param) -> c_int,
    pub update: unsafe extern "C" fn(*mut c_void) -> u64,
    pub free: unsafe extern "C" fn(*mut c_void),
    pub get_property_string: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_char,
    pub set_property_string: unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int,
    pub mpv_free: unsafe extern "C" fn(*mut c_void),
}

#[repr(C)]
pub struct Param {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct InitParams {
    get_proc_address: unsafe extern "C" fn(*mut c_void, *const c_char) -> *mut c_void,
    ctx: *mut c_void,
}

#[repr(C)]
struct Fbo {
    fbo: c_int,
    w: c_int,
    h: c_int,
    internal_format: c_int,
}

const PARAM_API_TYPE: c_int = 1;
const PARAM_OPENGL_INIT_PARAMS: c_int = 2;
const PARAM_OPENGL_FBO: c_int = 3;
const PARAM_FLIP_Y: c_int = 4;
const PARAM_X11_DISPLAY: c_int = 8;
const PARAM_WL_DISPLAY: c_int = 9;
const PARAM_BLOCK_FOR_TARGET_TIME: c_int = 12;
const UPDATE_FRAME: u64 = 1;
const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_COLOR_BUFFER_BIT: u32 = 0x4000;

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
    fn setlocale(category: c_int, locale: *const c_char) -> *mut c_char;
}

/// mpv refuses to start under a locale whose numbers are not C's, and GTK sets the user's.
pub fn numeric_locale() {
    const LC_NUMERIC: c_int = 1;
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| unsafe {
        setlocale(LC_NUMERIC, c"C".as_ptr());
    });
}

type GetProc = unsafe extern "C" fn(*const c_char) -> *mut c_void;

/// GTK draws with EGL on Wayland and GLX on X11: the loader of whichever owns the current context.
fn loader() -> Option<GetProc> {
    static LOADER: OnceLock<Option<(Library, GetProc)>> = OnceLock::new();
    let found = LOADER.get_or_init(|| unsafe {
        if let Ok(egl) = Library::new("libEGL.so.1")
            && let Ok(current) = egl.get::<unsafe extern "C" fn() -> *mut c_void>(b"eglGetCurrentContext\0")
            && !current().is_null()
            && let Ok(get) = egl.get::<GetProc>(b"eglGetProcAddress\0")
        {
            let get = *get;
            return Some((egl, get));
        }
        let glx = Library::new("libGL.so.1").ok()?;
        let get = *glx.get::<GetProc>(b"glXGetProcAddressARB\0").ok()?;
        Some((glx, get))
    });
    found.as_ref().map(|(_, get)| *get)
}

unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    loader().map_or(std::ptr::null_mut(), |get| unsafe { get(name) })
}

fn gl<T: Copy>(name: &CStr) -> Option<T> {
    let ptr = unsafe { get_proc_address(std::ptr::null_mut(), name.as_ptr()) };
    (!ptr.is_null()).then(|| unsafe { std::mem::transmute_copy(&ptr) })
}

/// The display connection mpv needs for hardware decoding into GL (VA-API), if GDK has one it knows.
fn display_param(widget: &gtk::GLArea) -> Option<(c_int, *mut c_void)> {
    let display = WidgetExt::display(widget);
    let (kind, getter) = match display.type_().name() {
        "GdkWaylandDisplay" => (PARAM_WL_DISPLAY, c"gdk_wayland_display_get_wl_display"),
        "GdkX11Display" => (PARAM_X11_DISPLAY, c"gdk_x11_display_get_xdisplay"),
        _ => return None,
    };
    let getter = unsafe { dlsym(std::ptr::null_mut(), getter.as_ptr()) };
    if getter.is_null() {
        return None;
    }
    let getter: unsafe extern "C" fn(*mut c_void) -> *mut c_void = unsafe { std::mem::transmute(getter) };
    Some((kind, unsafe { getter(display.as_ptr().cast()) }))
}

struct State {
    ctx: *mut c_void,
    /// mpv is gone or going: never make a context again.
    dead: bool,
    /// The video track while the area moves window (its GL context goes with the old one): mpv drops video
    /// when its render context is freed, and gets it back once the new one is made.
    track: Option<String>,
}

/// One mpv instance's drawing into its area. The render context lives in the area's GL context, so it is freed
/// when the area leaves a window and made again in the next one; it is first made with the instance, before any
/// file plays (mpv's video output fails without it).
pub struct Renderer {
    fns: RenderFns,
    mpv: *mut c_void,
    area: isize,
    state: Mutex<State>,
}

// The mutex guards the render context; it is only made, drawn and freed on the main thread.
unsafe impl Send for Renderer {}
unsafe impl Sync for Renderer {}

impl Renderer {
    /// In the area's realize, its context current.
    fn create(&self, area: &gtk::GLArea) -> Result<(), String> {
        let Ok(mut state) = self.state.lock() else { return Ok(()) };
        if state.dead || !state.ctx.is_null() {
            return Ok(());
        }
        if let Some(e) = area.error() {
            return Err(format!("No OpenGL context: {e}"));
        }
        let mut init = InitParams { get_proc_address, ctx: std::ptr::null_mut() };
        let mut block: c_int = 0;
        let mut params = vec![
            Param { kind: PARAM_API_TYPE, data: c"opengl".as_ptr() as *mut c_void },
            Param { kind: PARAM_OPENGL_INIT_PARAMS, data: (&raw mut init).cast() },
            // Drawing happens on GTK's main thread: never wait there for a frame's time.
            Param { kind: PARAM_BLOCK_FOR_TARGET_TIME, data: (&raw mut block).cast() },
        ];
        if let Some((kind, data)) = display_param(area) {
            params.push(Param { kind, data });
        }
        params.push(Param { kind: 0, data: std::ptr::null_mut() });
        let mut ctx = std::ptr::null_mut();
        let code = unsafe { (self.fns.create)(&mut ctx, self.mpv, params.as_mut_ptr()) };
        if code < 0 {
            return Err(format!("mpv_render_context_create: {code}"));
        }
        state.ctx = ctx;
        let track = state.track.take();
        drop(state);
        if let Some(track) = track.filter(|t| t != "no") {
            self.set("vid", "no");
            self.set("vid", &track);
        }
        Ok(())
    }

    fn set(&self, name: &str, value: &str) {
        let (name, value) = (CString::new(name).unwrap_or_default(), CString::new(value).unwrap_or_default());
        unsafe { (self.fns.set_property_string)(self.mpv, name.as_ptr(), value.as_ptr()) };
    }

    fn track(&self) -> Option<String> {
        let ptr = unsafe { (self.fns.get_property_string)(self.mpv, c"vid".as_ptr()) };
        if ptr.is_null() {
            return None;
        }
        let track = unsafe { CStr::from_ptr(ptr) }.to_string_lossy().into_owned();
        unsafe { (self.fns.mpv_free)(ptr.cast()) };
        Some(track)
    }

    /// In the area's unrealize, or before mpv goes (`dead`), with the area's context current.
    fn release(&self, dead: bool) {
        let Ok(mut state) = self.state.lock() else { return };
        state.dead |= dead;
        if state.ctx.is_null() {
            return;
        }
        if !state.dead {
            state.track = self.track();
        }
        unsafe { (self.fns.free)(state.ctx) };
        state.ctx = std::ptr::null_mut();
    }

    fn wants_frame(&self) -> bool {
        let Ok(state) = self.state.lock() else { return false };
        // mpv's update must be read each time, or its next frames are not announced.
        !state.ctx.is_null() && unsafe { (self.fns.update)(state.ctx) } & UPDATE_FRAME != 0
    }

    /// In the area's render, its framebuffer bound.
    fn draw(&self, area: &gtk::GLArea) {
        let Ok(state) = self.state.lock() else { return };
        if state.ctx.is_null() {
            // Black until mpv's first frame: never the desktop through the page's hole.
            if let (Some(color), Some(clear)) = (gl::<unsafe extern "C" fn(f32, f32, f32, f32)>(c"glClearColor"), gl::<unsafe extern "C" fn(u32)>(c"glClear")) {
                unsafe {
                    color(0.0, 0.0, 0.0, 1.0);
                    clear(GL_COLOR_BUFFER_BIT);
                }
            }
            return;
        }
        let Some(get) = gl::<unsafe extern "C" fn(u32, *mut c_int)>(c"glGetIntegerv") else { return };
        let mut fbo = 0;
        unsafe { get(GL_FRAMEBUFFER_BINDING, &mut fbo) };
        let k = area.scale_factor();
        let mut target = Fbo { fbo, w: area.allocated_width() * k, h: area.allocated_height() * k, internal_format: 0 };
        let mut flip: c_int = 1;
        let mut params = [
            Param { kind: PARAM_OPENGL_FBO, data: (&raw mut target).cast() },
            Param { kind: PARAM_FLIP_Y, data: (&raw mut flip).cast() },
            Param { kind: 0, data: std::ptr::null_mut() },
        ];
        unsafe { (self.fns.render)(state.ctx, params.as_mut_ptr()) };
    }

    /// Must come before `mpv_terminate_destroy`, which waits for it.
    pub fn free(self: &Arc<Self>) {
        let renderer = self.clone();
        on_main(move || {
            let area = area(renderer.area);
            if area.is_realized() {
                area.make_current();
            }
            renderer.release(true);
        });
    }
}

/// Hosts mpv's drawing in the `host` area; the renderer is mpv's to free.
pub fn attach(host: isize, mpv: *mut c_void, fns: RenderFns) -> Result<Arc<Renderer>, String> {
    let renderer = Arc::new(Renderer { fns, mpv, area: host, state: Mutex::new(State { ctx: std::ptr::null_mut(), dead: false, track: None }) });
    let shared = renderer.clone();
    on_main(move || {
        let area = area(host);
        let r = shared.clone();
        area.connect_realize(move |area| {
            area.make_current();
            if let Err(e) = r.create(area) {
                log::error!("{e}");
            }
        });
        let r = shared.clone();
        area.connect_unrealize(move |area| {
            area.make_current();
            r.release(false);
        });
        let r = shared.clone();
        area.connect_render(move |area, _| {
            r.draw(area);
            glib::Propagation::Stop
        });
        // Polled each frame of the window; draws only when mpv has a new picture.
        let r = shared.clone();
        area.add_tick_callback(move |area, _| {
            if r.wants_frame() {
                area.queue_render();
            }
            glib::ControlFlow::Continue
        });
        area.realize();
        if area.is_realized() && shared.state.lock().is_ok_and(|s| s.ctx.is_null()) {
            area.make_current();
            shared.create(&area)?;
        }
        Ok::<(), String>(())
    })?;
    Ok(renderer)
}
