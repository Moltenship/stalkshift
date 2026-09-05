//! Portable capture format. Reports are raw HIDAPI bytes, not decoded MOZA states.

use std::collections::BTreeSet;
use std::io::{BufRead, Write};

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

pub const SCHEMA_VERSION: u32 = 1;
pub const MAX_REPORT_BYTES: usize = 65_536;
const MAX_LINE_BYTES: usize = 512 * 1024;

/// Deliberately excludes device paths and serial numbers from shareable captures.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetadata {
    pub vendor_id: u16,
    pub product_id: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface_number: i32,
    pub product: Option<String>,
    pub release_number: u16,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Event {
    Header {
        schema: u32,
        tool_version: String,
        label: String,
        device: DeviceMetadata,
        /// Descriptor may be unavailable with the selected backend.
        descriptor: Option<Vec<u8>>,
        descriptor_error: Option<String>,
    },
    Report {
        sequence: u64,
        elapsed_us: u64,
        data: Vec<u8>,
    },
    End {
        elapsed_us: u64,
        reports: u64,
    },
}

pub fn write_event(mut output: impl Write, event: &Event) -> Result<()> {
    serde_json::to_writer(&mut output, event)?;
    output.write_all(b"\n")?;
    Ok(())
}

#[derive(Debug)]
pub struct Summary {
    pub label: String,
    pub reports: u64,
    pub changes: u64,
    pub elapsed_us: u64,
    pub report_lengths: BTreeSet<usize>,
    pub changed_byte_offsets: BTreeSet<usize>,
}

