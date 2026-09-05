//! Measured MOZA indicator pulse decoding. No game commands are emitted here.

use std::fmt;

mod lights;
pub use lights::{DirectLightDecoder, LightPosition};
mod wipers;
pub use wipers::{DirectWiperDecoder, WiperPosition};
mod beams;
pub use beams::DirectBeamDecoder;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum IndicatorPosition {
    /// No observed position event in this session, or an invalid input.
    #[default]
    Unknown,
    Centre,
    Left,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeError {
    UnexpectedReportLength(usize),
    ConflictingIndicatorPulses,
    ConflictingLightPulses,
    ConflictingWiperPulses,
    ConflictingBeamInputs,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedReportLength(length) => {
                write!(output, "expected 8-byte MOZA report, got {length}")
            }
            Self::ConflictingIndicatorPulses => {
                write!(output, "multiple indicator position pulses in one report")
            }
            Self::ConflictingLightPulses => {
                write!(output, "multiple light-ring position pulses in one report")
            }
            Self::ConflictingWiperPulses => {
                write!(output, "multiple wiper-wheel position pulses in one report")
            }
            Self::ConflictingBeamInputs => {
                write!(output, "conflicting left-lever beam inputs in one report")
            }
        }
    }
}

impl std::error::Error for DecodeError {}

/// Profile for the observed "Multi function key switch direct" device mode.
/// Validated on one device/capture; firmware version remains unknown.
#[derive(Debug, Default)]
pub struct DirectIndicatorDecoder {
    position: IndicatorPosition,
    previous_pulse: u16,
}

impl DirectIndicatorDecoder {
    pub fn position(&self) -> IndicatorPosition {
        self.position
    }

    /// Call on disconnect/reconnect, dropped input, or profile changes.
    pub fn reset(&mut self) {
        self.position = IndicatorPosition::Unknown;
        self.previous_pulse = 0;
    }

    /// Returns a position only when a new pulse changes our known state.
    /// All-zero reports release a pulse, NOT the physical indicator lever.
    /// Unrelated control bits do not affect the indicator state.
    pub fn feed(&mut self, data: &[u8]) -> Result<Option<IndicatorPosition>, DecodeError> {
        if data.len() != 8 {
            self.reset();
            return Err(DecodeError::UnexpectedReportLength(data.len()));
        }
        let pulse = u16::from_le_bytes([data[0], data[1]]) & 0x0380;
        if pulse.count_ones() > 1 {
            self.reset();
            return Err(DecodeError::ConflictingIndicatorPulses);
        }
        let is_new = pulse != 0 && pulse != self.previous_pulse;
        self.previous_pulse = pulse;
        if !is_new {
            return Ok(None);
        }
        let position = match pulse {
            0x0080 => IndicatorPosition::Right,
            0x0100 => IndicatorPosition::Centre,
            0x0200 => IndicatorPosition::Left,
            _ => unreachable!("masked, nonzero single-bit pulse"),
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

    const ZERO: [u8; 8] = [0; 8];
    const LEFT: [u8; 8] = [0, 2, 0, 0, 0, 0, 0, 0];
    const CENTRE: [u8; 8] = [0, 1, 0, 0, 0, 0, 0, 0];
    const RIGHT: [u8; 8] = [128, 0, 0, 0, 0, 0, 0, 0];

    #[test]
    fn idle_reports_do_not_invent_initial_centre() {
        let mut decoder = DirectIndicatorDecoder::default();
        assert_eq!(decoder.feed(&ZERO).unwrap(), None);
        assert_eq!(decoder.position(), IndicatorPosition::Unknown);
    }

    #[test]
    fn pulse_release_does_not_cancel_indicator() {
        let mut decoder = DirectIndicatorDecoder::default();
        assert_eq!(decoder.feed(&LEFT).unwrap(), Some(IndicatorPosition::Left));
        assert_eq!(decoder.feed(&LEFT).unwrap(), None);
        assert_eq!(decoder.feed(&ZERO).unwrap(), None);
        assert_eq!(decoder.position(), IndicatorPosition::Left);
        assert_eq!(
            decoder.feed(&CENTRE).unwrap(),
            Some(IndicatorPosition::Centre)
        );
        assert_eq!(decoder.feed(&ZERO).unwrap(), None);
        assert_eq!(decoder.position(), IndicatorPosition::Centre);
    }

    #[test]
    fn reconnect_forgets_stale_position_and_accepts_new_pulse() {
        let mut decoder = DirectIndicatorDecoder::default();
        decoder.feed(&RIGHT).unwrap();
        decoder.reset();
        decoder.feed(&ZERO).unwrap();
        assert_eq!(decoder.position(), IndicatorPosition::Unknown);
        assert_eq!(
            decoder.feed(&RIGHT).unwrap(),
            Some(IndicatorPosition::Right)
        );
    }

    #[test]
    fn invalid_data_invalidates_position() {
        for data in [&[1_u8][..], &[128, 2, 0, 0, 0, 0, 0, 0][..]] {
            let mut decoder = DirectIndicatorDecoder::default();
            decoder.feed(&LEFT).unwrap();
            assert!(decoder.feed(data).is_err());
            assert_eq!(decoder.position(), IndicatorPosition::Unknown);
        }
    }

    #[test]
    fn unrelated_buttons_can_be_combined_with_position_events() {
        let mut decoder = DirectIndicatorDecoder::default();
        assert_eq!(
            decoder.feed(&[1, 2, 255, 255, 0, 0, 0, 0]).unwrap(),
            Some(IndicatorPosition::Left)
        );
        assert_eq!(decoder.feed(&[1, 0, 255, 255, 0, 0, 0, 0]).unwrap(), None);
        assert_eq!(decoder.position(), IndicatorPosition::Left);
    }

    #[test]
    fn accepts_adjacent_events_without_zero_report() {
        let mut decoder = DirectIndicatorDecoder::default();
        decoder.feed(&LEFT).unwrap();
        assert_eq!(
            decoder.feed(&RIGHT).unwrap(),
            Some(IndicatorPosition::Right)
        );
    }

    #[test]
    fn decodes_confirmed_hardware_sequence_without_false_cancellations() {
        let input = std::io::Cursor::new(include_bytes!(
            "../../../fixtures/moza/direct-indicators.jsonl"
        ));
        let mut decoder = DirectIndicatorDecoder::default();
        let mut positions = Vec::new();
        let summary = stalkshift_capture::visit_reports(input, |_, data| {
            if let Some(position) = decoder.feed(data)? {
                positions.push(position);
            }
            Ok(())
        })
        .unwrap();
        use IndicatorPosition::{Centre, Left, Right};
        assert_eq!(summary.reports, 306);
        assert_eq!(
            positions,
            [Right, Centre, Left, Centre, Right, Centre, Left, Centre]
        );
        assert_eq!(decoder.position(), Centre);
    }
}
