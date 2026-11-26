//! The parts of Chromium's CDM interface (`media/cdm/api/content_decryption_module.h`) a decrypt-only host uses:
//! CDM interface 10, host interface 10, module version 4.
//!
//! Ported from that header, Copyright (c) 2012 The Chromium Authors, under the BSD-3-Clause licence
//! reproduced in `licenses/chromium-cdm.txt`.
//!
//! The C++ classes are reached through their vtables. Their virtual destructors come last, after every method:
//! one slot with MSVC, two with the Itanium ABI. The CDM never calls them on the host's objects.

#![allow(dead_code)]

use std::ffi::{c_char, c_void};

pub const MODULE_VERSION: u32 = 4;
pub const INTERFACE_VERSION: i32 = 10;
pub const HOST_VERSION: i32 = 10;
pub const KEY_SYSTEM: &str = "com.widevine.alpha";

pub type GetCdmHostFunc = extern "C" fn(host_interface_version: i32, user_data: *mut c_void) -> *mut c_void;
pub type InitializeCdmModuleFn = unsafe extern "C" fn();
pub type DeinitializeCdmModuleFn = unsafe extern "C" fn();
pub type CreateCdmInstanceFn = unsafe extern "C" fn(i32, *const c_char, u32, GetCdmHostFunc, *mut c_void) -> *mut c_void;
pub type GetCdmVersionFn = unsafe extern "C" fn() -> *const c_char;

/// `cdm::Status`. Plain integers: a CDM may answer values a Rust enum would not allow.
pub mod status {
    pub const SUCCESS: u32 = 0;
    pub const NEED_MORE_DATA: u32 = 1;
    pub const NO_KEY: u32 = 2;
    pub const INITIALIZATION_ERROR: u32 = 3;
    pub const DECRYPT_ERROR: u32 = 4;
    pub const DECODE_ERROR: u32 = 5;
    pub const DEFERRED_INITIALIZATION: u32 = 6;
}

/// `cdm::EncryptionScheme`.
pub mod scheme {
    pub const UNENCRYPTED: u32 = 0;
    pub const CENC: u32 = 1;
    pub const CBCS: u32 = 2;
}

