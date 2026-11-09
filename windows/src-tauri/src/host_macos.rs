//! macOS video windows: plain NSViews kept under the webview in the window's content view, for mpv to
//! draw into through an OpenGL layer (see `attach`). Same functions as the Win32 ones in `mpv.rs`; a host is
//! a retained NSView pointer. AppKit wants the main thread: each function runs there, waiting for it if called
//! from elsewhere, so callers must never hold the main thread while waiting on mpv (mpv commands are async).

// OpenGL is deprecated on macOS but still the only API libmpv's render API offers there.
#![allow(deprecated)]

use std::ffi::{c_char, c_int, c_void};
use std::ptr::NonNull;
use std::sync::{Arc, Mutex};

use dispatch2::run_on_main;
use objc2::rc::Retained;
use objc2::{AllocAnyThread, DefinedClass, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{NSAutoresizingMaskOptions, NSView, NSWindowOrderingMode};
use objc2_core_graphics::CGColor;
use objc2_core_video::CVTimeStamp;
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize};
use objc2_open_gl::{
    CGLChoosePixelFormat, CGLContextObj, CGLCreateContext, CGLDestroyPixelFormat, CGLPixelFormatObj, CGLReleaseContext, CGLRetainContext,
    CGLSetCurrentContext,
};
use objc2_quartz_core::{CALayer, CAOpenGLLayer};

fn view<'a>(ptr: isize) -> &'a NSView {
    unsafe { &*(ptr as *const NSView) }
}

/// A view filling `parent` (a window's content view), under its webview, following its size.
pub fn video_host(parent: isize) -> isize {
    run_on_main(|mtm| {
        let host = NSView::initWithFrame(NSView::alloc(mtm), view(parent).bounds());
        host.setAutoresizingMask(NSAutoresizingMaskOptions::ViewWidthSizable | NSAutoresizingMaskOptions::ViewHeightSizable);
        view(parent).addSubview_positioned_relativeTo(&host, NSWindowOrderingMode::Below, None);
        // Ours until `destroy_host`.
        Retained::into_raw(host) as isize
    })
}

/// A view for a multi-view tile, at `rect` (x, y, width, height in physical pixels, from the top left).
pub fn tile_host(parent: isize, rect: [i32; 4]) -> isize {
    let host = video_host(parent);
    run_on_main(|_| view(host).setAutoresizingMask(NSAutoresizingMaskOptions::ViewNotSizable));
    move_host(host, rect);
    host
}

pub fn move_host(host: isize, [x, y, w, h]: [i32; 4]) {
    run_on_main(|_| {
        let host = view(host);
        let (Some(parent), Some(window)) = (unsafe { host.superview() }, host.window()) else { return };
        let k = window.backingScaleFactor();
        // AppKit's origin is the bottom left.
        let height = parent.bounds().size.height;
        let (x, y, w, h) = (f64::from(x) / k, f64::from(y) / k, f64::from(w.max(1)) / k, f64::from(h.max(1)) / k);
        host.setFrame(NSRect::new(NSPoint::new(x, height - y - h), NSSize::new(w, h)));
    })
}

pub fn show_host(host: isize, visible: bool) {
    run_on_main(|_| view(host).setHidden(!visible));
}

pub fn destroy_host(host: isize) {
    run_on_main(|_| unsafe { Retained::from_raw(host as *mut NSView) }.expect("host view").removeFromSuperview());
}

/// Moves the view into `parent` (the main window's content view, or the PiP one's), filling it, under its webview.
pub fn adopt_host(host: isize, parent: isize) {
    run_on_main(|_| {
        let (host, parent) = (view(host), view(parent));
        host.setFrame(parent.bounds());
        parent.addSubview_positioned_relativeTo(host, NSWindowOrderingMode::Below, None);
        fit_scale(host);
    });
}

/// The content view the video view is in now.
pub fn host_parent(host: isize) -> isize {
    run_on_main(|_| unsafe { view(host).superview() }.map_or(0, |p| Retained::as_ptr(&p) as isize))
}

/// Nothing to do: the autoresizing mask keeps the view the size of its window.
pub fn resize_host(_host: isize, _width: u32, _height: u32) {}

// mpv ignores `wid` on macOS (it only embeds on X11, Windows and Android): it draws through its render
// API instead, into an OpenGL layer hosted by the video view.