/// Validate incrementally so a long capture need not fit in memory.
/// A missing footer is an error: interrupted captures must not appear complete.
pub fn inspect(mut input: impl BufRead) -> Result<Summary> {
    let mut label = None;
    let mut reports = 0_u64;
    let mut changes = 0;
    let mut elapsed_us = 0;
    let mut previous: Option<Vec<u8>> = None;
    let mut report_lengths = BTreeSet::new();
    let mut changed_byte_offsets = BTreeSet::new();
    let mut ended = false;
    let mut line_number = 0;
    let mut line = Vec::new();
    loop {
        line.clear();
        // Bound a single JSON line before parsing untrusted/shared recordings.
        let bytes = std::io::Read::take(&mut input, (MAX_LINE_BYTES + 1) as u64)
            .read_until(b'\n', &mut line)?;
        if bytes == 0 {
            break;
        }
        line_number += 1;
        ensure!(
            bytes <= MAX_LINE_BYTES,
            "line {line_number}: record too large"
        );
        ensure!(!ended, "line {line_number}: data after capture end");
        let event: Event = serde_json::from_slice(&line)
            .with_context(|| format!("line {line_number}: invalid capture record"))?;
        match event {
            Event::Header {
                schema,
                label: value,
                ..
            } => {
                ensure!(line_number == 1, "line {line_number}: unexpected header");
                ensure!(
                    schema == SCHEMA_VERSION,
                    "unsupported capture schema {schema}"
                );
                ensure!(!value.trim().is_empty(), "capture label is empty");
                label = Some(value);
            }
            Event::Report {
                sequence,
                elapsed_us: timestamp,
                data,
            } => {
                ensure!(label.is_some(), "missing capture header");
                ensure!(
                    sequence == reports,
                    "line {line_number}: out-of-order report"
                );
                ensure!(
                    timestamp >= elapsed_us,
                    "line {line_number}: time moved backwards"
                );
                ensure!(
                    !data.is_empty() && data.len() <= MAX_REPORT_BYTES,
                    "invalid report length"
                );
                if let Some(old) = &previous
                    && old != &data
                {
                    changes += 1;
                    for offset in 0..old.len().max(data.len()) {
                        if old.get(offset) != data.get(offset) {
                            changed_byte_offsets.insert(offset);
                        }
                    }
                }
                report_lengths.insert(data.len());
                previous = Some(data);
                elapsed_us = timestamp;
                reports += 1;
            }
            Event::End {
                elapsed_us: timestamp,
                reports: expected,
            } => {
                ensure!(label.is_some(), "missing capture header");
                ensure!(expected == reports, "footer report count mismatch");
                ensure!(timestamp >= elapsed_us, "footer time moved backwards");
                elapsed_us = timestamp;
                ended = true;
            }
        }
    }
    ensure!(ended, "incomplete capture: missing end record");
    Ok(Summary {
        label: label.context("missing capture header")?,
        reports,
        changes,
        elapsed_us,
        report_lengths,
        changed_byte_offsets,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const HEADER: &str = r#"{"type":"header","schema":1,"tool_version":"test","label":"synthetic","device":{"vendor_id":13422,"product_id":36,"usage_page":1,"usage":4,"interface_number":0,"product":null,"release_number":0},"descriptor":null,"descriptor_error":null}"#;
    const FIRST: &str = r#"{"type":"report","sequence":0,"elapsed_us":10,"data":[1,0,0]}"#;

    fn parse(lines: &[&str]) -> Result<Summary> {
        inspect(Cursor::new(lines.join("\n")))
    }

    #[test]
    fn preserves_repeated_reports_and_detects_changes() {
        let summary = parse(&[
            HEADER,
            FIRST,
            r#"{"type":"report","sequence":1,"elapsed_us":20,"data":[1,0,0]}"#,
            r#"{"type":"report","sequence":2,"elapsed_us":30,"data":[1,4,0,8]}"#,
            r#"{"type":"end","elapsed_us":40,"reports":3}"#,
        ])
        .unwrap();
        assert_eq!(summary.reports, 3);
        assert_eq!(summary.changes, 1);
        assert_eq!(summary.changed_byte_offsets, BTreeSet::from([1, 3]));
        assert_eq!(summary.report_lengths, BTreeSet::from([3, 4]));
    }

    #[test]
    fn accepts_complete_capture_with_no_reports() {
        assert_eq!(
            parse(&[HEADER, r#"{"type":"end","elapsed_us":10,"reports":0}"#])
                .unwrap()
                .reports,
            0
        );
    }

    #[test]
    fn bundled_fixture_matches_documented_example() {
        let summary = inspect(Cursor::new(include_bytes!(
            "../../../fixtures/synthetic-transition.jsonl"
        )))
        .unwrap();
        assert_eq!(summary.reports, 4);
        assert_eq!(summary.changes, 2);
        assert_eq!(summary.changed_byte_offsets, BTreeSet::from([1]));
    }

    #[test]
    fn rejects_incomplete_or_misordered_streams() {
        let bad_streams = [
            vec![HEADER, FIRST],
            vec![FIRST],
            vec![HEADER, HEADER],
            vec![
                HEADER,
                FIRST,
                r#"{"type":"report","sequence":0,"elapsed_us":20,"data":[1]}"#,
            ],
            vec![
                HEADER,
                FIRST,
                r#"{"type":"report","sequence":1,"elapsed_us":9,"data":[1]}"#,
            ],
            vec![
                HEADER,
                FIRST,
                r#"{"type":"end","elapsed_us":20,"reports":2}"#,
            ],
            vec![
                HEADER,
                FIRST,
                r#"{"type":"end","elapsed_us":9,"reports":1}"#,
            ],
            vec![
                HEADER,
                r#"{"type":"end","elapsed_us":20,"reports":0}"#,
                FIRST,
            ],
            vec![
                HEADER,
                r#"{"type":"report","sequence":0,"elapsed_us":1,"data":[]}"#,
            ],
        ];
        for lines in bad_streams {
            assert!(parse(&lines).is_err(), "accepted {lines:?}");
        }
    }

    #[test]
    fn rejects_unknown_schema_and_truncated_json() {
        assert!(parse(&[&HEADER.replace("\"schema\":1", "\"schema\":2")]).is_err());
        assert!(parse(&[HEADER, "{\"type\":"]).is_err());
    }

    #[test]
    fn rejects_oversized_line_before_json_parse() {
        let error = inspect(Cursor::new(vec![b' '; MAX_LINE_BYTES + 1])).unwrap_err();
        assert!(error.to_string().contains("too large"));
    }

    #[test]
    fn writes_newline_delimited_json_with_escaped_labels() {
        let event: Event =
            serde_json::from_str(&HEADER.replace("synthetic", "left\\nright")).unwrap();
        let mut output = Vec::new();
        write_event(&mut output, &event).unwrap();
        assert_eq!(output.iter().filter(|&&byte| byte == b'\n').count(), 1);
        assert!(serde_json::from_slice::<Event>(&output).is_ok());
    }
}
