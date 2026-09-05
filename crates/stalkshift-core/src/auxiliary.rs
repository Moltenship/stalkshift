use crate::DecodeError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Selector {
    #[default]
    Unknown,
    Drive,
    Neutral,
    Reverse,
    Park,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct AuxiliaryState {
    pub selector: Selector,
    pub horn: bool,
    pub parking_press: bool,
    pub hazard_press: bool,
    pub cruise_toggle: bool,
    pub cruise_pull: bool,
    pub cruise_up: bool,
    pub cruise_down: bool,
    pub automatic_toggle: bool,
}

/// Independent neutral arming prevents one already-held control from blocking
/// another. REAR additionally requires a measured OFF position before a press.
#[derive(Debug, Default)]
pub struct DirectAuxiliaryDecoder {
    state: AuxiliaryState,
    armed: u32,
    rear_off: Option<bool>,
    rear_held: bool,
}

impl DirectAuxiliaryDecoder {
    pub fn state(&self) -> AuxiliaryState {
        self.state
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn feed(&mut self, data: &[u8]) -> Result<bool, DecodeError> {
        if data.len() != 8 {
            self.reset();
            return Err(DecodeError::UnexpectedReportLength(data.len()));
        }
        let bits = u32::from_le_bytes(data[..4].try_into().expect("four bytes"));
        if (bits & 0xf00000).count_ones() > 1
            || (bits & 0xc00).count_ones() > 1
            || (bits & 0xe000000).count_ones() > 1
        {
            self.reset();
            return Err(DecodeError::ConflictingAuxiliaryInputs);
        }
        let previous = self.state;
        self.armed |= !bits;
        let held = bits & self.armed;
        match bits & 0xf00000 {
            0x100000 => self.state.selector = Selector::Drive,
            0x200000 => self.state.selector = Selector::Neutral,
            0x400000 => self.state.selector = Selector::Reverse,
            0x800000 => self.state.selector = Selector::Park,
            _ => {}
        }
        // Capture context at press onset; changing context while held must not
        // turn an initially unknown or upper spring action into a hazard press.
        if bits & 0x1000 == 0 {
            self.state.hazard_press = false;
        } else if !self.rear_held && held & 0x1000 != 0 && self.rear_off == Some(true) {
            self.state.hazard_press = true;
        }
        self.rear_held = bits & 0x1000 != 0;
        if bits & 0x400 != 0 {
            self.rear_off = Some(true);
        }
        if bits & 0x800 != 0 {
            self.rear_off = Some(false);
        }
        self.state.horn = held & 0x40000 != 0;
        self.state.parking_press = held & 0x80000 != 0;
        self.state.cruise_toggle = held & 0x1000000 != 0;
        self.state.cruise_down = held & 0x2000000 != 0;
        self.state.cruise_up = held & 0x4000000 != 0;
        self.state.cruise_pull = held & 0x8000000 != 0;
        self.state.automatic_toggle = held & 0x40 != 0;
        Ok(previous != self.state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn replay(data: &[u8]) -> Vec<AuxiliaryState> {
        let mut decoder = DirectAuxiliaryDecoder::default();
        let mut states = Vec::new();
        stalkshift_capture::visit_reports(std::io::Cursor::new(data), |_, bytes| {
            if decoder.feed(bytes)? {
                states.push(decoder.state());
            }
            Ok(())
        })
        .unwrap();
        states
    }
    #[test]
    fn measured_holds_and_accidental_overtravel_remain_distinct() {
        let states = replay(include_bytes!(
            "../../../fixtures/moza/direct-right-pull.jsonl"
        ));
        assert_eq!(states.iter().filter(|s| s.horn).count(), 6);
        assert_eq!(states.iter().filter(|s| s.parking_press).count(), 1);
        assert!(!states.last().unwrap().horn);
        let states = replay(include_bytes!(
            "../../../fixtures/moza/direct-right-main.jsonl"
        ));
        assert_eq!(states.iter().filter(|s| s.parking_press).count(), 2);
        assert_eq!(states.last().unwrap().selector, Selector::Drive);
    }
    #[test]
    fn cruise_and_left_switch_replay() {
        let states = replay(include_bytes!(
            "../../../fixtures/moza/direct-cruise-directions.jsonl"
        ));
        assert_eq!(states.iter().filter(|s| s.cruise_pull).count(), 2);
        assert_eq!(states.iter().filter(|s| s.cruise_up).count(), 2);
        assert_eq!(states.iter().filter(|s| s.cruise_down).count(), 2);
        assert_eq!(
            replay(include_bytes!(
                "../../../fixtures/moza/direct-cruise-on-off.jsonl"
            ))
            .iter()
            .filter(|s| s.cruise_toggle)
            .count(),
            3
        );
        assert_eq!(
            replay(include_bytes!(
                "../../../fixtures/moza/direct-left-switch.jsonl"
            ))
            .iter()
            .filter(|s| s.automatic_toggle)
            .count(),
            3
        );
    }
    #[test]
    fn rear_needs_off_context_and_never_triggers_from_upper() {
        let states = replay(include_bytes!(
            "../../../fixtures/moza/direct-rear-ring.jsonl"
        ));
        assert_eq!(
            states.iter().filter(|s| s.hazard_press).count(),
            2,
            "first lower press has unknown startup context"
        );
        let mut d = DirectAuxiliaryDecoder::default();
        d.feed(&[0, 0, 4, 1, 0, 0, 0, 0]).unwrap();
        assert!(!d.state().horn && !d.state().cruise_toggle);
        d.feed(&[0; 8]).unwrap();
        d.feed(&[0, 0, 4, 1, 0, 0, 0, 0]).unwrap();
        assert!(d.state().horn && d.state().cruise_toggle);
        d.reset();
        assert_eq!(d.state(), AuxiliaryState::default());
    }
}
