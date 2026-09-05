use crate::DecodeError;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum LightPosition {
    #[default]
    Unknown,
    Off,
    Parking,
    LowBeam,
}

/// Direct-mode light-ring events measured on the same device as the indicators.
#[derive(Debug, Default)]
pub struct DirectLightDecoder {
    position: LightPosition,
    previous_pulse: u8,
}

impl DirectLightDecoder {
    pub fn position(&self) -> LightPosition {
        self.position
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<Option<LightPosition>, DecodeError> {
        if data.len() != 8 {
            self.reset();
            return Err(DecodeError::UnexpectedReportLength(data.len()));
        }
        let pulse = data[0] & 7;
        if pulse.count_ones() > 1 {
            self.reset();
            return Err(DecodeError::ConflictingLightPulses);
        }
        let is_new = pulse != 0 && pulse != self.previous_pulse;
        self.previous_pulse = pulse;
        if !is_new {
            return Ok(None);
        }
        let position = match pulse {
            1 => LightPosition::Off,
            2 => LightPosition::Parking,
            4 => LightPosition::LowBeam,
            _ => unreachable!("masked single-bit light pulse"),
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
    fn replays_operator_confirmed_light_ring_capture() {
        let mut decoder = DirectLightDecoder::default();
        let mut positions = Vec::new();
        let summary = stalkshift_capture::visit_reports(
            std::io::Cursor::new(include_bytes!(
                "../../../fixtures/moza/direct-light-ring.jsonl"
            )),
            |_, data| {
                if let Some(position) = decoder.feed(data)? {
                    positions.push(position);
                }
                Ok(())
            },
        )
        .unwrap();
        use LightPosition::{LowBeam, Off, Parking};
        assert_eq!(summary.reports, 456);
        assert_eq!(
            positions,
            [
                Parking, LowBeam, Parking, Off, Parking, LowBeam, Parking, Off
            ]
        );
    }

    #[test]
    fn light_ring_ignores_release_and_other_controls_but_rejects_conflicts() {
        let mut decoder = DirectLightDecoder::default();
        decoder.feed(&[0; 8]).unwrap();
        assert_eq!(decoder.position(), LightPosition::Unknown);
        assert_eq!(
            decoder.feed(&[4, 2, 0, 0, 0, 0, 0, 0]).unwrap(),
            Some(LightPosition::LowBeam)
        );
        decoder.feed(&[0; 8]).unwrap();
        assert_eq!(decoder.position(), LightPosition::LowBeam);
        assert!(decoder.feed(&[3, 0, 0, 0, 0, 0, 0, 0]).is_err());
        assert_eq!(decoder.position(), LightPosition::Unknown);
        decoder.feed(&[1, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert_eq!(decoder.position(), LightPosition::Off);
        decoder.reset();
        decoder.feed(&[0; 8]).unwrap();
        assert_eq!(decoder.position(), LightPosition::Unknown);
    }
}
