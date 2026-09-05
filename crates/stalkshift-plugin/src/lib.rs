//! SCS ABI boundary. All unsafe operations are confined to this module and ffi.rs.
//! Caller-owned SDK pointers are used only during the callback that supplied them.

pub mod ffi;
mod state;
#[cfg(windows)]
mod worker;

use ffi::*;
use state::{Dispatch, GENERATION, INPUT_ACTIVE, OBSERVED, RUNNING, TELEMETRY_INSTALLED, shared};
use std::cell::RefCell;
use std::ffi::{CStr, c_char, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

static INPUT_INITIALIZED: AtomicBool = AtomicBool::new(false);
static TELEMETRY_INITIALIZED: AtomicBool = AtomicBool::new(false);
static FAULT: AtomicBool = AtomicBool::new(false);
thread_local! { static DISPATCH: RefCell<Dispatch> = RefCell::new(Dispatch::default()); }

fn boundary(action: impl FnOnce() -> i32) -> i32 {
    catch_unwind(AssertUnwindSafe(action)).unwrap_or_else(|_| {
        FAULT.store(true, Ordering::Relaxed);
        ERROR
    })
}

unsafe fn is_ets2(common: &Common) -> bool {
    // SAFETY: SCS supplies a valid terminated game_id for the lifetime of init.
    !common.game_id.is_null() && unsafe { CStr::from_ptr(common.game_id) } == c"eut2"
}
unsafe fn log(common: &Common, level: i32, message: &CStr) {
    if let Some(log) = common.log {
        // SAFETY: SDK logging is called only during an SDK invocation on its main thread.
        unsafe {
            log(level, message.as_ptr());
        }
    }
}

/// # Safety
/// `params` must point to valid SCS Input API 1.00 parameters when version is 1.00.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn scs_input_init(version: u32, params: *const InputParams) -> i32 {
    boundary(|| {
        if version != 0x10000 {
            return UNSUPPORTED;
        }
        if params.is_null() {
            return INVALID;
        }
        // SAFETY: version checked above; the caller guarantees the matching parameter layout.
        let params = unsafe { &*params };
        // SAFETY: common is part of the validated parameter object.
        if !unsafe { is_ets2(&params.common) } {
            return UNSUPPORTED;
        }
        let Some(register) = params.register_device else {
            return INVALID;
        };
        if INPUT_INITIALIZED.swap(true, Ordering::SeqCst) {
            return ALREADY_REGISTERED;
        }
        FAULT.store(false, Ordering::Relaxed);
        #[cfg(windows)]
        if worker::start().is_err() {
            INPUT_INITIALIZED.store(false, Ordering::SeqCst);
            return ERROR;
        }
        let inputs = [
            Input {
                name: c"lblinkerh".as_ptr(),
                display_name: c"StalkShift Left Indicator".as_ptr(),
                value_type: BOOL,
                padding: 0,
            },
            Input {
                name: c"rblinkerh".as_ptr(),
                display_name: c"StalkShift Right Indicator".as_ptr(),
                value_type: BOOL,
                padding: 0,
            },
        ];
        let device = InputDevice {
            name: c"stalkshift".as_ptr(),
            display_name: c"StalkShift".as_ptr(),
            device_type: 2,
            input_count: 2,
            inputs: inputs.as_ptr(),
            context: std::ptr::null_mut(),
            active: Some(active_callback),
            event: Some(input_callback),
        };
        // SAFETY: all pointed-to arrays/strings live through registration; SCS fully processes them during this call.
        let result = unsafe { register(&device) };
        if result != OK {
            #[cfg(windows)]
            worker::stop();
            INPUT_INITIALIZED.store(false, Ordering::SeqCst);
            // SAFETY: still inside init on the SCS thread.
            unsafe {
                log(&params.common, 2, c"[StalkShift] Input registration failed");
            }
            return result;
        }
        // SAFETY: still inside init on the SCS thread.
        unsafe {
            log(
                &params.common,
                0,
                c"[StalkShift] Input 1.00 initialized: lblinkerh / rblinkerh",
            );
        }
        OK
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn scs_input_shutdown() {
    boundary(|| {
        INPUT_INITIALIZED.store(false, Ordering::SeqCst);
        INPUT_ACTIVE.store(false, Ordering::SeqCst);
        GENERATION.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut state) = shared().lock() {
            state.input_active = false;
            state.gate.disconnect();
            state.update_ready();
        }
        #[cfg(windows)]
        worker::stop();
        OK
    });
}

unsafe extern "system" fn active_callback(active: u8, _: *mut c_void) {
    boundary(|| {
        if INPUT_ACTIVE.swap(active != 0, Ordering::SeqCst) != (active != 0) {
            GENERATION.fetch_add(1, Ordering::SeqCst);
        }
        OK
    });
}

unsafe extern "system" fn input_callback(
    event: *mut InputEvent,
    flags: u32,
    _: *mut c_void,
) -> i32 {
    boundary(|| {
        if event.is_null() {
            return INVALID;
        }
        DISPATCH.with(|dispatch| {
            let mut dispatch = dispatch.borrow_mut();
            if flags & 3 != 0 {
                if !INPUT_ACTIVE.swap(true, Ordering::SeqCst) {
                    GENERATION.fetch_add(1, Ordering::SeqCst);
                }
                let desired = if let Ok(mut state) = shared().try_lock() {
                    state.refresh();
                    let outputs = if FAULT.load(Ordering::Relaxed) {
                        [false; 2]
                    } else {
                        state.gate.outputs(Instant::now())
                    };
                    state.sent = outputs;
                    outputs
                } else {
                    [false; 2]
                };
                dispatch.begin(desired);
            }
            let Some((index, enabled)) = dispatch.pop() else {
                return NOT_FOUND;
            };
            // SAFETY: SCS supplies an aligned writable InputEvent; initialized payload covers the entire SDK union.
            unsafe {
                event.write(InputEvent {
                    index,
                    payload: [u32::from(enabled), 0, 0, 0, 0, 0],
                });
            }
            OK
        })
    })
}

/// # Safety
/// `params` must point to the SCS telemetry parameters for a supported API version.
#[unsafe(no_mangle)]
pub unsafe extern "system" fn scs_telemetry_init(
    version: u32,
    params: *const TelemetryParams,
) -> i32 {
    boundary(|| {
        if version != 0x10000 && version != 0x10001 {
            return UNSUPPORTED;
        }
        if params.is_null() {
            return INVALID;
        }
        // SAFETY: both supported versions have the same documented layout.
        let params = unsafe { &*params };
        // SAFETY: common is part of caller's valid initialization parameters.
        if !unsafe { is_ets2(&params.common) } {
            return UNSUPPORTED;
        }
        let (
            Some(register_event),
            Some(unregister_event),
            Some(register_channel),
            Some(unregister_channel),
        ) = (
            params.register_event,
            params.unregister_event,
            params.register_channel,
            params.unregister_channel,
        )
        else {
            return INVALID;
        };
        if TELEMETRY_INITIALIZED.swap(true, Ordering::SeqCst) {
            return ALREADY_REGISTERED;
        }
        let channels = [c"truck.lblinker", c"truck.rblinker"];
        let mut events_registered = Vec::new();
        let mut channels_registered = Vec::new();
        let result = (|| {
            for event in [3, 4] {
                // SAFETY: callback ABI matches SDK, context is unused, initialization permits registration.
                let result =
                    unsafe { register_event(event, Some(telemetry_event), std::ptr::null_mut()) };
                if result != OK {
                    return result;
                }
                events_registered.push(event);
            }
            for (index, channel) in channels.iter().enumerate() {
                // SAFETY: static channel names; opaque context encodes an index, never dereferenced.
                let result = unsafe {
                    register_channel(
                        channel.as_ptr(),
                        u32::MAX,
                        BOOL,
                        3,
                        Some(telemetry_channel),
                        index as *mut c_void,
                    )
                };
                if result != OK {
                    return result;
                }
                channels_registered.push(*channel);
            }
            OK
        })();
        if result != OK {
            for channel in channels_registered {
                // SAFETY: undo only registrations completed above, during the same init invocation.
                unsafe {
                    unregister_channel(channel.as_ptr(), u32::MAX, BOOL);
                }
            }
            for event in events_registered {
                // SAFETY: undo only registrations completed above, during the same init invocation.
                unsafe {
                    unregister_event(event);
                }
            }
            TELEMETRY_INITIALIZED.store(false, Ordering::SeqCst);
            return result;
        }
        TELEMETRY_INSTALLED.store(true, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
        for observed in &OBSERVED {
            observed.store(0, Ordering::SeqCst);
        }
        GENERATION.fetch_add(1, Ordering::SeqCst);
        // SAFETY: still inside telemetry init on the game thread.
        unsafe {
            log(
                &params.common,
                0,
                c"[StalkShift] Telemetry initialized: logical indicators and pause state",
            );
        }
        OK
    })
}

#[unsafe(no_mangle)]
pub extern "system" fn scs_telemetry_shutdown() {
    boundary(|| {
        TELEMETRY_INITIALIZED.store(false, Ordering::SeqCst);
        TELEMETRY_INSTALLED.store(false, Ordering::SeqCst);
        RUNNING.store(false, Ordering::SeqCst);
        for observed in &OBSERVED {
            observed.store(0, Ordering::SeqCst);
        }
        GENERATION.fetch_add(1, Ordering::SeqCst);
        // SDK automatically unregisters telemetry callbacks when this function returns.
        OK
    });
}

unsafe extern "system" fn telemetry_event(event: u32, _: *const c_void, _: *mut c_void) {
    boundary(|| {
        RUNNING.store(event == 4, Ordering::SeqCst);
        for observed in &OBSERVED {
            observed.store(0, Ordering::SeqCst);
        }
        GENERATION.fetch_add(1, Ordering::SeqCst);
        OK
    });
}

unsafe extern "system" fn telemetry_channel(
    _: *const c_char,
    _: u32,
    value: *const Value,
    context: *mut c_void,
) {
    boundary(|| {
        let index = context as usize;
        if index > 1 {
            return INVALID;
        }
        let observed = if value.is_null() {
            0
        } else {
            // SAFETY: SCS supplies initialized type and bool byte; the rest of its union
            // may be uninitialized, so read only the single byte for the registered bool type.
            unsafe {
                if std::ptr::addr_of!((*value).value_type).read() == BOOL {
                    if std::ptr::addr_of!((*value).payload).cast::<u8>().read() != 0 {
                        2
                    } else {
                        1
                    }
                } else {
                    0
                }
            }
        };
        OBSERVED[index].store(observed, Ordering::SeqCst);
        OK
    });
}
