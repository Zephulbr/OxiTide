//! Hand-written binding for the Steinberg ASIO driver interface (IASIO) and
//! the driver registry. The interface is Steinberg's (ASIO SDK, used under its
//! GPLv3 option; see THIRD_PARTY_NOTICES.md); declaring it here avoids the
//! LLVM/bindgen build that `asio-sys` needs.
//!
//! 64-bit only: x64 and ARM64 have a single calling convention, so the C++
//! virtual methods take `this` as their first argument under `extern "C"`.

#[cfg(not(target_pointer_width = "64"))]
compile_error!("the ASIO binding supports 64-bit Windows only");

use std::ffi::{c_char, c_long, c_void};

use windows::core::{GUID, HRESULT, PCWSTR, PWSTR};
use windows::Win32::Foundation::{ERROR_SUCCESS, HWND};
use windows::Win32::System::Com::CLSIDFromString;
use windows::Win32::System::Registry::{
    RegCloseKey, RegEnumKeyExW, RegGetValueW, RegOpenKeyExW, HKEY, HKEY_LOCAL_MACHINE, KEY_READ,
    RRF_RT_REG_SZ,
};

pub type AsioBool = c_long;
pub type AsioError = c_long;

pub const ASIO_TRUE: AsioBool = 1;
pub const ASE_OK: AsioError = 0;
pub const ASE_SUCCESS: AsioError = 0x3f48_47a0;

/// `asioMessage` selectors.
pub const K_SELECTOR_SUPPORTED: c_long = 1;
pub const K_ENGINE_VERSION: c_long = 2;
pub const K_RESET_REQUEST: c_long = 3;
pub const K_BUFFER_SIZE_CHANGE: c_long = 4;
pub const K_RESYNC_REQUEST: c_long = 5;
pub const K_LATENCIES_CHANGED: c_long = 6;
pub const K_SUPPORTS_TIME_INFO: c_long = 7;

/// `ASIOSampleType` values for the little-endian PCM types we render.
pub const ST_INT16_LSB: c_long = 16;
pub const ST_INT24_LSB: c_long = 17;
pub const ST_INT32_LSB: c_long = 18;
pub const ST_FLOAT32_LSB: c_long = 19;
pub const ST_FLOAT64_LSB: c_long = 20;
pub const ST_INT32_LSB16: c_long = 24;
pub const ST_INT32_LSB18: c_long = 25;
pub const ST_INT32_LSB20: c_long = 26;
pub const ST_INT32_LSB24: c_long = 27;

