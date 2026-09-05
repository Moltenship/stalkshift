//! SCS ABI boundary. All unsafe operations are confined to this module and ffi.rs.
//! Caller-owned SDK pointers are used only during the callback that supplied them.

pub mod ffi;
mod game;
mod state;
#[cfg(windows)]
mod worker;

use ffi::*;
use stalkshift_protocol::{CHANNEL_COUNT, INPUT_COUNT, INPUT_NAMES};
use state::{Dispatch, GENERATION, INPUT_ACTIVE, OBSERVED, RUNNING, TELEMETRY_INSTALLED, shared};
use std::cell::RefCell;
use std::ffi::{CStr, CString, c_char, c_void};
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

unsafe fn supported_game(common: &Common) -> Option<game::Game> {
    // SAFETY: SCS supplies a valid terminated game_id for the lifetime of init.
    if common.game_id.is_null() {
        None
    } else {
        // SAFETY: non-null SDK game_id is valid and terminated during init.
        game::Game::from_id(unsafe { CStr::from_ptr(common.game_id) })
    }
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
        let Some(game) = (unsafe { supported_game(&params.common) }) else {
            return UNSUPPORTED;
        };
        let Some(register) = params.register_device else {
            return INVALID;
        };
        if INPUT_INITIALIZED.swap(true, Ordering::SeqCst) {
            return ALREADY_REGISTERED;
        }
        if let Ok(mut state) = shared().lock() {
            state.gate.set_cruise_unit(game.installed_unit());
        }
        FAULT.store(false, Ordering::Relaxed);
        #[cfg(windows)]
        if worker::start().is_err() {
            INPUT_INITIALIZED.store(false, Ordering::SeqCst);
            return ERROR;
        }
        let names: Vec<_> = INPUT_NAMES
            .iter()
            .map(|name| CString::new(*name).expect("static input name"))
            .collect();
        let inputs: Vec<_> = names
            .iter()
            .map(|name| Input {
                name: name.as_ptr(),
                display_name: name.as_ptr(),
                value_type: BOOL,
                padding: 0,
            })
            .collect();
        let device = InputDevice {
            name: c"stalkshift".as_ptr(),
            display_name: c"StalkShift".as_ptr(),
            device_type: 2,
            input_count: INPUT_COUNT as u32,
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
                c"[StalkShift] Input 1.00 initialized: indicators, lights, beams and wipers",
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
                        [false; INPUT_COUNT]
                    } else {
                        state.gate.outputs(Instant::now())
                    };
                    state.sent = outputs;
                    outputs
                } else {
                    [false; INPUT_COUNT]
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
        let Some(game) = (unsafe { supported_game(&params.common) }) else {
            return UNSUPPORTED;
        };
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
        if let Ok(mut state) = shared().lock() {
            state.gate.set_cruise_unit(game.installed_unit());
        }
        let channels = [
            c"truck.lblinker",
            c"truck.rblinker",
            c"truck.light.parking",
            c"truck.light.beam.low",
            c"truck.wipers",
            c"truck.light.beam.high",
            c"truck.brake.parking",
        ];
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
                channels_registered.push((*channel, BOOL));
            }
            for (index, (name, kind)) in [
                (c"truck.cruise_control", 5),
                (c"truck.navigation.speed.limit", 5),
                (c"truck.speed", 5),
                (c"truck.displayed.gear", 2),
            ]
            .into_iter()
            .enumerate()
            {
                // SAFETY: static names, SDK types float/s32, opaque integer context.
                let result = unsafe {
                    register_channel(
                        name.as_ptr(),
                        u32::MAX,
                        kind,
                        3,
                        Some(numeric_channel),
                        index as *mut c_void,
                    )
                };
                if result != OK {
                    return result;
                }
                channels_registered.push((name, kind));
            }
            OK
        })();
        if result != OK {
            for (channel, kind) in channels_registered {
                // SAFETY: undo only registrations completed above, during the same init invocation.
                unsafe {
                    unregister_channel(channel.as_ptr(), u32::MAX, kind);
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
        for timestamp in &crate::state::NUMBER_TIMES {
            timestamp.store(0, Ordering::SeqCst);
        }
        for observed in &OBSERVED {
            observed.store(0, Ordering::SeqCst);
        }
        GENERATION.fetch_add(1, Ordering::SeqCst);
        // SAFETY: still inside telemetry init on the game thread.
        unsafe {
            log(
                &params.common,
                0,
                c"[StalkShift] Telemetry initialized: indicators, lights, wipers and pause state",
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
        for timestamp in &crate::state::NUMBER_TIMES {
            timestamp.store(0, Ordering::SeqCst);
        }
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
        for timestamp in &crate::state::NUMBER_TIMES {
            timestamp.store(0, Ordering::SeqCst);
        }
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
        if index >= CHANNEL_COUNT {
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

unsafe extern "system" fn numeric_channel(
    _: *const c_char,
    _: u32,
    value: *const Value,
    context: *mut c_void,
) {
    boundary(|| {
        let index = context as usize;
        if index >= 4 {
            return INVALID;
        }
        let mut number = i32::MIN;
        if !value.is_null() {
            // SAFETY: read only the SDK's initialized 4-byte float/s32 payload,
            // never the uninitialized remainder of the union.
            unsafe {
                let kind = std::ptr::addr_of!((*value).value_type).read();
                let payload = std::ptr::addr_of!((*value).payload);
                if index == 3 && kind == 2 {
                    number = payload.cast::<i32>().read();
                } else if index < 3 && kind == 5 {
                    let speed = payload.cast::<f32>().read();
                    if speed.is_finite() && speed.abs() <= 200.0 {
                        number = (speed * 1000.0).round() as i32;
                    }
                }
            }
        }
        crate::state::NUMBERS[index].store(number, Ordering::SeqCst);
        crate::state::NUMBER_TIMES[index].store(crate::state::clock_ms(), Ordering::SeqCst);
        OK
    });
}

#[cfg(test)]
mod numeric_tests {
    use super::*;
    #[test]
    fn sdk_float_and_signed_gear_decode_and_missing_values_expire() {
        let mut value = Value {
            value_type: 5,
            padding: 0,
            payload: [0; 5],
        };
        value.payload[0] = u64::from(22.5_f32.to_bits());
        // SAFETY: test values initialize the entire SDK-compatible object.
        unsafe {
            numeric_channel(std::ptr::null(), u32::MAX, &value, std::ptr::null_mut());
        }
        assert_eq!(crate::state::motion()[0], 22500);
        value.payload[0] = u64::from(f32::NAN.to_bits());
        // SAFETY: initialized Value; the callback validates the float payload.
        unsafe {
            numeric_channel(std::ptr::null(), u32::MAX, &value, std::ptr::null_mut());
        }
        assert_eq!(crate::state::motion()[0], i32::MIN);
        value.value_type = 2;
        value.payload[0] = u64::from((-1_i32) as u32);
        // SAFETY: initialized signed payload; context encodes index 3, not a pointer to memory.
        unsafe {
            numeric_channel(std::ptr::null(), u32::MAX, &value, 3_usize as *mut c_void);
        }
        assert_eq!(crate::state::motion()[3], -1);
        // SAFETY: null is the documented no-value callback argument.
        unsafe {
            numeric_channel(
                std::ptr::null(),
                u32::MAX,
                std::ptr::null(),
                3_usize as *mut c_void,
            );
        }
        assert_eq!(crate::state::motion()[3], i32::MIN);
        crate::state::NUMBERS[0].store(22500, Ordering::SeqCst);
        crate::state::NUMBER_TIMES[0].store(0, Ordering::SeqCst);
        assert_eq!(crate::state::motion()[0], i32::MIN);
    }
}