/// The render API functions, loaded from libmpv with the rest.
pub struct RenderFns {
    pub create: unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *mut Param) -> c_int,
    pub render: unsafe extern "C" fn(*mut c_void, *mut Param) -> c_int,
    pub update: unsafe extern "C" fn(*mut c_void) -> u64,
    pub free: unsafe extern "C" fn(*mut c_void),
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
const UPDATE_FRAME: u64 = 1;
const GL_FRAMEBUFFER_BINDING: u32 = 0x8CA6;
const GL_VIEWPORT: u32 = 0x0BA2;

#[link(name = "OpenGL", kind = "framework")]
unsafe extern "C" {
    fn glGetIntegerv(pname: u32, data: *mut c_int);
}

unsafe extern "C" {
    fn dlsym(handle: *mut c_void, symbol: *const c_char) -> *mut c_void;
}

unsafe extern "C" fn get_proc_address(_: *mut c_void, name: *const c_char) -> *mut c_void {
    const RTLD_DEFAULT: *mut c_void = -2isize as *mut c_void;
    unsafe { dlsym(RTLD_DEFAULT, name) }
}

struct State {
    ctx: *mut c_void,
    cgl: CGLContextObj,
    freed: bool,
    /// The size last drawn at, in pixels: a layer resized loses its picture, so it must be drawn again.
    drawn: (c_int, c_int),
}

/// One mpv instance's drawing, in a GL context of its own that the layer draws with. Made with the instance, before
/// any file plays (mpv's video output fails without it); freed before mpv is destroyed.
pub struct Renderer {
    fns: RenderFns,
    state: Mutex<State>,
}

// The mutex guards the render context; libmpv's render API may be used from any one thread at a time.
unsafe impl Send for Renderer {}
unsafe impl Sync for Renderer {}

impl Renderer {
    fn new(mpv: *mut c_void, fns: RenderFns) -> Result<Self, String> {
        let pf = pixel_format();
        let mut cgl = std::ptr::null_mut();
        unsafe {
            CGLCreateContext(pf, std::ptr::null_mut(), NonNull::from(&mut cgl));
            CGLDestroyPixelFormat(pf);
        }
        if cgl.is_null() {
            return Err("No OpenGL context".into());
        }
        let mut init = InitParams { get_proc_address, ctx: std::ptr::null_mut() };
        let mut params = [
            Param { kind: PARAM_API_TYPE, data: c"opengl".as_ptr() as *mut c_void },
            Param { kind: PARAM_OPENGL_INIT_PARAMS, data: (&raw mut init).cast() },
            Param { kind: 0, data: std::ptr::null_mut() },
        ];
        let mut ctx = std::ptr::null_mut();
        let code = unsafe {
            CGLSetCurrentContext(cgl);
            let code = (fns.create)(&mut ctx, mpv, params.as_mut_ptr());
            CGLSetCurrentContext(std::ptr::null_mut());
            code
        };
        if code < 0 {
            unsafe { CGLReleaseContext(cgl) };
            return Err(format!("mpv_render_context_create: {code}"));
        }
        Ok(Renderer { fns, state: Mutex::new(State { ctx, cgl, freed: false, drawn: (0, 0) }) })
    }

    fn wants_frame(&self, size: (c_int, c_int)) -> bool {
        let Ok(state) = self.state.lock() else { return false };
        // mpv's update first: it must be read each time, or its next frames are not announced.
        !state.freed && (unsafe { (self.fns.update)(state.ctx) } & UPDATE_FRAME != 0 || state.drawn != size)
    }

    /// In the layer's draw, with our context current (see `copyCGLContextForPixelFormat:`).
    fn draw(&self) {
        let Ok(mut state) = self.state.lock() else { return };
        if state.freed {
            return;
        }
        let (mut fbo, mut viewport) = (0, [0; 4]);
        unsafe {
            glGetIntegerv(GL_FRAMEBUFFER_BINDING, &mut fbo);
            glGetIntegerv(GL_VIEWPORT, viewport.as_mut_ptr());
        }
        state.drawn = (viewport[2], viewport[3]);
        let mut target = Fbo { fbo, w: viewport[2], h: viewport[3], internal_format: 0 };
        let mut flip: c_int = 1;
        let mut params = [
            Param { kind: PARAM_OPENGL_FBO, data: (&raw mut target).cast() },
            Param { kind: PARAM_FLIP_Y, data: (&raw mut flip).cast() },
            Param { kind: 0, data: std::ptr::null_mut() },
        ];
        unsafe { (self.fns.render)(state.ctx, params.as_mut_ptr()) };
    }

