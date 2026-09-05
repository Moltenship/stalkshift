use crate::*;
use std::time::{Duration, Instant};

const PULSE: Duration = Duration::from_millis(150);
const REPEAT: Duration = Duration::from_millis(300);
const ACK: Duration = Duration::from_secs(1);
const MOMENTARY: u64 = HAZARD
    | GEAR_MASK
    | PARKING
    | CRUISE_TOGGLE
    | CRUISE_RESUME
    | CRUISE_UP
    | CRUISE_DOWN
    | CRUISE_PULL
    | AUTO_TOGGLE;

#[derive(Debug, Default)]
pub(crate) struct Actions {
    previous: u64,
    pulses: [Option<Instant>; INPUT_COUNT],
    repeat: Option<Instant>,
    park_wait: Option<Instant>,
    release_park_wait: Option<Instant>,
    parking_quiet_until: Option<Instant>,
    pub automatic: bool,
    auto_target: Option<i32>,
    auto_pending: Option<(i32, Instant, i32)>,
    auto_blocked: bool,
    last_cruise: Option<i32>,
}
impl Actions {
    fn pulse(&mut self, mask: u64, now: Instant) {
        if mask & PARKING != 0 {
            // Let an earlier toggle release and reach telemetry before deciding
            // whether the next requested brake state needs another toggle.
            self.parking_quiet_until = Some(now + REPEAT);
        }
        for (index, deadline) in self.pulses.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                *deadline = Some(now + PULSE);
            }
        }
    }
    fn clear(&mut self, mask: u64) {
        for (index, deadline) in self.pulses.iter_mut().enumerate() {
            if mask & (1 << index) != 0 {
                *deadline = None;
            }
        }
    }
    pub fn apply(
        &mut self,
        desired: u64,
        parking: Option<bool>,
        motion: [i32; 4],
        now: Instant,
    ) -> u64 {
        let rising = desired & !self.previous;
        let gear = desired & GEAR_MASK;
        let previous_gear = self.previous & GEAR_MASK;
        if gear != previous_gear {
            self.clear(NEUTRAL | DRIVE | REVERSE);
            self.park_wait = None;
            if gear == PARK_REQUEST {
                self.release_park_wait = None;
                self.pulse(NEUTRAL, now);
                self.park_wait = Some(now);
            } else {
                self.pulse(gear, now);
                if gear == 0 {
                    self.release_park_wait = None;
                } else if previous_gear == PARK_REQUEST {
                    self.release_park_wait = Some(now);
                }
            }
        }
        // A deliberate manual brake press takes precedence over selector work.
        if rising & PARKING != 0 {
            self.park_wait = None;
            self.release_park_wait = None;
        }
        let brake_settled = self.parking_quiet_until.is_none_or(|until| now >= until);
        if let Some(start) = self.park_wait {
            if gear != PARK_REQUEST || now.duration_since(start) >= ACK {
                self.park_wait = None;
            } else if brake_settled {
                if parking == Some(false) {
                    self.pulse(PARKING, now);
                }
                if parking.is_some() {
                    self.park_wait = None;
                }
            }
        }
        if let Some(start) = self.release_park_wait {
            if now.duration_since(start) >= ACK {
                self.release_park_wait = None;
            } else if brake_settled {
                if parking == Some(true) {
                    self.pulse(PARKING, now);
                }
                if parking.is_some() {
                    self.release_park_wait = None;
                }
            }
        }
        self.pulse(
            rising & (HAZARD | PARKING | CRUISE_TOGGLE | CRUISE_RESUME),
            now,
        );
        if rising & CRUISE_PULL != 0 && motion[0] != UNKNOWN_NUMBER {
            self.clear(CRUISE_TOGGLE | CRUISE_RESUME);
            self.pulse(
                if motion[0] > 0 {
                    CRUISE_TOGGLE
                } else {
                    CRUISE_RESUME
                },
                now,
            );
        }
        let manual = desired & (CRUISE_UP | CRUISE_DOWN);
        if manual != 0 {
            self.automatic = false;
            if manual != self.previous & (CRUISE_UP | CRUISE_DOWN)
                || self
                    .repeat
                    .is_none_or(|then| now.duration_since(then) >= REPEAT)
            {
                self.clear(CRUISE_UP | CRUISE_DOWN);
                self.pulse(manual, now);
                self.repeat = Some(now);
            }
        } else if self.previous & (CRUISE_UP | CRUISE_DOWN) != 0 {
            self.repeat = None;
        }
        if rising & (CRUISE_TOGGLE | CRUISE_PULL | CRUISE_RESUME) != 0 {
            self.automatic = false;
        }
        if rising & AUTO_TOGGLE != 0 {
            self.automatic = !self.automatic;
            self.clear(CRUISE_UP | CRUISE_DOWN);
            self.auto_pending = None;
            self.auto_blocked = false;
            self.auto_target = None;
            self.last_cruise = None;
        }
        if self.automatic {
            self.adjust(motion, now);
        } else {
            self.auto_pending = None;
            self.last_cruise = None;
        }
        self.previous = desired;
        let mut output = desired & !MOMENTARY;
        for (index, deadline) in self.pulses.iter_mut().enumerate() {
            if deadline.is_some_and(|until| now < until) {
                output |= 1 << index;
            } else {
                *deadline = None;
            }
        }
        output
    }
    fn adjust(&mut self, motion: [i32; 4], now: Instant) {
        let [cruise, limit, _, _] = motion;
        if cruise <= 0 || !(1..=100_000).contains(&limit) {
            self.clear(CRUISE_UP | CRUISE_DOWN);
            self.auto_pending = None;
            self.auto_target = None;
            self.last_cruise = None;
            return;
        }
        // ETS2 is configured for a 5 km/h grid. Round the SDK limit to km/h,
        // then choose a reachable grid value at or below that limit.
        let target = ((i64::from(limit) * 36 + 5000) / 10000 / 5 * 50000 / 36) as i32;
        if target <= 0 {
            return;
        }
        if self.auto_target != Some(target) {
            self.auto_target = Some(target);
            self.auto_blocked = false;
        }
        let direction = (target - cruise).signum();
        if let Some((before, sent, sign)) = self.auto_pending {
            if (cruise - before).abs() < 100 {
                if now.duration_since(sent) >= ACK {
                    self.auto_blocked = true;
                    self.auto_pending = None;
                }
                return;
            }
            self.auto_pending = None;
            // Opposite/excessive movement is likely a manual adjustment. A
            // refused or overshooting step must never oscillate indefinitely.
            let delta = cruise - before;
            if delta.signum() != sign || delta.abs() > 1700 {
                self.automatic = false;
                self.clear(CRUISE_UP | CRUISE_DOWN);
                return;
            }
            if direction != sign && (target - cruise).abs() > 250 {
                self.auto_blocked = true;
            }
        } else if self
            .last_cruise
            .is_some_and(|last| (last - cruise).abs() > 100)
        {
            self.automatic = false;
            self.clear(CRUISE_UP | CRUISE_DOWN);
            return;
        }
        self.last_cruise = Some(cruise);
        if self.auto_blocked || (target - cruise).abs() <= 250 {
            return;
        }
        if self
            .repeat
            .is_some_and(|last| now.duration_since(last) < REPEAT)
        {
            return;
        }
        self.clear(CRUISE_UP | CRUISE_DOWN);
        self.pulse(
            if direction > 0 {
                CRUISE_UP
            } else {
                CRUISE_DOWN
            },
            now,
        );
        self.repeat = Some(now);
        self.auto_pending = Some((cruise, now, direction));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn entering_park_never_releases_an_applied_brake_or_repeats_a_toggle() {
        let now = Instant::now();
        let mut a = Actions::default();
        assert_eq!(
            a.apply(PARK_REQUEST, Some(true), UNKNOWN_MOTION, now),
            NEUTRAL
        );
        assert_eq!(
            a.apply(PARK_REQUEST, Some(true), UNKNOWN_MOTION, now + ACK),
            0
        );
        a.apply(NEUTRAL, Some(false), UNKNOWN_MOTION, now + ACK);
        assert_eq!(
            a.apply(PARK_REQUEST, Some(false), UNKNOWN_MOTION, now + ACK * 2),
            NEUTRAL | PARKING
        );
        assert_eq!(
            a.apply(PARK_REQUEST, Some(false), UNKNOWN_MOTION, now + ACK * 3),
            0
        );
    }
    #[test]
    fn leaving_park_releases_once_into_each_lower_position() {
        let now = Instant::now();
        for gear in [REVERSE, NEUTRAL, DRIVE] {
            let mut a = Actions::default();
            a.apply(PARK_REQUEST, Some(true), UNKNOWN_MOTION, now);
            assert_eq!(
                a.apply(gear, Some(true), UNKNOWN_MOTION, now + ACK),
                gear | PARKING
            );
            assert_eq!(a.apply(gear, Some(true), UNKNOWN_MOTION, now + ACK * 2), 0);
        }
    }
    #[test]
    fn lower_position_does_not_toggle_an_off_brake_or_invent_a_park_exit() {
        let now = Instant::now();
        let mut a = Actions::default();
        assert_eq!(a.apply(REVERSE, Some(true), UNKNOWN_MOTION, now), REVERSE);
        a.apply(PARK_REQUEST, Some(true), UNKNOWN_MOTION, now + ACK);
        assert_eq!(
            a.apply(REVERSE, Some(false), UNKNOWN_MOTION, now + ACK * 2),
            REVERSE
        );
        assert_eq!(
            a.apply(REVERSE, Some(true), UNKNOWN_MOTION, now + ACK * 3),
            0
        );
    }
    #[test]
    fn park_exit_waits_for_missing_telemetry_but_has_a_deadline() {
        let now = Instant::now();
        let mut a = Actions::default();
        a.apply(PARK_REQUEST, Some(true), UNKNOWN_MOTION, now);
        assert_eq!(a.apply(REVERSE, None, UNKNOWN_MOTION, now + ACK), REVERSE);
        assert_eq!(
            a.apply(REVERSE, Some(true), UNKNOWN_MOTION, now + ACK + REPEAT),
            PARKING
        );
        a.apply(PARK_REQUEST, Some(true), UNKNOWN_MOTION, now + ACK * 2);
        a.apply(REVERSE, None, UNKNOWN_MOTION, now + ACK * 3);
        assert_eq!(
            a.apply(REVERSE, Some(true), UNKNOWN_MOTION, now + ACK * 4),
            0
        );
    }
    #[test]
    fn rapid_park_exit_waits_for_the_earlier_brake_toggle_to_settle() {
        let now = Instant::now();
        let mut a = Actions::default();
        assert_eq!(
            a.apply(PARK_REQUEST, Some(false), UNKNOWN_MOTION, now),
            NEUTRAL | PARKING
        );
        a.apply(
            REVERSE,
            Some(false),
            UNKNOWN_MOTION,
            now + Duration::from_millis(10),
        );
        // Old press has finished, but the release request must still wait.
        assert_eq!(
            a.apply(
                REVERSE,
                Some(true),
                UNKNOWN_MOTION,
                now + Duration::from_millis(200)
            ),
            0
        );
        assert_eq!(
            a.apply(REVERSE, Some(true), UNKNOWN_MOTION, now + REPEAT),
            PARKING
        );
        assert_eq!(a.apply(REVERSE, Some(false), UNKNOWN_MOTION, now + ACK), 0);
    }
    #[test]
    fn held_toggle_is_once_and_speed_holds_have_release_intervals() {
        let now = Instant::now();
        let mut a = Actions::default();
        assert_eq!(a.apply(HAZARD, None, UNKNOWN_MOTION, now), HAZARD);
        assert_eq!(a.apply(HAZARD, None, UNKNOWN_MOTION, now + ACK), 0);
        assert_eq!(
            a.apply(CRUISE_UP, None, UNKNOWN_MOTION, now + ACK * 2),
            CRUISE_UP
        );
        assert_eq!(
            a.apply(CRUISE_UP, None, UNKNOWN_MOTION, now + ACK * 2 + PULSE),
            0
        );
        assert_eq!(
            a.apply(CRUISE_UP, None, UNKNOWN_MOTION, now + ACK * 2 + REPEAT),
            CRUISE_UP
        );
    }
    #[test]
    fn cruise_pull_requires_telemetry_and_chooses_resume_or_cancel() {
        let now = Instant::now();
        assert_eq!(
            Actions::default().apply(CRUISE_PULL, None, UNKNOWN_MOTION, now),
            0
        );
        assert_eq!(
            Actions::default().apply(CRUISE_PULL, None, [0, 0, 0, 0], now),
            CRUISE_RESUME
        );
        assert_eq!(
            Actions::default().apply(CRUISE_PULL, None, [20000, 0, 0, 0], now),
            CRUISE_TOGGLE
        );
    }
    #[test]
    fn auto_waits_for_ack_and_stops_on_refusal_or_manual_command() {
        let now = Instant::now();
        let mut a = Actions::default();
        let motion = [20000, 25000, 20000, 10];
        assert_eq!(a.apply(AUTO_TOGGLE, None, motion, now), CRUISE_UP);
        assert_eq!(a.apply(0, None, motion, now + REPEAT), 0);
        assert_eq!(a.apply(0, None, motion, now + ACK * 2), 0);
        assert_eq!(a.apply(0, None, motion, now + ACK * 3), 0);
        a.apply(CRUISE_DOWN, None, motion, now + ACK * 4);
        assert!(!a.automatic);
    }
    #[test]
    fn auto_never_enables_cruise_or_acts_without_a_limit() {
        let now = Instant::now();
        for motion in [
            [0, 25000, 20000, 10],
            [20000, UNKNOWN_NUMBER, 20000, 10],
            UNKNOWN_MOTION,
        ] {
            assert_eq!(Actions::default().apply(AUTO_TOGGLE, None, motion, now), 0);
        }
    }
    #[test]
    fn auto_tracks_acknowledged_steps_then_stops_at_the_limit() {
        let now = Instant::now();
        let mut a = Actions::default();
        assert_eq!(
            a.apply(AUTO_TOGGLE, None, [22222, 25000, 22000, 10], now),
            CRUISE_UP
        );
        assert_eq!(
            a.apply(0, None, [23611, 25000, 22000, 10], now + REPEAT),
            CRUISE_UP
        );
        assert_eq!(
            a.apply(0, None, [25000, 25000, 22000, 10], now + REPEAT * 2),
            0
        );
        assert_eq!(
            a.apply(0, None, [25000, 25000, 22000, 10], now + ACK * 2),
            0
        );
        // An external keyboard/wheel target change yields control to the user.
        assert_eq!(
            a.apply(0, None, [23611, 25000, 22000, 10], now + ACK * 3),
            0
        );
        assert!(!a.automatic);
    }
    #[test]
    fn auto_does_not_oscillate_after_overshoot_and_can_be_disabled_immediately() {
        let now = Instant::now();
        let mut a = Actions::default();
        assert_eq!(
            a.apply(AUTO_TOGGLE, None, [24300, 25000, 22000, 10], now),
            CRUISE_UP
        );
        assert_eq!(a.apply(0, None, [25689, 25000, 22000, 10], now + REPEAT), 0);
        assert_eq!(
            a.apply(0, None, [25689, 25000, 22000, 10], now + ACK * 2),
            0
        );
        let mut a = Actions::default();
        a.apply(AUTO_TOGGLE, None, [22000, 25000, 22000, 10], now);
        a.apply(
            0,
            None,
            [22000, 25000, 22000, 10],
            now + Duration::from_millis(10),
        );
        assert_eq!(
            a.apply(
                AUTO_TOGGLE,
                None,
                [22000, 25000, 22000, 10],
                now + Duration::from_millis(20)
            ),
            0
        );
        assert!(!a.automatic);
    }
}