/// `ASIOSamples` / `ASIOTimeStamp`: a 64-bit value split into two halves.
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct AsioInt64 {
    pub hi: u32,
    pub lo: u32,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AsioChannelInfo {
    pub channel: c_long,
    pub is_input: AsioBool,
    pub is_active: AsioBool,
    pub channel_group: c_long,
    pub sample_type: c_long,
    pub name: [c_char; 32],
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct AsioBufferInfo {
    pub is_input: AsioBool,
    pub channel_num: c_long,
    pub buffers: [*mut c_void; 2],
}

/// Opaque: we never read the time-info struct, only hand its pointer back.
pub enum AsioTime {}

#[repr(C)]
pub struct AsioCallbacks {
    pub buffer_switch: extern "C" fn(index: c_long, direct_process: AsioBool),
    pub sample_rate_did_change: extern "C" fn(rate: f64),
    pub asio_message:
        extern "C" fn(selector: c_long, value: c_long, message: *mut c_void, opt: *mut f64) -> c_long,
    pub buffer_switch_time_info:
        extern "C" fn(params: *mut AsioTime, index: c_long, direct_process: AsioBool) -> *mut AsioTime,
}

type This = *mut IAsio;

/// IUnknown followed by the IASIO methods, in declaration order.
#[repr(C)]
pub struct IAsioVtbl {
    pub query_interface: unsafe extern "C" fn(This, *const GUID, *mut *mut c_void) -> HRESULT,
    pub add_ref: unsafe extern "C" fn(This) -> u32,
    pub release: unsafe extern "C" fn(This) -> u32,
    pub init: unsafe extern "C" fn(This, sys_handle: *mut c_void) -> AsioBool,
    pub get_driver_name: unsafe extern "C" fn(This, name: *mut c_char),
    pub get_driver_version: unsafe extern "C" fn(This) -> c_long,
    pub get_error_message: unsafe extern "C" fn(This, message: *mut c_char),
    pub start: unsafe extern "C" fn(This) -> AsioError,
    pub stop: unsafe extern "C" fn(This) -> AsioError,
    pub get_channels: unsafe extern "C" fn(This, inputs: *mut c_long, outputs: *mut c_long) -> AsioError,
    pub get_latencies: unsafe extern "C" fn(This, input: *mut c_long, output: *mut c_long) -> AsioError,
    pub get_buffer_size: unsafe extern "C" fn(
        This,
        min: *mut c_long,
        max: *mut c_long,
        preferred: *mut c_long,
        granularity: *mut c_long,
    ) -> AsioError,
    pub can_sample_rate: unsafe extern "C" fn(This, rate: f64) -> AsioError,
    pub get_sample_rate: unsafe extern "C" fn(This, rate: *mut f64) -> AsioError,
    pub set_sample_rate: unsafe extern "C" fn(This, rate: f64) -> AsioError,
    pub get_clock_sources: unsafe extern "C" fn(This, clocks: *mut c_void, count: *mut c_long) -> AsioError,
    pub set_clock_source: unsafe extern "C" fn(This, reference: c_long) -> AsioError,
    pub get_sample_position:
        unsafe extern "C" fn(This, position: *mut AsioInt64, stamp: *mut AsioInt64) -> AsioError,
    pub get_channel_info: unsafe extern "C" fn(This, info: *mut AsioChannelInfo) -> AsioError,
    pub create_buffers: unsafe extern "C" fn(
        This,
        infos: *mut AsioBufferInfo,
        channels: c_long,
        buffer_size: c_long,
        callbacks: *const AsioCallbacks,
    ) -> AsioError,
    pub dispose_buffers: unsafe extern "C" fn(This) -> AsioError,
    pub control_panel: unsafe extern "C" fn(This) -> AsioError,
    pub future: unsafe extern "C" fn(This, selector: c_long, opt: *mut c_void) -> AsioError,
    pub output_ready: unsafe extern "C" fn(This) -> AsioError,
}

#[repr(C)]
pub struct IAsio {
    pub vtbl: *const IAsioVtbl,
}

#[link(name = "ole32")]
extern "system" {
    fn CoCreateInstance(
        clsid: *const GUID,
        outer: *mut c_void,
        context: u32,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT;
}

const CLSCTX_INPROC_SERVER: u32 = 0x1;

/// An owned driver instance. Not `Send`: COM requires the creating thread to
/// make every call (the render callback's `output_ready` is the documented
/// exception and goes through the raw pointer).
pub struct Driver {
    ptr: This,
}

impl Driver {
    /// Instantiate the driver registered under `clsid`. The caller's thread
    /// must have initialized COM. ASIO drivers answer to their CLSID as IID.
    pub fn create(clsid: &GUID) -> Result<Self, String> {
        let mut out: *mut c_void = std::ptr::null_mut();
        // SAFETY: plain COM activation with valid out-pointer.
        let hr = unsafe {
            CoCreateInstance(clsid, std::ptr::null_mut(), CLSCTX_INPROC_SERVER, clsid, &mut out)
        };
        if hr.is_err() || out.is_null() {
            return Err(format!("could not load the ASIO driver ({hr:?})"));
        }
        Ok(Self { ptr: out.cast() })
    }

    pub fn raw(&self) -> This {
        self.ptr
    }

    fn vtbl(&self) -> &IAsioVtbl {
        // SAFETY: a live COM object always points at its vtable.
        unsafe { &*(*self.ptr).vtbl }
    }

    pub fn init(&self, window: HWND) -> bool {
        // SAFETY: valid driver pointer; the handle may be any window.
        unsafe { (self.vtbl().init)(self.ptr, window.0) == ASIO_TRUE }
    }

    pub fn error_message(&self) -> String {
        let mut buf = [0 as c_char; 128];
        // SAFETY: the SDK documents a 124-byte buffer; ours is larger.
        unsafe { (self.vtbl().get_error_message)(self.ptr, buf.as_mut_ptr()) };
        c_string(&buf)
    }

    pub fn channels(&self) -> Result<(i32, i32), AsioError> {
        let (mut inputs, mut outputs) = (0, 0);
        // SAFETY: valid out-pointers.
        check(unsafe { (self.vtbl().get_channels)(self.ptr, &mut inputs, &mut outputs) })?;
        Ok((inputs, outputs))
    }

    /// `(input, output)` latencies in frames, including the buffer itself;
    /// valid once buffers exist.
    pub fn latencies(&self) -> Result<(i32, i32), AsioError> {
        let (mut input, mut output) = (0, 0);
        // SAFETY: valid out-pointers.
        check(unsafe { (self.vtbl().get_latencies)(self.ptr, &mut input, &mut output) })?;
        Ok((input, output))
    }

    /// `(min, max, preferred, granularity)` in frames.
    pub fn buffer_size(&self) -> Result<(i32, i32, i32, i32), AsioError> {
        let (mut min, mut max, mut preferred, mut granularity) = (0, 0, 0, 0);
        // SAFETY: valid out-pointers.
        check(unsafe {
            (self.vtbl().get_buffer_size)(self.ptr, &mut min, &mut max, &mut preferred, &mut granularity)
        })?;
        Ok((min, max, preferred, granularity))
    }

    pub fn can_sample_rate(&self, rate: f64) -> bool {
        // SAFETY: valid driver pointer.
        unsafe { (self.vtbl().can_sample_rate)(self.ptr, rate) == ASE_OK }
    }

    pub fn sample_rate(&self) -> Result<f64, AsioError> {
        let mut rate = 0.0;
        // SAFETY: valid out-pointer.
        check(unsafe { (self.vtbl().get_sample_rate)(self.ptr, &mut rate) })?;
        Ok(rate)
    }

    pub fn set_sample_rate(&self, rate: f64) -> Result<(), AsioError> {
        // SAFETY: valid driver pointer.
        check(unsafe { (self.vtbl().set_sample_rate)(self.ptr, rate) })
    }

    pub fn output_channel_type(&self, channel: i32) -> Result<c_long, AsioError> {
        let mut info = AsioChannelInfo {
            channel,
            is_input: 0,
            is_active: 0,
            channel_group: 0,
            sample_type: 0,
            name: [0; 32],
        };
        // SAFETY: valid in/out struct.
        check(unsafe { (self.vtbl().get_channel_info)(self.ptr, &mut info) })?;
        Ok(info.sample_type)
    }

    /// # Safety
    /// `callbacks` must outlive the buffers (until `dispose_buffers`).
    pub unsafe fn create_buffers(
        &self,
        infos: &mut [AsioBufferInfo],
        buffer_size: i32,
        callbacks: *const AsioCallbacks,
    ) -> Result<(), AsioError> {
        check(unsafe {
            (self.vtbl().create_buffers)(
                self.ptr,
                infos.as_mut_ptr(),
                infos.len() as c_long,
                buffer_size,
                callbacks,
            )
        })
    }

    pub fn dispose_buffers(&self) -> Result<(), AsioError> {
        // SAFETY: valid driver pointer.
        check(unsafe { (self.vtbl().dispose_buffers)(self.ptr) })
    }

    pub fn start(&self) -> Result<(), AsioError> {
        // SAFETY: valid driver pointer.
        check(unsafe { (self.vtbl().start)(self.ptr) })
    }

    pub fn stop(&self) -> Result<(), AsioError> {
        // SAFETY: valid driver pointer.
        check(unsafe { (self.vtbl().stop)(self.ptr) })
    }

    pub fn supports_output_ready(&self) -> bool {
        // SAFETY: valid driver pointer.
        unsafe { (self.vtbl().output_ready)(self.ptr) == ASE_OK }
    }
}

impl Drop for Driver {
    fn drop(&mut self) {
        // SAFETY: we own one reference from CoCreateInstance.
        unsafe { (self.vtbl().release)(self.ptr) };
    }
}

/// Call `outputReady` from the render callback.
///
/// # Safety
/// `ptr` must be a live driver with created buffers.
pub unsafe fn output_ready(ptr: This) {
    unsafe { ((*(*ptr).vtbl).output_ready)(ptr) };
}

fn check(code: AsioError) -> Result<(), AsioError> {
    if code == ASE_OK || code == ASE_SUCCESS {
        Ok(())
    } else {
        Err(code)
    }
}

/// Human text for an `ASIOError`.
pub fn error_text(code: AsioError) -> &'static str {
    match code {
        -1000 => "device not present",
        -999 => "hardware malfunction",
        -998 => "invalid parameter",
        -997 => "invalid mode",
        -996 => "sample position not advancing",
        -995 => "sample rate or clock not supported",
        -994 => "out of memory",
        _ => "driver error",
    }
}

fn c_string(buf: &[c_char]) -> String {
    let bytes: Vec<u8> = buf.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    String::from_utf8_lossy(&bytes).trim().to_string()
}

/// One installed driver from `HKLM\SOFTWARE\ASIO`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredDriver {
    /// The registry key name (stable; used in device ids).
    pub key: String,
    pub description: String,
    pub clsid: String,
}

const ASIO_KEY: PCWSTR = windows::core::w!("SOFTWARE\\ASIO");

/// List installed drivers without loading any of them. Missing key = none.
pub fn registered_drivers() -> Vec<RegisteredDriver> {
    let mut root = HKEY::default();
    // SAFETY: valid key handle out-pointer; closed below.
    if unsafe { RegOpenKeyExW(HKEY_LOCAL_MACHINE, ASIO_KEY, None, KEY_READ, &mut root) }
        != ERROR_SUCCESS
    {
        return Vec::new();
    }
    let mut out = Vec::new();
    for index in 0.. {
        let mut name = [0u16; 256];
        let mut len = name.len() as u32;
        // SAFETY: buffer and length describe `name`.
        let rc = unsafe {
            RegEnumKeyExW(root, index, Some(PWSTR(name.as_mut_ptr())), &mut len, None, None, None, None)
        };
        if rc != ERROR_SUCCESS {
            break;
        }
        let key = String::from_utf16_lossy(&name[..len as usize]);
        let subkey: Vec<u16> = key.encode_utf16().chain(std::iter::once(0)).collect();
        let Some(clsid) = read_string(root, &subkey, windows::core::w!("CLSID")) else {
            continue;
        };
        let description = read_string(root, &subkey, windows::core::w!("Description"))
            .filter(|d| !d.is_empty())
            .unwrap_or_else(|| key.clone());
        out.push(RegisteredDriver { key, description, clsid });
    }
    // SAFETY: opened above.
    unsafe {
        let _ = RegCloseKey(root);
    }
    out
}

fn read_string(root: HKEY, subkey: &[u16], value: PCWSTR) -> Option<String> {
    let mut buf = [0u16; 512];
    let mut bytes = (buf.len() * 2) as u32;
    // SAFETY: buffer and byte count describe `buf`; `subkey` is NUL-terminated.
    let rc = unsafe {
        RegGetValueW(
            root,
            PCWSTR(subkey.as_ptr()),
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut bytes),
        )
    };
    if rc != ERROR_SUCCESS {
        return None;
    }
    let chars = (bytes as usize / 2).min(buf.len());
    let text = String::from_utf16_lossy(&buf[..chars]);
    Some(text.trim_end_matches('\0').trim().to_string())
}

