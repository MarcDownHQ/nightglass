//! The CDM library, one instance of it, and the host objects it calls back into.

use std::ffi::{CString, c_char, c_void};
use std::path::Path;
use std::ptr::null_mut;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::ffi::*;

/// What the CDM tells the host. Some calls must not reach back into the CDM before it returns: those are `deferred`.
pub trait Callbacks {
    fn set_timer(&self, delay_ms: i64, context: usize);
    fn initialized(&self, success: bool);
    fn new_session(&self, promise: u32, session: String);
    fn resolved(&self, promise: u32);
    fn rejected(&self, promise: u32, exception: u32, system_code: u32, message: String);
    fn message(&self, session: String, kind: u32, message: Vec<u8>);
    fn keys(&self, session: String, keys: Vec<KeyStatus>);
    fn expiration(&self, session: String, time: f64);
    fn closed(&self, session: String);
    fn deferred(&self, call: Deferred);
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeyStatus {
    pub kid: Vec<u8>,
    pub status: u32,
    pub system_code: u32,
}

/// Answers the CDM waits for, given once its current call has returned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Deferred {
    OutputProtection,
    PlatformChallenge,
    StorageId(u32),
}

/// `cdm::Host_10`. Lives as long as the instance that was given it.
#[repr(C)]
pub struct Host {
    vtable: *const Host10Vtable<Host>,
    callbacks: Box<dyn Callbacks>,
}

impl Host {
    pub fn new(callbacks: Box<dyn Callbacks>) -> Box<Host> {
        Box::new(Host { vtable: &HOST_VTABLE, callbacks })
    }
}

static HOST_VTABLE: Host10Vtable<Host> = Host10Vtable {
    allocate: host_allocate,
    set_timer: host_set_timer,
    get_current_wall_time: host_wall_time,
    on_initialized: host_initialized,
    on_resolve_key_status_promise: host_key_status,
    on_resolve_new_session_promise: host_new_session,
    on_resolve_promise: host_resolved,
    on_reject_promise: host_rejected,
    on_session_message: host_message,
    on_session_keys_change: host_keys,
    on_expiration_change: host_expiration,
    on_session_closed: host_closed,
    send_platform_challenge: host_platform_challenge,
    enable_output_protection: host_enable_output_protection,
    query_output_protection_status: host_query_output_protection,
    on_deferred_initialization_done: host_deferred_initialization_done,
    create_file_io: host_create_file_io,
    request_storage_id: host_request_storage_id,
    destructors: [never_destroyed; size_of::<Destructors>() / size_of::<usize>()],
};

fn callbacks<'a>(host: *mut Host) -> &'a dyn Callbacks {
    unsafe { &*(*host).callbacks }
}

fn text(data: *const c_char, size: u32) -> String {
    String::from_utf8_lossy(bytes(data.cast(), size)).into_owned()
}

fn bytes<'a>(data: *const u8, size: u32) -> &'a [u8] {
    if data.is_null() || size == 0 { &[] } else { unsafe { std::slice::from_raw_parts(data, size as usize) } }
}

extern "C" fn host_allocate(_: *mut Host, capacity: u32) -> *mut Buffer {
    HeapBuffer::allocate(capacity)
}

extern "C" fn host_set_timer(host: *mut Host, delay_ms: i64, context: *mut c_void) {
    callbacks(host).set_timer(delay_ms, context as usize);
}

extern "C" fn host_wall_time(_: *mut Host) -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_secs_f64())
}

extern "C" fn host_initialized(host: *mut Host, success: bool) {
    callbacks(host).initialized(success);
}

extern "C" fn host_key_status(host: *mut Host, promise: u32, _status: u32) {
    callbacks(host).resolved(promise);
}

extern "C" fn host_new_session(host: *mut Host, promise: u32, session: *const c_char, size: u32) {
    callbacks(host).new_session(promise, text(session, size));
}

extern "C" fn host_resolved(host: *mut Host, promise: u32) {
    callbacks(host).resolved(promise);
}

