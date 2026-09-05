use std::io;
use std::time::{Duration, Instant};

mod controls;
pub use controls::*;
mod actions;
mod mist;

#[cfg(windows)]
pub mod pipe;

pub const PIPE_NAME: &str = r"\\.\pipe\stalkshift-controls-v3";
pub const FRAME_SIZE: usize = 56;
pub const LEASE: Duration = Duration::from_millis(600);
pub const IO_TIMEOUT: Duration = Duration::from_millis(300);
pub const INTERVAL: Duration = Duration::from_millis(50);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Status = 1,
    Command = 2,
}

/// Fixed-size request/reply frame; no strings, lengths, queues or allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    pub kind: Kind,
    pub motion: [i32; 4],
    pub value: u64,
    pub session: u64,
    pub sequence: u64,
    pub epoch: u64,
}

impl Packet {
    pub fn encode(self) -> [u8; FRAME_SIZE] {
        let mut bytes = [0; FRAME_SIZE];
        bytes[..4].copy_from_slice(b"STSF");
        bytes[4..6].copy_from_slice(&3_u16.to_le_bytes());
        bytes[6] = self.kind as u8;
        bytes[8..16].copy_from_slice(&self.value.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.session.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[32..40].copy_from_slice(&self.epoch.to_le_bytes());
        for (index, value) in self.motion.iter().enumerate() {
            bytes[40 + index * 4..44 + index * 4].copy_from_slice(&value.to_le_bytes());
        }
        bytes
    }

    pub fn decode(bytes: &[u8; FRAME_SIZE]) -> io::Result<Self> {
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid StalkShift protocol frame",
            )
        };
        if &bytes[..4] != b"STSF" || bytes[4..6] != [3, 0] || bytes[7] != 0 {
            return Err(invalid());
        }
        let kind = match bytes[6] {
            1 => Kind::Status,
            2 => Kind::Command,
            _ => return Err(invalid()),
        };
        let value = u64::from_le_bytes(bytes[8..16].try_into().expect("fixed value field"));
        if (kind == Kind::Command && !valid_inputs(value))
            || (kind == Kind::Status && value & !STATUS_MASK != 0)
        {
            return Err(invalid());
        }
        let read =
            |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed field"));
        let motion = std::array::from_fn(|index| {
            i32::from_le_bytes(
                bytes[40 + index * 4..44 + index * 4]
                    .try_into()
                    .expect("numeric field"),
            )
        });
        if kind == Kind::Command && motion != UNKNOWN_MOTION {
            return Err(invalid());
        }
        let packet = Self {
            motion,
            kind,
            value,
            session: read(16),
            sequence: read(24),
            epoch: read(32),
        };
        if packet.session == 0 || packet.epoch == 0 {
            return Err(invalid());
        }
        Ok(packet)
    }

    pub fn reply(self, inputs: u64) -> Self {
        Self {
            kind: Kind::Command,
            motion: UNKNOWN_MOTION,
            value: inputs,
            ..self
        }
    }
}

/// This gate runs inside the DLL. The host app cannot bypass its readiness/lease.
#[derive(Debug)]
pub struct InputGate {
    pub session: u64,
    pub epoch: u64,
    ready: bool,
    sequence: Option<u64>,
    last_received: Option<Instant>,
    desired: u64,
    mist: mist::Mist,
    wipers_on: Option<bool>,
    actions: actions::Actions,
    parking: Option<bool>,
    motion: [i32; 4],
}

impl Default for InputGate {
    fn default() -> Self {
        Self {
            session: 0,
            epoch: 1,
            ready: false,
            sequence: None,
            last_received: None,
            desired: 0,
            mist: mist::Mist::default(),
            wipers_on: None,
            actions: actions::Actions::default(),
            parking: None,
            motion: UNKNOWN_MOTION,
        }
    }
}

