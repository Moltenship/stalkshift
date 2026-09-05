use crate::DecodeError;

/// Measured held inputs, unlike the position pulses for the rings and indicators.
/// A new session requires a neutral report before accepting presses.
#[derive(Debug, Default)]
pub struct DirectBeamDecoder {
    armed: bool,
    flash: bool,
    high_beam_pressed: bool,
}

impl DirectBeamDecoder {
    pub fn flash(&self) -> bool {
        self.flash
    }
    pub fn high_beam_pressed(&self) -> bool {
        self.high_beam_pressed
    }
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn feed(&mut self, data: &[u8]) -> Result<bool, DecodeError> {
        if data.len() != 8 {
            self.reset();
            return Err(DecodeError::UnexpectedReportLength(data.len()));
        }
        let bits = data[0] & 0x38;
        if bits.count_ones() > 1 {
            self.reset();
            return Err(DecodeError::ConflictingBeamInputs);
        }
        let previous = (self.flash, self.high_beam_pressed);
        if bits == 0 || bits == 0x10 {
            self.armed = true;
            self.flash = false;
            self.high_beam_pressed = false;
        } else if self.armed {
            self.flash = bits == 0x20;
            self.high_beam_pressed = bits == 0x08;
        }
        Ok(previous != (self.flash, self.high_beam_pressed))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn replay_preserves_holds_and_releases_instead_of_latching_beam_pulses() {
        let mut decoder = DirectBeamDecoder::default();
        let mut transitions = Vec::new();
        let summary = stalkshift_capture::visit_reports(
            std::io::Cursor::new(include_bytes!(
                "../../../fixtures/moza/direct-beam-lever.jsonl"
            )),
            |_, data| {
                if decoder.feed(data)? {
                    transitions.push((decoder.flash(), decoder.high_beam_pressed()));
                }
                Ok(())
            },
        )
        .unwrap();
        assert_eq!(summary.reports, 610);
        assert_eq!(
            transitions,
            [
                (true, false),
                (false, false),
                (false, true),
                (false, false),
                (false, true),
                (false, false),
                (true, false),
                (false, false),
                (false, true),
                (false, false)
            ]
        );
    }
    #[test]
    fn startup_held_press_is_suppressed_until_release_and_new_press() {
        let mut decoder = DirectBeamDecoder::default();
        decoder.feed(&[8, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(!decoder.high_beam_pressed());
        decoder.feed(&[0; 8]).unwrap();
        decoder.feed(&[8, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(decoder.high_beam_pressed());
        assert!(!decoder.feed(&[8, 0, 0, 0, 0, 0, 0, 0]).unwrap());
        decoder.reset();
        decoder.feed(&[0x20, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(!decoder.flash());
        decoder.feed(&[0x10, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        decoder.feed(&[0x24, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(decoder.flash());
        decoder.feed(&[4, 0, 0, 0, 0, 0, 0, 0]).unwrap();
        assert!(!decoder.flash(), "zero beam bits release the held flash");
        assert!(decoder.feed(&[0x28, 0, 0, 0, 0, 0, 0, 0]).is_err());
    }
}