extern "C" fn host_rejected(host: *mut Host, promise: u32, exception: u32, system_code: u32, message: *const c_char, size: u32) {
    callbacks(host).rejected(promise, exception, system_code, text(message, size));
}

extern "C" fn host_message(host: *mut Host, session: *const c_char, session_size: u32, kind: u32, message: *const c_char, size: u32) {
    callbacks(host).message(text(session, session_size), kind, bytes(message.cast(), size).to_vec());
}

extern "C" fn host_keys(host: *mut Host, session: *const c_char, session_size: u32, _new_usable: bool, keys: *const KeyInformation, count: u32) {
    let keys = if keys.is_null() { &[][..] } else { unsafe { std::slice::from_raw_parts(keys, count as usize) } };
    let keys = keys.iter().map(|k| KeyStatus { kid: bytes(k.key_id, k.key_id_size).to_vec(), status: k.status, system_code: k.system_code }).collect();
    callbacks(host).keys(text(session, session_size), keys);
}

extern "C" fn host_expiration(host: *mut Host, session: *const c_char, size: u32, time: f64) {
    callbacks(host).expiration(text(session, size), time);
}

extern "C" fn host_closed(host: *mut Host, session: *const c_char, size: u32) {
    callbacks(host).closed(text(session, size));
}

extern "C" fn host_platform_challenge(host: *mut Host, _: *const c_char, _: u32, _: *const c_char, _: u32) {
    callbacks(host).deferred(Deferred::PlatformChallenge);
}

extern "C" fn host_enable_output_protection(_: *mut Host, _: u32) {}

extern "C" fn host_query_output_protection(host: *mut Host) {
    callbacks(host).deferred(Deferred::OutputProtection);
}

extern "C" fn host_deferred_initialization_done(_: *mut Host, _: u32, _: u32) {}

extern "C" fn host_create_file_io(_: *mut Host, _: *mut c_void) -> *mut c_void {
    null_mut()
}

extern "C" fn host_request_storage_id(host: *mut Host, version: u32) {
    callbacks(host).deferred(Deferred::StorageId(version));
}

/// `cdm::Buffer` on the Rust heap.
#[repr(C)]
struct HeapBuffer {
    base: Buffer,
    data: Vec<u8>,
    size: u32,
}

static BUFFER_VTABLE: BufferVtable = BufferVtable {
    destroy: buffer_destroy,
    capacity: buffer_capacity,
    data: buffer_data,
    set_size: buffer_set_size,
    size: buffer_size,
    destructors: [never_destroyed; size_of::<Destructors>() / size_of::<usize>()],
};

impl HeapBuffer {
    fn allocate(capacity: u32) -> *mut Buffer {
        let buffer = Box::new(HeapBuffer { base: Buffer { vtable: &BUFFER_VTABLE }, data: vec![0; capacity as usize], size: 0 });
        Box::into_raw(buffer).cast()
    }

    /// The bytes the CDM wrote, and the buffer given back.
    fn take(buffer: *mut Buffer) -> Vec<u8> {
        let buffer = unsafe { Box::from_raw(buffer.cast::<HeapBuffer>()) };
        let mut data = buffer.data;
        data.truncate(buffer.size as usize);
        data
    }
}

extern "C" fn buffer_destroy(buffer: *mut Buffer) {
    drop(unsafe { Box::from_raw(buffer.cast::<HeapBuffer>()) });
}

extern "C" fn buffer_capacity(buffer: *const Buffer) -> u32 {
    unsafe { (*buffer.cast::<HeapBuffer>()).data.len() as u32 }
}

extern "C" fn buffer_data(buffer: *mut Buffer) -> *mut u8 {
    unsafe { (*buffer.cast::<HeapBuffer>()).data.as_mut_ptr() }
}

extern "C" fn buffer_set_size(buffer: *mut Buffer, size: u32) {
    let buffer = unsafe { &mut *buffer.cast::<HeapBuffer>() };
    buffer.size = size.min(buffer.data.len() as u32);
}