impl InputGate {
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        self.sequence = None;
        self.last_received = None;
        self.desired = 0;
        self.mist.invalidate();
        self.actions = actions::Actions::default();
    }
    pub fn connect(&mut self, session: u64) {
        self.session = session;
        self.invalidate();
    }
    pub fn disconnect(&mut self) {
        self.session = 0;
        self.invalidate();
    }
    pub fn set_ready(&mut self, ready: bool) {
        if self.ready != ready {
            self.ready = ready;
            self.invalidate();
        }
    }
    pub fn ready(&self) -> bool {
        self.ready
    }
    pub fn expire(&mut self, now: Instant) {
        if self
            .last_received
            .is_some_and(|received| now.saturating_duration_since(received) >= LEASE)
        {
            self.invalidate();
        }
    }
    pub fn accept(&mut self, packet: Packet, now: Instant) -> bool {
        self.expire(now);
        if packet.kind != Kind::Command
            || packet.session == 0
            || packet.session != self.session
            || packet.epoch != self.epoch
            || !valid_inputs(packet.value)
            || self
                .sequence
                .is_some_and(|sequence| packet.sequence <= sequence)
        {
            return false;
        }
        self.sequence = Some(packet.sequence);
        self.last_received = Some(now);
        self.desired = if self.ready { packet.value } else { 0 };
        if self.desired & MIST_REQUEST == 0 {
            self.mist.invalidate();
        }
        true
    }
    pub fn observe_wipers(&mut self, observed: Option<bool>) {
        self.wipers_on = observed;
    }
    pub fn observe_driving(&mut self, parking: Option<bool>, motion: [i32; 4]) {
        self.parking = parking;
        self.motion = motion;
    }
    pub fn automatic(&self) -> bool {
        self.actions.automatic
    }
    /// Call once per input frame. MIST waits for wiper telemetry between phases.
    pub fn outputs(&mut self, now: Instant) -> [bool; INPUT_COUNT] {
        self.expire(now);
        let desired = if self.ready && self.session != 0 {
            self.desired
        } else {
            0
        };
        let desired = self.mist.apply(desired, self.wipers_on, now);
        let desired = self.actions.apply(desired, self.parking, self.motion, now);
        std::array::from_fn(|index| desired & (1 << index) != 0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mist_waits_for_ack_then_stays_off_until_a_new_request() {
        let (mut gate, mut packet, now) = setup();
        packet.value = MIST_REQUEST | (1 << 4);
        gate.accept(packet, now);
        gate.observe_wipers(Some(false));
        assert!(gate.outputs(now)[5]);
        assert_eq!(
            gate.outputs(now),
            std::array::from_fn(|index| index == 4 || index == 7)
        );
        gate.observe_wipers(Some(true));
        assert!(gate.outputs(now)[5]);
        gate.observe_wipers(Some(false));
        for _ in 0..20 {
            packet.sequence += 1;
            gate.accept(packet, now);
            assert_eq!(
                gate.outputs(now),
                std::array::from_fn(|index| index == 4 || index == 5)
            );
        }
        packet.sequence += 1;
        packet.value = 1 << 5;
        gate.accept(packet, now);
        packet.sequence += 1;
        packet.value = MIST_REQUEST;
        gate.accept(packet, now);
        assert!(gate.outputs(now)[5]);
        assert!(gate.outputs(now)[7], "fresh MIST entry must trigger again");
    }

    #[test]
    fn interrupted_mist_parks_at_next_callback_without_replaying_the_wipe() {
        for interrupt in [0, 1, 2] {
            let (mut gate, mut packet, now) = setup();
            packet.value = MIST_REQUEST;
            gate.accept(packet, now);
            gate.observe_wipers(Some(false));
            assert!(gate.outputs(now)[5]);
            assert!(gate.outputs(now)[7]);
            match interrupt {
                0 => gate.disconnect(),
                1 => gate.set_ready(false),
                _ => gate.expire(now + LEASE),
            }
            assert_eq!(
                gate.outputs(now + LEASE),
                std::array::from_fn(|index| index == 5)
            );
            assert_eq!(gate.outputs(now + LEASE), [false; INPUT_COUNT]);
        }
    }

    #[test]
    fn conflicting_modes_are_rejected_and_all_groups_expire_together() {
        let (mut gate, mut packet, now) = setup();
        packet.value = 1 | light_inputs(stalkshift_core::LightPosition::LowBeam);
        assert!(gate.accept(packet, now));
        assert_eq!(
            gate.outputs(now),
            std::array::from_fn(|index| index == 0 || index == 4)
        );
        for invalid in [
            3,
            (1 << 2) | (1 << 4),
            1 << 63,
            DRIVE | REVERSE,
            CRUISE_UP | CRUISE_DOWN,
            MIST_REQUEST | (1 << 5),
        ] {
            packet.value = invalid;
            packet.sequence += 1;
            assert!(Packet::decode(&packet.encode()).is_err());
            assert!(!gate.accept(packet, now));
        }
        assert_eq!(gate.outputs(now + LEASE), [false; INPUT_COUNT]);
    }

    #[test]
    fn telemetry_distinguishes_unknown_off_and_sent_input() {
        let mut telemetry = [None; CHANNEL_COUNT];
        telemetry[0] = Some(false);
        telemetry[2] = Some(true);
        let mut sent = [false; INPUT_COUNT];
        sent[4] = true;
        let value = status_value(true, &telemetry, &sent);
        assert_eq!(observed(value, 0), Some(false));
        assert_eq!(observed(value, 1), None);
        assert_eq!(observed(value, 2), Some(true));
        assert_ne!(value & sent_bit(4), 0);
        assert_eq!(value & !STATUS_MASK, 0);
    }

    fn setup() -> (InputGate, Packet, Instant) {
        let mut gate = InputGate::default();
        gate.connect(42);
        gate.set_ready(true);
        let packet = Packet {
            motion: [i32::MIN; 4],
            kind: Kind::Command,
            session: 42,
            epoch: gate.epoch,
            sequence: 0,
            value: 1,
        };
        (gate, packet, Instant::now())
    }
    #[test]
    fn wire_is_little_endian_and_strict() {
        let (_, packet, _) = setup();
        let bytes = packet.encode();
        assert_eq!(Packet::decode(&bytes).unwrap(), packet);
        assert_eq!(&bytes[16..24], &[42, 0, 0, 0, 0, 0, 0, 0]);
        for (offset, value) in [(0, 0), (4, 1), (6, 3), (7, 255), (8, 3), (12, 1), (16, 0)] {
            let mut invalid = bytes;
            invalid[offset] = value;
            assert!(Packet::decode(&invalid).is_err());
        }
    }
    #[test]
    fn numerical_telemetry_and_wide_status_survive_wire_round_trip() {
        let (_, mut packet, _) = setup();
        packet.kind = Kind::Status;
        packet.value = AUTO_ENABLED | sent_bit(20) | READY;
        packet.motion = [25000, 22222, 21000, -1];
        assert_eq!(Packet::decode(&packet.encode()).unwrap(), packet);
        let reply = packet.reply(HORN);
        assert_eq!(reply.motion, UNKNOWN_MOTION);
        assert_eq!(Packet::decode(&reply.encode()).unwrap(), reply);
        packet.kind = Kind::Command;
        packet.value = HORN;
        assert!(Packet::decode(&packet.encode()).is_err());
    }
    #[test]
    fn connection_expiry_cancels_horn_gear_and_automatic_adjustment() {
        let (mut gate, mut packet, now) = setup();
        gate.observe_driving(Some(false), [22000, 25000, 22000, 10]);
        packet.value = HORN | DRIVE | AUTO_TOGGLE;
        assert!(gate.accept(packet, now));
        let output = gate.outputs(now);
        assert!(output[11] && output[14] && output[19]);
        assert!(gate.automatic());
        assert_eq!(gate.outputs(now + LEASE), [false; INPUT_COUNT]);
        assert!(!gate.automatic());
        assert!(!gate.accept(packet, now + LEASE));
    }
    #[test]
    fn pause_requires_new_epoch_and_never_replays_held_input() {
        let (mut gate, mut packet, now) = setup();
        assert!(gate.accept(packet, now));
        assert_eq!(gate.outputs(now), std::array::from_fn(|index| index == 0));
        gate.set_ready(false);
        gate.set_ready(true);
        packet.sequence += 1;
        assert!(!gate.accept(packet, now));
        assert_eq!(gate.outputs(now), [false; INPUT_COUNT]);
        packet.epoch = gate.epoch;
        assert!(gate.accept(packet, now));
    }
    #[test]
    fn heartbeat_expiry_rejects_delayed_packets_until_new_epoch() {
        let (mut gate, mut packet, now) = setup();
        gate.accept(packet, now);
        assert_eq!(gate.outputs(now + LEASE), [false; INPUT_COUNT]);
        packet.sequence += 1;
        assert!(!gate.accept(packet, now + LEASE));
        packet.epoch = gate.epoch;
        packet.value = 1;
        assert!(gate.accept(packet, now + LEASE));
    }
    #[test]
    fn reconnect_and_reordered_messages_cannot_restore_stale_inputs() {
        let (mut gate, packet, now) = setup();
        assert!(gate.accept(packet, now));
        assert!(!gate.accept(packet, now));
        gate.disconnect();
        assert_eq!(gate.outputs(now), [false; INPUT_COUNT]);
        gate.connect(43);
        assert!(!gate.accept(packet, now));
    }
    #[test]
    fn unknown_and_centre_release_both_inputs() {
        let (mut gate, mut packet, now) = setup();
        for (sequence, value, expected) in [
            (0, 1, std::array::from_fn(|index| index == 0)),
            (1, 0, [false; INPUT_COUNT]),
            (2, 2, std::array::from_fn(|index| index == 1)),
            (3, 0, [false; INPUT_COUNT]),
        ] {
            packet.sequence = sequence;
            packet.value = value;
            assert!(gate.accept(packet, now));
            assert_eq!(gate.outputs(now), expected);
        }
    }
}