    /// Must come before `mpv_terminate_destroy`, which waits for it.
    pub fn free(&self) {
        let Ok(mut state) = self.state.lock() else { return };
        if state.freed {
            return;
        }
        unsafe {
            CGLSetCurrentContext(state.cgl);
            (self.fns.free)(state.ctx);
            CGLSetCurrentContext(std::ptr::null_mut());
            // The layer keeps its own reference until it goes.
            CGLReleaseContext(state.cgl);
        }
        state.freed = true;
    }
}

define_class!(
    #[unsafe(super(CAOpenGLLayer, CALayer, NSObject))]
    #[name = "NightglassVideoLayer"]
    #[ivars = Arc<Renderer>]
    struct VideoLayer;

    impl VideoLayer {
        #[unsafe(method(canDrawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn can_draw(&self, _ctx: CGLContextObj, _pf: CGLPixelFormatObj, _t: f64, _ts: *const CVTimeStamp) -> bool {
            let (size, k) = (self.bounds().size, self.contentsScale());
            self.ivars().wants_frame(((size.width * k).round() as c_int, (size.height * k).round() as c_int))
        }

        #[unsafe(method(drawInCGLContext:pixelFormat:forLayerTime:displayTime:))]
        fn draw_in(&self, ctx: CGLContextObj, pf: CGLPixelFormatObj, t: f64, ts: *const CVTimeStamp) {
            self.ivars().draw();
            // Flushes the frame.
            let _: () = unsafe { msg_send![super(self), drawInCGLContext: ctx, pixelFormat: pf, forLayerTime: t, displayTime: ts] };
        }

        #[unsafe(method(copyCGLPixelFormatForDisplayMask:))]
        fn copy_pixel_format(&self, _mask: u32) -> CGLPixelFormatObj {
            pixel_format()
        }

        /// The renderer's own context, which mpv's render context lives in.
        #[unsafe(method(copyCGLContextForPixelFormat:))]
        fn copy_context(&self, _pf: CGLPixelFormatObj) -> CGLContextObj {
            let cgl = self.ivars().state.lock().map_or(std::ptr::null_mut(), |s| s.cgl);
            unsafe { CGLRetainContext(cgl) }
        }
    }
);

/// Hosts mpv's drawing in `host`'s layer; the renderer is mpv's to free.
pub fn attach(host: isize, mpv: *mut c_void, fns: RenderFns) -> Result<Arc<Renderer>, String> {
    let renderer = Arc::new(Renderer::new(mpv, fns)?);
    let shared = renderer.clone();
    run_on_main(move |_| {
        let layer: Retained<VideoLayer> = unsafe { msg_send![super(VideoLayer::alloc().set_ivars(shared)), init] };
        // Polled each display refresh; draws only when mpv has a new frame.
        layer.setAsynchronous(true);
        layer.setNeedsDisplayOnBoundsChange(true);
        // Black until mpv's first frame, like the video window on Windows: never the desktop through the page's hole.
        layer.setBackgroundColor(Some(&CGColor::new_srgb(0.0, 0.0, 0.0, 1.0)));
        let host = view(host);
        host.setLayer(Some(&layer));
        host.setWantsLayer(true);
        fit_scale(host);
    });
    Ok(renderer)
}

/// A 3.2 core profile: mpv's renderer wants more than the legacy 2.1 one.
fn pixel_format() -> CGLPixelFormatObj {
    let attributes: [u32; 8] = [99, 0x3200, 73, 5, 8, 24, 11, 0]; // profile 3.2 core, accelerated, double buffer, 24-bit colour
    let (mut pf, mut count) = (std::ptr::null_mut(), 0);
    unsafe { CGLChoosePixelFormat(NonNull::from(&attributes).cast(), NonNull::from(&mut pf), NonNull::from(&mut count)) };
    pf
}

/// Draw at the screen's resolution (Retina or not).
fn fit_scale(host: &NSView) {
    if let (Some(layer), Some(window)) = (host.layer(), host.window()) {
        layer.setContentsScale(window.backingScaleFactor());
    }
}
