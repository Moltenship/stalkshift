use stalkshift_core::{IndicatorPosition, LightPosition, WiperPosition};

/// Order is shared by the command mask, SDK registration and sent-input telemetry.
pub const INPUT_NAMES: [&str; 11] = [
    "lblinkerh",
    "rblinkerh",
    "lightoff",
    "lightpark",
    "lighton",
    "wipers0",
    "wipers1",
    "wipers2",
    "wipers3",
    "lighthorn",
    "hblight",
];
pub const INPUT_COUNT: usize = INPUT_NAMES.len();
pub const INPUT_MASK: u32 = (1 << INPUT_COUNT) - 1;
pub const INDICATOR_MASK: u32 = 0b11;
pub const LIGHT_MASK: u32 = 0b11100;
pub const FLASH_INPUT: u32 = 1 << 9;
pub const HIGH_BEAM_INPUT: u32 = 1 << 10;
/// Logical request consumed in the DLL; it is not a registered SDK input.
pub const MIST_REQUEST: u32 = 1 << INPUT_COUNT;
pub const WIPER_MASK: u32 = (0b1111 << 5) | MIST_REQUEST;

pub fn valid_inputs(mask: u32) -> bool {
    mask & !(INPUT_MASK | MIST_REQUEST) == 0
        && (mask & INDICATOR_MASK).count_ones() <= 1
        && (mask & LIGHT_MASK).count_ones() <= 1
        && (mask & WIPER_MASK).count_ones() <= 1
        && (mask & (FLASH_INPUT | HIGH_BEAM_INPUT)).count_ones() <= 1
}

pub fn indicator_inputs(position: IndicatorPosition) -> u32 {
    match position {
        IndicatorPosition::Unknown | IndicatorPosition::Centre => 0,
        IndicatorPosition::Left => 1,
        IndicatorPosition::Right => 2,
    }
}

pub fn light_inputs(position: LightPosition) -> u32 {
    match position {
        LightPosition::Unknown => 0,
        LightPosition::Off => 1 << 2,
        LightPosition::Parking => 1 << 3,
        LightPosition::LowBeam => 1 << 4,
    }
}

pub fn wiper_inputs(position: WiperPosition) -> u32 {
    match position {
        WiperPosition::Unknown => 0,
        WiperPosition::Off => 1 << 5,
        WiperPosition::Intermittent => 1 << 6,
        WiperPosition::Low => 1 << 7,
        WiperPosition::High => 1 << 8,
        WiperPosition::Mist => MIST_REQUEST,
    }
}

pub const CHANNEL_NAMES: [&str; 6] = [
    "truck.lblinker",
    "truck.rblinker",
    "truck.light.parking",
    "truck.light.beam.low",
    "truck.wipers",
    "truck.light.beam.high",
];
pub const CHANNEL_COUNT: usize = CHANNEL_NAMES.len();
pub const READY: u32 = 1;
pub const fn valid_bit(index: usize) -> u32 {
    1 << (1 + index)
}
pub const fn on_bit(index: usize) -> u32 {
    1 << (1 + CHANNEL_COUNT + index)
}
pub const fn sent_bit(index: usize) -> u32 {
    1 << (1 + CHANNEL_COUNT * 2 + index)
}
pub const STATUS_MASK: u32 = (1 << (1 + CHANNEL_COUNT * 2 + INPUT_COUNT)) - 1;

pub fn observed(value: u32, index: usize) -> Option<bool> {
    (value & valid_bit(index) != 0).then_some(value & on_bit(index) != 0)
}

pub fn status_value(
    ready: bool,
    observed: &[Option<bool>; CHANNEL_COUNT],
    sent: &[bool; INPUT_COUNT],
) -> u32 {
    let mut value = u32::from(ready);
    for (index, state) in observed.iter().enumerate() {
        if let Some(on) = state {
            value |= valid_bit(index);
            if *on {
                value |= on_bit(index);
            }
        }
    }
    for (index, on) in sent.iter().enumerate() {
        if *on {
            value |= sent_bit(index);
        }
    }
    value
}