extern "C" fn buffer_size(buffer: *const Buffer) -> u32 {
    unsafe { (*buffer.cast::<HeapBuffer>()).size }
}

/// `cdm::DecryptedBlock` for one `Decrypt` call.
#[repr(C)]
struct Block {
    base: DecryptedBlock,
    buffer: *mut Buffer,
    timestamp: i64,
}

static BLOCK_VTABLE: DecryptedBlockVtable = DecryptedBlockVtable {
    set_decrypted_buffer: block_set_buffer,
    decrypted_buffer: block_buffer,
    set_timestamp: block_set_timestamp,
    timestamp: block_timestamp,
    destructors: [never_destroyed; size_of::<Destructors>() / size_of::<usize>()],
};

extern "C" fn block_set_buffer(block: *mut DecryptedBlock, buffer: *mut Buffer) {
    let block = unsafe { &mut *block.cast::<Block>() };
    if !block.buffer.is_null() && block.buffer != buffer {
        drop(HeapBuffer::take(block.buffer));
    }
    block.buffer = buffer;
}

extern "C" fn block_buffer(block: *mut DecryptedBlock) -> *mut Buffer {
    unsafe { (*block.cast::<Block>()).buffer }
}

extern "C" fn block_set_timestamp(block: *mut DecryptedBlock, timestamp: i64) {
    unsafe { (*block.cast::<Block>()).timestamp = timestamp };
}

extern "C" fn block_timestamp(block: *const DecryptedBlock) -> i64 {
    unsafe { (*block.cast::<Block>()).timestamp }
}

/// The CDM library, loaded and initialised once for the process.
pub struct Module {
    _library: libloading::Library,
    create: CreateCdmInstanceFn,
    version: String,
}

impl Module {
    pub fn load(path: &Path) -> Result<Module, String> {
        #[cfg(windows)]
        if let Ok(dxva) = unsafe { libloading::Library::new("dxva2.dll") } {
            std::mem::forget(dxva);
        }
        let library = unsafe { libloading::Library::new(path) }.map_err(|e| format!("{}: {e}", path.display()))?;
        let symbol = |name: &str| format!("{name}: {}", path.display());
        unsafe {
            let initialize: InitializeCdmModuleFn = *library.get::<InitializeCdmModuleFn>(format!("InitializeCdmModule_{MODULE_VERSION}").as_bytes()).map_err(|_| symbol("InitializeCdmModule"))?;
            let create: CreateCdmInstanceFn = *library.get::<CreateCdmInstanceFn>(b"CreateCdmInstance").map_err(|_| symbol("CreateCdmInstance"))?;
            let get_version: GetCdmVersionFn = *library.get::<GetCdmVersionFn>(b"GetCdmVersion").map_err(|_| symbol("GetCdmVersion"))?;
            let version = get_version();
            let version = if version.is_null() { String::new() } else { std::ffi::CStr::from_ptr(version).to_string_lossy().into_owned() };
            initialize();
            Ok(Module { _library: library, create, version })
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// A CDM instance for Widevine, calling back into `host`, which must outlive it.
    pub fn create(&self, host: &mut Host) -> Result<Instance, String> {
        let key_system = CString::new(KEY_SYSTEM).expect("key system");
        let host: *mut Host = host;
        let cdm = unsafe { (self.create)(INTERFACE_VERSION, key_system.as_ptr(), KEY_SYSTEM.len() as u32, get_host, host.cast()) };
        if cdm.is_null() {
            return Err(format!("the CDM refused interface {INTERFACE_VERSION}"));
        }
        Ok(Instance(cdm.cast()))
    }
}

extern "C" fn get_host(version: i32, user_data: *mut c_void) -> *mut c_void {
    if version == HOST_VERSION { user_data } else { null_mut() }
}

/// `cdm::ContentDecryptionModule_10`. Never destroyed: a CDM destroyed with sessions open may crash, and the
/// helper's process ends with it anyway.
pub struct Instance(*mut Cdm10);

/// What one sample needs for `Decrypt`.
pub struct Encrypted<'a> {
    pub data: &'a [u8],
    pub scheme: u32,
    pub kid: &'a [u8],
    pub iv: &'a [u8],
    pub subsamples: &'a [SubsampleEntry],
    pub pattern: Pattern,
}

impl Instance {
    fn vtable(&self) -> &Cdm10Vtable {
        unsafe { &*(*self.0).vtable }
    }

