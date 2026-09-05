use stalkshift_core::{IndicatorPosition, LightPosition, WiperPosition};

/// Order is shared by the command mask, SDK registration and sent-input telemetry.
pub const INPUT_NAMES: [&str; 21] = [
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
    "horn",
    "flasher4way",
    "gear0",
    "geardrive",
    "gearreverse",
    "parkingbrake",
    "cruiectrl",
    "cruiectrlres",
    "cruiectrlinc",
    "cruiectrldec",
];
pub const INPUT_COUNT: usize = INPUT_NAMES.len();
pub const INPUT_MASK: u64 = (1 << INPUT_COUNT) - 1;
pub const INDICATOR_MASK: u64 = 0b11;
pub const LIGHT_MASK: u64 = 0b11100;
pub const FLASH_INPUT: u64 = 1 << 9;
pub const HIGH_BEAM_INPUT: u64 = 1 << 10;
/// Logical request consumed in the DLL; it is not a registered SDK input.
pub const MIST_REQUEST: u64 = 1 << INPUT_COUNT;
pub const WIPER_MASK: u64 = (0b1111 << 5) | MIST_REQUEST;
pub const HORN: u64 = 1 << 11;
pub const HAZARD: u64 = 1 << 12;
pub const NEUTRAL: u64 = 1 << 13;
pub const DRIVE: u64 = 1 << 14;
pub const REVERSE: u64 = 1 << 15;
pub const PARKING: u64 = 1 << 16;
pub const CRUISE_TOGGLE: u64 = 1 << 17;
pub const CRUISE_RESUME: u64 = 1 << 18;
pub const CRUISE_UP: u64 = 1 << 19;
pub const CRUISE_DOWN: u64 = 1 << 20;
pub const PARK_REQUEST: u64 = 1 << 22;
pub const CRUISE_PULL: u64 = 1 << 23;
pub const AUTO_TOGGLE: u64 = 1 << 24;
pub const GEAR_MASK: u64 = NEUTRAL | DRIVE | REVERSE | PARK_REQUEST;
pub const LOGICAL_MASK: u64 = MIST_REQUEST | PARK_REQUEST | CRUISE_PULL | AUTO_TOGGLE;
pub const UNKNOWN_NUMBER: i32 = i32::MIN;
/// Speeds in millimetres/second, then displayed gear; MIN means unavailable.
pub const UNKNOWN_MOTION: [i32; 4] = [UNKNOWN_NUMBER; 4];
pub const NUMBER_NAMES: [&str; 4] = [
    "truck.cruise_control",
    "truck.navigation.speed.limit",
    "truck.speed",
    "truck.displayed.gear",
];

pub fn auxiliary_inputs(state: stalkshift_core::AuxiliaryState) -> u64 {
    use stalkshift_core::Selector;
    let mut mask = match state.selector {
        Selector::Unknown => 0,
        Selector::Drive => DRIVE,
        Selector::Neutral => NEUTRAL,
        Selector::Reverse => REVERSE,
        Selector::Park => PARK_REQUEST,
    };
    for (enabled, bit) in [
        (state.horn, HORN),
        (state.parking_press, PARKING),
        (state.hazard_press, HAZARD),
        (state.cruise_toggle, CRUISE_TOGGLE),
        (state.cruise_pull, CRUISE_PULL),
        (state.cruise_up, CRUISE_UP),
        (state.cruise_down, CRUISE_DOWN),
        (state.automatic_toggle, AUTO_TOGGLE),
    ] {
        if enabled {
            mask |= bit;
        }
    }
    mask
}

pub fn valid_inputs(mask: u64) -> bool {
    mask & !(INPUT_MASK | LOGICAL_MASK) == 0
        && (mask & GEAR_MASK).count_ones() <= 1
        && (mask & (CRUISE_UP | CRUISE_DOWN)).count_ones() <= 1
        && (mask & (CRUISE_TOGGLE | CRUISE_RESUME | CRUISE_PULL)).count_ones() <= 1
        && (mask & INDICATOR_MASK).count_ones() <= 1
        && (mask & LIGHT_MASK).count_ones() <= 1
        && (mask & WIPER_MASK).count_ones() <= 1
        && (mask & (FLASH_INPUT | HIGH_BEAM_INPUT)).count_ones() <= 1
}

pub fn indicator_inputs(position: IndicatorPosition) -> u64 {
    match position {
        IndicatorPosition::Unknown | IndicatorPosition::Centre => 0,
        IndicatorPosition::Left => 1,
        IndicatorPosition::Right => 2,
    }
}

pub fn light_inputs(position: LightPosition) -> u64 {
    match position {
        LightPosition::Unknown => 0,
        LightPosition::Off => 1 << 2,
        LightPosition::Parking => 1 << 3,
        LightPosition::LowBeam => 1 << 4,
    }
}

pub fn wiper_inputs(position: WiperPosition) -> u64 {
    match position {
        WiperPosition::Unknown => 0,
        WiperPosition::Off => 1 << 5,
        WiperPosition::Intermittent => 1 << 6,
        WiperPosition::Low => 1 << 7,
        WiperPosition::High => 1 << 8,
        WiperPosition::Mist => MIST_REQUEST,
    }
}

pub const CHANNEL_NAMES: [&str; 7] = [
    "truck.lblinker",
    "truck.rblinker",
    "truck.light.parking",
    "truck.light.beam.low",
    "truck.wipers",
    "truck.light.beam.high",
    "truck.brake.parking",
];
pub const CHANNEL_COUNT: usize = CHANNEL_NAMES.len();
pub const READY: u64 = 1;
pub const fn valid_bit(index: usize) -> u64 {
    1 << (1 + index)
}
pub const fn on_bit(index: usize) -> u64 {
    1 << (1 + CHANNEL_COUNT + index)
}
pub const fn sent_bit(index: usize) -> u64 {
    1 << (1 + CHANNEL_COUNT * 2 + index)
}
pub const AUTO_ENABLED: u64 = 1 << (1 + CHANNEL_COUNT * 2 + INPUT_COUNT);
pub const STATUS_MASK: u64 = (AUTO_ENABLED << 1) - 1;

pub fn observed(value: u64, index: usize) -> Option<bool> {
    (value & valid_bit(index) != 0).then_some(value & on_bit(index) != 0)
}

pub fn status_value(
    ready: bool,
    observed: &[Option<bool>; CHANNEL_COUNT],
    sent: &[bool; INPUT_COUNT],
) -> u64 {
    let mut value = u64::from(ready);
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
