use crate::DecodeError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum WiperPosition {
    #[default]
    Unknown,
    Mist,
    Off,
    Intermittent,
    Low,
    High,
}

/// The small front-wiper thumbwheel beside MIST/OFF/INT/LO/HI, not the REAR ring.
#[derive(Debug, Default)]
pub struct DirectWiperDecoder {
    position: WiperPosition,
    previous_pulse: u32,
}

impl DirectWiperDecoder {
    pub fn position(&self) -> WiperPosition {
        self.position
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<Option<WiperPosition>, DecodeError> {
        if data.len() != 8 {
            self.reset();
            return Err(DecodeError::UnexpectedReportLength(data.len()));
        }
        let pulse = u32::from_le_bytes(data[..4].try_into().expect("four report bytes")) & 0x3e000;
        if pulse.count_ones() > 1 {
            self.reset();
            return Err(DecodeError::ConflictingWiperPulses);
        }
        let is_new = pulse != 0 && pulse != self.previous_pulse;
        self.previous_pulse = pulse;
        if !is_new {
            return Ok(None);
        }
        let position = match pulse {
            0x02000 => WiperPosition::Mist,
            0x04000 => WiperPosition::Off,
            0x08000 => WiperPosition::Intermittent,
            0x10000 => WiperPosition::Low,
            0x20000 => WiperPosition::High,
            _ => unreachable!("masked single-bit wiper pulse"),
        };
        if position == self.position {
            return Ok(None);
        }
        self.position = position;
        Ok(Some(position))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replays_capture_with_operator_slips_and_final_off_outside_recording() {
        let mut decoder = DirectWiperDecoder::default();
        let mut second_pass = Vec::new();
        let summary = stalkshift_capture::visit_reports(
            std::io::Cursor::new(include_bytes!(
                "../../../fixtures/moza/direct-wiper-wheel.jsonl"
            )),
            |elapsed, data| {
                if let Some(position) = decoder.feed(data)?
                    && elapsed >= 96_000_000
                {
                    second_pass.push(position);
                }
                Ok(())
            },
        )
        .unwrap();
        use WiperPosition::{High, Intermittent, Low, Mist, Off};
        assert_eq!(summary.reports, 614);
        assert_eq!(
            second_pass,
            [Off, Intermittent, Low, High, Low, Intermittent, Off, Mist]
        );
        assert_eq!(
            decoder.position(),
            Mist,
            "do not invent the final OFF outside the capture"
        );
    }
    #[test]
    fn mist_is_a_latched_position_and_unrelated_controls_do_not_cancel_it() {
        let mut decoder = DirectWiperDecoder::default();
        decoder.feed(&[0; 8]).unwrap();
        assert_eq!(decoder.position(), WiperPosition::Unknown);
        decoder.feed(&[4, 0x20, 0, 0, 0, 0, 0, 0]).unwrap();
        decoder.feed(&[128, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(decoder.position(), WiperPosition::Mist);
        assert_eq!(decoder.feed(&[0, 0x20, 0, 0, 0, 0, 0, 0]).unwrap(), None);
        assert!(decoder.feed(&[0, 0x60, 0, 0, 0, 0, 0, 0]).is_err());
        assert_eq!(decoder.position(), WiperPosition::Unknown);
    }
}