    pub fn initialize(&self, distinctive_identifier: bool, persistent_state: bool) {
        unsafe { (self.vtable().initialize)(self.0, distinctive_identifier, persistent_state, false) }
    }

    pub fn set_server_certificate(&self, promise: u32, certificate: &[u8]) {
        unsafe { (self.vtable().set_server_certificate)(self.0, promise, certificate.as_ptr(), certificate.len() as u32) }
    }

    pub fn create_session(&self, promise: u32, pssh: &[u8]) {
        unsafe { (self.vtable().create_session_and_generate_request)(self.0, promise, SESSION_TEMPORARY, INIT_DATA_CENC, pssh.as_ptr(), pssh.len() as u32) }
    }

    pub fn update_session(&self, promise: u32, session: &str, response: &[u8]) {
        unsafe { (self.vtable().update_session)(self.0, promise, session.as_ptr().cast(), session.len() as u32, response.as_ptr(), response.len() as u32) }
    }

    pub fn close_session(&self, promise: u32, session: &str) {
        unsafe { (self.vtable().close_session)(self.0, promise, session.as_ptr().cast(), session.len() as u32) }
    }

    pub fn timer_expired(&self, context: usize) {
        unsafe { (self.vtable().timer_expired)(self.0, context as *mut c_void) }
    }

    pub fn answer(&self, call: Deferred) {
        unsafe {
            match call {
                Deferred::OutputProtection => (self.vtable().on_query_output_protection_status)(self.0, QUERY_FAILED, 0, 0),
                Deferred::PlatformChallenge => {
                    let empty = PlatformChallengeResponse {
                        signed_data: std::ptr::null(),
                        signed_data_length: 0,
                        signed_data_signature: std::ptr::null(),
                        signed_data_signature_length: 0,
                        platform_key_certificate: std::ptr::null(),
                        platform_key_certificate_length: 0,
                    };
                    (self.vtable().on_platform_challenge_response)(self.0, &empty)
                }
                Deferred::StorageId(version) => (self.vtable().on_storage_id)(self.0, version, std::ptr::null(), 0),
            }
        }
    }

    /// The clear sample, or the CDM's status.
    pub fn decrypt(&self, sample: &Encrypted) -> Result<Vec<u8>, u32> {
        let input = InputBuffer2 {
            data: sample.data.as_ptr(),
            data_size: sample.data.len() as u32,
            encryption_scheme: sample.scheme,
            key_id: sample.kid.as_ptr(),
            key_id_size: sample.kid.len() as u32,
            _pad0: 0,
            iv: sample.iv.as_ptr(),
            iv_size: sample.iv.len() as u32,
            _pad1: 0,
            subsamples: if sample.subsamples.is_empty() { std::ptr::null() } else { sample.subsamples.as_ptr() },
            num_subsamples: sample.subsamples.len() as u32,
            _pad2: 0,
            pattern: sample.pattern,
            timestamp: 0,
        };
        let mut block = Block { base: DecryptedBlock { vtable: &BLOCK_VTABLE }, buffer: null_mut(), timestamp: 0 };
        let status = unsafe { (self.vtable().decrypt)(self.0, &input, (&mut block as *mut Block).cast()) };
        let clear = (!block.buffer.is_null()).then(|| HeapBuffer::take(block.buffer));
        match (status, clear) {
            (status::SUCCESS, Some(clear)) => Ok(clear),
            (status::SUCCESS, None) => Err(status::DECRYPT_ERROR),
            (status, _) => Err(status),
        }
    }
}
