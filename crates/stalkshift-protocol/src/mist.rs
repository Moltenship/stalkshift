use crate::{MIST_REQUEST, WIPER_MASK};
use std::time::{Duration, Instant};

const OFF: u64 = 1 << 5;
const LOW: u64 = 1 << 7;
const ACK_TIMEOUT: Duration = Duration::from_secs(1);

#[derive(Debug, Default, Clone, Copy)]
enum Phase {
    #[default]
    Idle,
    Resetting,
    Starting,
    Parking,
    Done,
}

/// Input frames can precede the game's simulation update. Keep each phase until
/// the wiper telemetry acknowledges it, rather than sending a one-input-frame tap.
#[derive(Debug, Default)]
pub(crate) struct Mist {
    phase: Phase,
    since: Option<Instant>,
    needs_park: bool,
}

impl Mist {
    pub fn invalidate(&mut self) {
        if self.needs_park {
            if !matches!(self.phase, Phase::Parking) {
                self.phase = Phase::Parking;
                self.since = None;
            }
        } else {
            *self = Self::default();
        }
    }
    fn enter(&mut self, phase: Phase, now: Instant) {
        self.phase = phase;
        self.since = Some(now);
    }

    pub fn apply(&mut self, desired: u64, observed: Option<bool>, now: Instant) -> u64 {
        if self.since.is_none() && self.needs_park {
            self.since = Some(now);
        }
        let requested = desired & MIST_REQUEST != 0;
        let direct_mode = desired & WIPER_MASK & !MIST_REQUEST;
        if direct_mode != 0 || (!requested && !self.needs_park) {
            *self = Self::default();
            return desired;
        }
        if !requested && !matches!(self.phase, Phase::Parking) {
            self.enter(Phase::Parking, now);
        }
        let other_controls = desired & !WIPER_MASK;
        let expired = self
            .since
            .is_some_and(|since| now.saturating_duration_since(since) >= ACK_TIMEOUT);
        let mode = match self.phase {
            Phase::Idle => {
                self.enter(Phase::Resetting, now);
                OFF
            }
            Phase::Resetting => {
                if observed == Some(false) {
                    self.enter(Phase::Starting, now);
                    self.needs_park = true;
                    LOW
                } else {
                    if expired {
                        self.enter(Phase::Done, now);
                    }
                    OFF
                }
            }
            Phase::Starting => {
                if observed == Some(true) || expired {
                    self.enter(Phase::Parking, now);
                    OFF
                } else {
                    LOW
                }
            }
            Phase::Parking => {
                if observed == Some(false) || expired {
                    self.needs_park = false;
                    self.enter(if requested { Phase::Done } else { Phase::Idle }, now);
                }
                OFF
            }
            Phase::Done => OFF,
        };
        other_controls | mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn waits_for_game_updates_and_never_repeats_while_mist_remains_selected() {
        let mut mist = Mist::default();
        let now = Instant::now();
        assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), OFF);
        for _ in 0..20 {
            assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), LOW);
        }
        assert_eq!(mist.apply(MIST_REQUEST, Some(true), now), OFF);
        for _ in 0..20 {
            assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), OFF);
        }
        mist.apply(0, Some(false), now);
        assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), OFF);
        assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), LOW);
    }

    #[test]
    fn reset_ack_is_required_when_entering_from_an_already_running_mode() {
        let mut mist = Mist::default();
        let now = Instant::now();
        for _ in 0..20 {
            assert_eq!(mist.apply(MIST_REQUEST, Some(true), now), OFF);
        }
        assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), LOW);
    }

    #[test]
    fn missing_ack_cancels_and_does_not_restart() {
        let mut mist = Mist::default();
        let now = Instant::now();
        mist.apply(MIST_REQUEST, Some(false), now);
        assert_eq!(mist.apply(MIST_REQUEST, Some(false), now), LOW);
        assert_eq!(
            mist.apply(MIST_REQUEST, Some(false), now + ACK_TIMEOUT),
            OFF
        );
        assert_eq!(
            mist.apply(MIST_REQUEST, Some(false), now + ACK_TIMEOUT * 3),
            OFF
        );
        assert_eq!(
            mist.apply(MIST_REQUEST, Some(false), now + ACK_TIMEOUT * 4),
            OFF
        );
    }
}
