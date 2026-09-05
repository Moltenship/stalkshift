use std::io;
use std::time::{Duration, Instant};

use stalkshift_core::IndicatorPosition;

#[cfg(windows)]
pub mod pipe;

pub const PIPE_NAME: &str = r"\\.\pipe\stalkshift-indicators-v1";
pub const FRAME_SIZE: usize = 32;
pub const LEASE: Duration = Duration::from_millis(600);
pub const IO_TIMEOUT: Duration = Duration::from_millis(300);
pub const INTERVAL: Duration = Duration::from_millis(50);
pub const READY: u8 = 1;
pub const LEFT_VALID: u8 = 2;
pub const RIGHT_VALID: u8 = 4;
pub const LEFT_ON: u8 = 8;
pub const RIGHT_ON: u8 = 16;
pub const LEFT_SENT: u8 = 32;
pub const RIGHT_SENT: u8 = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Status = 1,
    Command = 2,
}

/// Fixed-size request/reply frame; no strings, lengths, queues or allocations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packet {
    pub kind: Kind,
    pub value: u8,
    pub session: u64,
    pub sequence: u64,
    pub epoch: u64,
}

impl Packet {
    pub fn encode(self) -> [u8; FRAME_SIZE] {
        let mut bytes = [0; FRAME_SIZE];
        bytes[..4].copy_from_slice(b"STSF");
        bytes[4..6].copy_from_slice(&1_u16.to_le_bytes());
        bytes[6] = self.kind as u8;
        bytes[7] = self.value;
        bytes[8..16].copy_from_slice(&self.session.to_le_bytes());
        bytes[16..24].copy_from_slice(&self.sequence.to_le_bytes());
        bytes[24..32].copy_from_slice(&self.epoch.to_le_bytes());
        bytes
    }

    pub fn decode(bytes: &[u8; FRAME_SIZE]) -> io::Result<Self> {
        let invalid = || {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "invalid StalkShift protocol frame",
            )
        };
        if &bytes[..4] != b"STSF" || bytes[4..6] != [1, 0] {
            return Err(invalid());
        }
        let kind = match bytes[6] {
            1 => Kind::Status,
            2 => Kind::Command,
            _ => return Err(invalid()),
        };
        if (kind == Kind::Command && bytes[7] > 3) || (kind == Kind::Status && bytes[7] > 127) {
            return Err(invalid());
        }
        let read =
            |offset| u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("fixed field"));
        let packet = Self {
            kind,
            value: bytes[7],
            session: read(8),
            sequence: read(16),
            epoch: read(24),
        };
        if packet.session == 0 || packet.epoch == 0 {
            return Err(invalid());
        }
        Ok(packet)
    }

    pub fn reply(self, position: IndicatorPosition) -> Self {
        Self {
            kind: Kind::Command,
            value: encode_position(position),
            ..self
        }
    }
}

pub fn encode_position(position: IndicatorPosition) -> u8 {
    match position {
        IndicatorPosition::Unknown => 0,
        IndicatorPosition::Centre => 1,
        IndicatorPosition::Left => 2,
        IndicatorPosition::Right => 3,
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
    desired: u8,
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
        }
    }
}

impl InputGate {
    pub fn invalidate(&mut self) {
        self.epoch = self.epoch.wrapping_add(1).max(1);
        self.sequence = None;
        self.last_received = None;
        self.desired = 0;
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
            || packet.value > 3
            || self
                .sequence
                .is_some_and(|sequence| packet.sequence <= sequence)
        {
            return false;
        }
        self.sequence = Some(packet.sequence);
        self.last_received = Some(now);
        self.desired = if self.ready { packet.value } else { 0 };
        true
    }
    pub fn outputs(&mut self, now: Instant) -> [bool; 2] {
        self.expire(now);
        if !self.ready || self.session == 0 {
            return [false; 2];
        }
        [self.desired == 2, self.desired == 3]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> (InputGate, Packet, Instant) {
        let mut gate = InputGate::default();
        gate.connect(42);
        gate.set_ready(true);
        let packet = Packet {
            kind: Kind::Command,
            session: 42,
            epoch: gate.epoch,
            sequence: 0,
            value: 2,
        };
        (gate, packet, Instant::now())
    }
    #[test]
    fn wire_is_little_endian_and_strict() {
        let (_, packet, _) = setup();
        let bytes = packet.encode();
        assert_eq!(Packet::decode(&bytes).unwrap(), packet);
        assert_eq!(&bytes[8..16], &[42, 0, 0, 0, 0, 0, 0, 0]);
        for (offset, value) in [(0, 0), (4, 2), (6, 3), (7, 255), (8, 0)] {
            let mut invalid = bytes;
            invalid[offset] = value;
            assert!(Packet::decode(&invalid).is_err());
        }
    }
    #[test]
    fn pause_requires_new_epoch_and_never_replays_held_input() {
        let (mut gate, mut packet, now) = setup();
        assert!(gate.accept(packet, now));
        assert_eq!(gate.outputs(now), [true, false]);
        gate.set_ready(false);
        gate.set_ready(true);
        packet.sequence += 1;
        assert!(!gate.accept(packet, now));
        assert_eq!(gate.outputs(now), [false; 2]);
        packet.epoch = gate.epoch;
        assert!(gate.accept(packet, now));
    }
    #[test]
    fn heartbeat_expiry_rejects_delayed_packets_until_new_epoch() {
        let (mut gate, mut packet, now) = setup();
        gate.accept(packet, now);
        assert_eq!(gate.outputs(now + LEASE), [false; 2]);
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
        assert_eq!(gate.outputs(now), [false; 2]);
        gate.connect(43);
        assert!(!gate.accept(packet, now));
    }
    #[test]
    fn unknown_and_centre_release_both_inputs() {
        let (mut gate, mut packet, now) = setup();
        for (sequence, value, expected) in [
            (0, 2, [true, false]),
            (1, 0, [false, false]),
            (2, 3, [false, true]),
            (3, 1, [false, false]),
        ] {
            packet.sequence = sequence;
            packet.value = value;
            assert!(gate.accept(packet, now));
            assert_eq!(gate.outputs(now), expected);
        }
    }
}
