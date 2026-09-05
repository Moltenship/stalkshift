use stalkshift_protocol::{CHANNEL_COUNT, INPUT_COUNT, InputGate, Kind, Packet, status_value};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU64, Ordering},
};
use std::time::Instant;

pub static INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static RUNNING: AtomicBool = AtomicBool::new(false);
pub static TELEMETRY_INSTALLED: AtomicBool = AtomicBool::new(false);
pub static OBSERVED: [AtomicU8; CHANNEL_COUNT] = [const { AtomicU8::new(0) }; CHANNEL_COUNT];
pub static NUMBERS: [AtomicI32; 4] = [const { AtomicI32::new(i32::MIN) }; 4];
pub static NUMBER_TIMES: [AtomicU64; 4] = [const { AtomicU64::new(0) }; 4];
pub fn clock_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64 + 1
}
pub fn motion() -> [i32; 4] {
    let now = clock_ms();
    std::array::from_fn(|i| {
        let time = NUMBER_TIMES[i].load(Ordering::SeqCst);
        if time == 0 || now.saturating_sub(time) > 500 {
            i32::MIN
        } else {
            NUMBERS[i].load(Ordering::SeqCst)
        }
    })
}
pub static GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct State {
    pub gate: InputGate,
    pub input_active: bool,
    pub telemetry_running: bool,
    pub telemetry_installed: bool,
    pub observed: [Option<bool>; CHANNEL_COUNT],
    pub sent: [bool; INPUT_COUNT],
    generation: u64,
}

impl State {
    pub fn refresh(&mut self) {
        let generation = GENERATION.load(Ordering::SeqCst);
        if generation != self.generation {
            self.generation = generation;
            self.gate.invalidate();
        }
        self.input_active = INPUT_ACTIVE.load(Ordering::SeqCst);
        self.telemetry_running = RUNNING.load(Ordering::SeqCst);
        self.telemetry_installed = TELEMETRY_INSTALLED.load(Ordering::SeqCst);
        self.observed = std::array::from_fn(|index| match OBSERVED[index].load(Ordering::SeqCst) {
            1 => Some(false),
            2 => Some(true),
            _ => None,
        });
        self.gate.observe_wipers(self.observed[4]);
        self.gate.observe_driving(self.observed[6], motion());
        self.update_ready();
    }
    pub fn update_ready(&mut self) {
        self.gate.set_ready(
            self.input_active
                && self.telemetry_running
                && self.telemetry_installed
                && self.observed[..6].iter().all(Option::is_some),
        );
    }
    pub fn status(&mut self, sequence: u64) -> Packet {
        self.refresh();
        self.gate.expire(Instant::now());
        let value = status_value(self.gate.ready(), &self.observed, &self.sent)
            | if self.gate.automatic() {
                stalkshift_protocol::AUTO_ENABLED
            } else {
                0
            };
        Packet {
            motion: motion(),
            kind: Kind::Status,
            value,
            session: self.gate.session,
            sequence,
            epoch: self.gate.epoch,
        }
    }
}

pub fn shared() -> &'static Mutex<State> {
    static SHARED: OnceLock<Mutex<State>> = OnceLock::new();
    SHARED.get_or_init(|| Mutex::new(State::default()))
}

/// Release inactive inputs before asserting any mode; each index appears once.
pub struct Dispatch {
    events: [(u32, bool); INPUT_COUNT],
    next: usize,
}
impl Default for Dispatch {
    fn default() -> Self {
        Self {
            events: [(0, false); INPUT_COUNT],
            next: INPUT_COUNT,
        }
    }
}
impl Dispatch {
    pub fn begin(&mut self, desired: [bool; INPUT_COUNT]) {
        let mut position = 0;
        for enabled in [false, true] {
            for (index, value) in desired.iter().copied().enumerate() {
                if value == enabled {
                    self.events[position] = (index as u32, value);
                    position += 1;
                }
            }
        }
        self.next = 0;
    }
    pub fn pop(&mut self) -> Option<(u32, bool)> {
        let event = self.events.get(self.next).copied();
        self.next = (self.next + 1).min(INPUT_COUNT);
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_releases_opposite_side_before_asserting_and_is_finite() {
        let mut dispatch = Dispatch::default();
        assert_eq!(dispatch.pop(), None);
        for mask in (0..INPUT_COUNT)
            .flat_map(|i| (0..INPUT_COUNT).map(move |j| (1_u64 << i) | (1_u64 << j)))
            .chain([0, (1_u64 << INPUT_COUNT) - 1])
        {
            let desired = std::array::from_fn(|index| mask & (1 << index) != 0);
            dispatch.begin(desired);
            let events: Vec<_> = std::iter::from_fn(|| dispatch.pop()).collect();
            assert_eq!(events.len(), INPUT_COUNT);
            let mut seen = [false; INPUT_COUNT];
            let mut asserted = false;
            for (index, enabled) in events {
                assert!(!seen[index as usize]);
                seen[index as usize] = true;
                assert_eq!(desired[index as usize], enabled);
                assert!(
                    !asserted || enabled,
                    "released a mode after asserting another"
                );
                asserted |= enabled;
            }
        }
    }
    #[test]
    fn missing_telemetry_or_pause_prevents_readiness() {
        let mut state = State {
            input_active: true,
            telemetry_running: true,
            telemetry_installed: true,
            ..State::default()
        };
        state.observed = [Some(false); CHANNEL_COUNT];
        state.observed[1] = None;
        state.update_ready();
        assert!(!state.gate.ready());
        state.observed[1] = Some(false);
        state.update_ready();
        assert!(state.gate.ready());
        state.telemetry_running = false;
        state.update_ready();
        assert!(!state.gate.ready());
    }
}