pub fn parse_clsid(text: &str) -> Option<GUID> {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: NUL-terminated string.
    unsafe { CLSIDFromString(PCWSTR(wide.as_ptr())) }.ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn struct_layouts_match_the_sdk() {
        // asio.h packs to 4 on MSVC; these sizes hold on 64-bit either way.
        assert_eq!(std::mem::size_of::<AsioBufferInfo>(), 24);
        assert_eq!(std::mem::size_of::<AsioChannelInfo>(), 52);
        assert_eq!(std::mem::size_of::<AsioCallbacks>(), 32);
        assert_eq!(std::mem::size_of::<IAsioVtbl>(), 24 * std::mem::size_of::<usize>());
    }

    #[test]
    fn parses_registry_clsids() {
        let guid = parse_clsid("{232685C6-6548-49D8-846D-4141A3EF7560}").unwrap();
        assert_eq!(guid.data1, 0x2326_85C6);
        assert!(parse_clsid("not a guid").is_none());
    }

    #[test]
    fn c_strings_stop_at_nul() {
        let mut buf = [0 as c_char; 8];
        for (slot, byte) in buf.iter_mut().zip(b"abc") {
            *slot = *byte as c_char;
        }
        assert_eq!(c_string(&buf), "abc");
    }

    #[test]
    fn registry_listing_does_not_fail() {
        // Machine-dependent contents; it must simply not panic.
        for driver in registered_drivers() {
            assert!(!driver.key.is_empty());
        }
    }
}