/// `cdm::InitDataType`.
pub const INIT_DATA_CENC: u32 = 0;
/// `cdm::SessionType`.
pub const SESSION_TEMPORARY: u32 = 0;
/// `cdm::QueryResult`.
pub const QUERY_SUCCEEDED: u32 = 0;
pub const QUERY_FAILED: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct Pattern {
    pub crypt_byte_block: u32,
    pub skip_byte_block: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SubsampleEntry {
    pub clear_bytes: u32,
    pub cipher_bytes: u32,
}

#[repr(C)]
pub struct InputBuffer2 {
    pub data: *const u8,
    pub data_size: u32,
    pub encryption_scheme: u32,
    pub key_id: *const u8,
    pub key_id_size: u32,
    pub _pad0: u32,
    pub iv: *const u8,
    pub iv_size: u32,
    pub _pad1: u32,
    pub subsamples: *const SubsampleEntry,
    pub num_subsamples: u32,
    pub _pad2: u32,
    pub pattern: Pattern,
    pub timestamp: i64,
}

#[repr(C)]
pub struct KeyInformation {
    pub key_id: *const u8,
    pub key_id_size: u32,
    pub status: u32,
    pub system_code: u32,
}

#[repr(C)]
pub struct Policy {
    pub min_hdcp_version: u32,
}

#[repr(C)]
pub struct PlatformChallengeResponse {
    pub signed_data: *const u8,
    pub signed_data_length: u32,
    pub signed_data_signature: *const u8,
    pub signed_data_signature_length: u32,
    pub platform_key_certificate: *const u8,
    pub platform_key_certificate_length: u32,
}

const _: () = {
    let pointer = size_of::<*const u8>();
    assert!(size_of::<InputBuffer2>() == if pointer == 8 { 80 } else { 64 });
    assert!(size_of::<KeyInformation>() == if pointer == 8 { 24 } else { 16 });
    assert!(size_of::<PlatformChallengeResponse>() == if pointer == 8 { 48 } else { 24 });
    assert!(size_of::<Pattern>() == 8 && size_of::<SubsampleEntry>() == 8 && size_of::<Policy>() == 4);
};

#[cfg(windows)]
pub type Destructors = [unsafe extern "C" fn(*mut c_void); 1];
#[cfg(not(windows))]
pub type Destructors = [unsafe extern "C" fn(*mut c_void); 2];

/// `cdm::ContentDecryptionModule_10`.
#[repr(C)]
pub struct Cdm10 {
    pub vtable: *const Cdm10Vtable,
}

#[repr(C)]
pub struct Cdm10Vtable {
    pub initialize: unsafe extern "C" fn(*mut Cdm10, bool, bool, bool),
    pub get_status_for_policy: unsafe extern "C" fn(*mut Cdm10, u32, *const Policy),
    pub set_server_certificate: unsafe extern "C" fn(*mut Cdm10, u32, *const u8, u32),
    pub create_session_and_generate_request: unsafe extern "C" fn(*mut Cdm10, u32, u32, u32, *const u8, u32),
    pub load_session: unsafe extern "C" fn(*mut Cdm10, u32, u32, *const c_char, u32),
    pub update_session: unsafe extern "C" fn(*mut Cdm10, u32, *const c_char, u32, *const u8, u32),
    pub close_session: unsafe extern "C" fn(*mut Cdm10, u32, *const c_char, u32),
    pub remove_session: unsafe extern "C" fn(*mut Cdm10, u32, *const c_char, u32),
    pub timer_expired: unsafe extern "C" fn(*mut Cdm10, *mut c_void),
    pub decrypt: unsafe extern "C" fn(*mut Cdm10, *const InputBuffer2, *mut DecryptedBlock) -> u32,
    pub initialize_audio_decoder: unsafe extern "C" fn(*mut Cdm10, *const c_void) -> u32,
    pub initialize_video_decoder: unsafe extern "C" fn(*mut Cdm10, *const c_void) -> u32,
    pub deinitialize_decoder: unsafe extern "C" fn(*mut Cdm10, u32),
    pub reset_decoder: unsafe extern "C" fn(*mut Cdm10, u32),
    pub decrypt_and_decode_frame: unsafe extern "C" fn(*mut Cdm10, *const InputBuffer2, *mut c_void) -> u32,
    pub decrypt_and_decode_samples: unsafe extern "C" fn(*mut Cdm10, *const InputBuffer2, *mut c_void) -> u32,
    pub on_platform_challenge_response: unsafe extern "C" fn(*mut Cdm10, *const PlatformChallengeResponse),
    pub on_query_output_protection_status: unsafe extern "C" fn(*mut Cdm10, u32, u32, u32),
    pub on_storage_id: unsafe extern "C" fn(*mut Cdm10, u32, *const u8, u32),
    pub destroy: unsafe extern "C" fn(*mut Cdm10),
}

/// `cdm::Host_10`, implemented by the helper.
#[repr(C)]
pub struct Host10Vtable<H> {
    pub allocate: extern "C" fn(*mut H, u32) -> *mut Buffer,
    pub set_timer: extern "C" fn(*mut H, i64, *mut c_void),
    pub get_current_wall_time: extern "C" fn(*mut H) -> f64,
    pub on_initialized: extern "C" fn(*mut H, bool),
    pub on_resolve_key_status_promise: extern "C" fn(*mut H, u32, u32),
    pub on_resolve_new_session_promise: extern "C" fn(*mut H, u32, *const c_char, u32),
    pub on_resolve_promise: extern "C" fn(*mut H, u32),
    pub on_reject_promise: extern "C" fn(*mut H, u32, u32, u32, *const c_char, u32),
    pub on_session_message: extern "C" fn(*mut H, *const c_char, u32, u32, *const c_char, u32),
    pub on_session_keys_change: extern "C" fn(*mut H, *const c_char, u32, bool, *const KeyInformation, u32),
    pub on_expiration_change: extern "C" fn(*mut H, *const c_char, u32, f64),
    pub on_session_closed: extern "C" fn(*mut H, *const c_char, u32),
    pub send_platform_challenge: extern "C" fn(*mut H, *const c_char, u32, *const c_char, u32),
    pub enable_output_protection: extern "C" fn(*mut H, u32),
    pub query_output_protection_status: extern "C" fn(*mut H),
    pub on_deferred_initialization_done: extern "C" fn(*mut H, u32, u32),
    pub create_file_io: extern "C" fn(*mut H, *mut c_void) -> *mut c_void,
    pub request_storage_id: extern "C" fn(*mut H, u32),
    pub destructors: Destructors,
}

/// `cdm::Buffer`, allocated by the host for the CDM.
#[repr(C)]
pub struct Buffer {
    pub vtable: *const BufferVtable,
}

#[repr(C)]
pub struct BufferVtable {
    pub destroy: extern "C" fn(*mut Buffer),
    pub capacity: extern "C" fn(*const Buffer) -> u32,
    pub data: extern "C" fn(*mut Buffer) -> *mut u8,
    pub set_size: extern "C" fn(*mut Buffer, u32),
    pub size: extern "C" fn(*const Buffer) -> u32,
    pub destructors: Destructors,
}

/// `cdm::DecryptedBlock`, handed to `Decrypt` by the host.
#[repr(C)]
pub struct DecryptedBlock {
    pub vtable: *const DecryptedBlockVtable,
}

#[repr(C)]
pub struct DecryptedBlockVtable {
    pub set_decrypted_buffer: extern "C" fn(*mut DecryptedBlock, *mut Buffer),
    pub decrypted_buffer: extern "C" fn(*mut DecryptedBlock) -> *mut Buffer,
    pub set_timestamp: extern "C" fn(*mut DecryptedBlock, i64),
    pub timestamp: extern "C" fn(*const DecryptedBlock) -> i64,
    pub destructors: Destructors,
}

pub extern "C" fn never_destroyed(_: *mut c_void) {}
