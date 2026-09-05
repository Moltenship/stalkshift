use stalkshift_protocol::{
    InputGate, Kind, LEFT_ON, LEFT_SENT, LEFT_VALID, Packet, READY, RIGHT_ON, RIGHT_SENT,
    RIGHT_VALID,
};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering},
};
use std::time::Instant;

pub static INPUT_ACTIVE: AtomicBool = AtomicBool::new(false);
pub static RUNNING: AtomicBool = AtomicBool::new(false);
pub static TELEMETRY_INSTALLED: AtomicBool = AtomicBool::new(false);
pub static OBSERVED: [AtomicU8; 2] = [AtomicU8::new(0), AtomicU8::new(0)];
pub static GENERATION: AtomicU64 = AtomicU64::new(1);

#[derive(Default)]
pub struct State {
    pub gate: InputGate,
    pub input_active: bool,
    pub telemetry_running: bool,
    pub telemetry_installed: bool,
    pub observed: [Option<bool>; 2],
    pub sent: [bool; 2],
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
        self.update_ready();
    }
    pub fn update_ready(&mut self) {
        self.gate.set_ready(
            self.input_active
                && self.telemetry_running
                && self.telemetry_installed
                && self.observed.iter().all(Option::is_some),
        );
    }
    pub fn status(&mut self, sequence: u64) -> Packet {
        self.refresh();
        self.gate.expire(Instant::now());
        let value = (u8::from(self.gate.ready()) * READY)
            | (u8::from(self.observed[0].is_some()) * LEFT_VALID)
            | (u8::from(self.observed[1].is_some()) * RIGHT_VALID)
            | (u8::from(self.observed[0] == Some(true)) * LEFT_ON)
            | (u8::from(self.observed[1] == Some(true)) * RIGHT_ON)
            | (u8::from(self.sent[0]) * LEFT_SENT)
            | (u8::from(self.sent[1]) * RIGHT_SENT);
        Packet {
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

/// A bounded two-event frame. Release the opposite input before asserting a side.
#[derive(Default)]
pub struct Dispatch {
    events: [(u32, bool); 2],
    next: usize,
}
impl Dispatch {
    pub fn begin(&mut self, desired: [bool; 2]) {
        self.events = if desired[0] {
            [(1, false), (0, true)]
        } else {
            [(0, false), (1, desired[1])]
        };
        self.next = 0;
    }
    pub fn pop(&mut self) -> Option<(u32, bool)> {
        let event = self.events.get(self.next).copied();
        self.next = (self.next + 1).min(2);
        event
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_releases_opposite_side_before_asserting_and_is_finite() {
        let mut dispatch = Dispatch::default();
        dispatch.begin([true, false]);
        assert_eq!(dispatch.pop(), Some((1, false)));
        assert_eq!(dispatch.pop(), Some((0, true)));
        assert_eq!(dispatch.pop(), None);
        dispatch.begin([false, true]);
        assert_eq!(dispatch.pop(), Some((0, false)));
        assert_eq!(dispatch.pop(), Some((1, true)));
        assert_eq!(dispatch.pop(), None);
        dispatch.begin([false, false]);
        assert_eq!(dispatch.pop(), Some((0, false)));
        assert_eq!(dispatch.pop(), Some((1, false)));
    }
    #[test]
    fn missing_telemetry_or_pause_prevents_readiness() {
        let mut state = State {
            input_active: true,
            telemetry_running: true,
            telemetry_installed: true,
            ..State::default()
        };
        state.observed = [Some(false), None];
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
