//! Minimal x64 ABI declarations adapted from the official SCS SDK 1.14 headers.
//! See THIRD_PARTY_NOTICES.md and third-party/scs-sdk-LICENSE.txt.
use std::ffi::{c_char, c_void};

pub const OK: i32 = 0;
pub const UNSUPPORTED: i32 = -1;
pub const INVALID: i32 = -2;
pub const ALREADY_REGISTERED: i32 = -3;
pub const NOT_FOUND: i32 = -4;
pub const ERROR: i32 = -7;
pub const BOOL: u32 = 1;

pub type Log = unsafe extern "system" fn(i32, *const c_char);
pub type InputCallback = unsafe extern "system" fn(*mut InputEvent, u32, *mut c_void) -> i32;
pub type ActiveCallback = unsafe extern "system" fn(u8, *mut c_void);
pub type EventCallback = unsafe extern "system" fn(u32, *const c_void, *mut c_void);
pub type ChannelCallback = unsafe extern "system" fn(*const c_char, u32, *const Value, *mut c_void);

#[repr(C)]
pub struct Common {
    pub game_name: *const c_char,
    pub game_id: *const c_char,
    pub game_version: u32,
    pub padding: u32,
    pub log: Option<Log>,
}
#[repr(C)]
pub struct InputParams {
    pub common: Common,
    pub register_device: Option<unsafe extern "system" fn(*const InputDevice) -> i32>,
}
#[repr(C)]
pub struct Input {
    pub name: *const c_char,
    pub display_name: *const c_char,
    pub value_type: u32,
    pub padding: u32,
}
#[repr(C)]
pub struct InputDevice {
    pub name: *const c_char,
    pub display_name: *const c_char,
    pub device_type: u32,
    pub input_count: u32,
    pub inputs: *const Input,
    pub context: *mut c_void,
    pub active: Option<ActiveCallback>,
    pub event: Option<InputCallback>,
}
#[repr(C)]
pub struct InputEvent {
    pub index: u32,
    // Official event union has six floats of reserved storage and 4-byte alignment.
    pub payload: [u32; 6],
}
#[repr(C)]
pub struct Value {
    pub value_type: u32,
    pub padding: u32,
    // scs_value_dplacement_t sets the union size (40) and alignment (8).
    pub payload: [u64; 5],
}
#[repr(C)]
pub struct TelemetryParams {
    pub common: Common,
    pub register_event:
        Option<unsafe extern "system" fn(u32, Option<EventCallback>, *mut c_void) -> i32>,
    pub unregister_event: Option<unsafe extern "system" fn(u32) -> i32>,
    pub register_channel: Option<
        unsafe extern "system" fn(
            *const c_char,
            u32,
            u32,
            u32,
            Option<ChannelCallback>,
            *mut c_void,
        ) -> i32,
    >,
    pub unregister_channel: Option<unsafe extern "system" fn(*const c_char, u32, u32) -> i32>,
}

// Match the explicit x64 assertions from the official SDK; fail compilation on drift.
const _: () = {
    assert!(std::mem::size_of::<Common>() == 32);
    assert!(std::mem::size_of::<InputParams>() == 40);
    assert!(std::mem::size_of::<TelemetryParams>() == 64);
    assert!(std::mem::size_of::<Input>() == 24);
    assert!(std::mem::size_of::<InputDevice>() == 56);
    assert!(std::mem::size_of::<InputEvent>() == 28);
    assert!(std::mem::align_of::<InputEvent>() == 4);
    assert!(std::mem::size_of::<Value>() == 48);
    assert!(std::mem::offset_of!(Value, payload) == 8);
};
