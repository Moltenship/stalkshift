use std::ffi::CString;
use std::io::Write;
use std::time::{Duration, Instant};

use anyhow::{Context, Result, ensure};
use hidapi::{HidApi, HidDevice, MAX_REPORT_DESCRIPTOR_SIZE};
use stalkshift_capture::{DeviceMetadata, Event, MAX_REPORT_BYTES, SCHEMA_VERSION, write_event};

use crate::{MOZA_STALK_PRODUCT_ID, MOZA_VENDOR_ID};

pub struct Device {
    /// Kept local; never included in a capture file.
    path: CString,
    pub metadata: DeviceMetadata,
}

/// Only enumerate the intended MOZA product. Sort paths to stabilize list indices
/// while topology is unchanged; users must list again after reconnecting.
pub fn discover() -> Result<Vec<Device>> {
    let api = HidApi::new().context("initialize Windows HID")?;
    let mut devices: Vec<_> = api
        .device_list()
        .filter(|device| {
            device.vendor_id() == MOZA_VENDOR_ID && device.product_id() == MOZA_STALK_PRODUCT_ID
        })
        .map(|device| Device {
            path: device.path().to_owned(),
            metadata: DeviceMetadata {
                vendor_id: device.vendor_id(),
                product_id: device.product_id(),
                usage_page: device.usage_page(),
                usage: device.usage(),
                interface_number: device.interface_number(),
                product: device.product_string().map(str::to_owned),
                release_number: device.release_number(),
            },
        })
        .collect();
    devices.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(devices)
}

impl Device {
    pub fn open(&self) -> Result<HidDevice> {
        HidApi::new()?.open_path(&self.path).context(
            "open selected MOZA interface; reconnect and run list again, and check HidHide access",
        )
    }
}

/// Records every received report, including duplicates, with monotonic timestamps.
/// Errors intentionally leave a file without End, recognizable as incomplete.
pub fn record(
    device: &Device,
    handle: &HidDevice,
    mut output: impl Write,
    label: String,
    duration: Duration,
    on_change: impl Fn(u64, &[u8]),
) -> Result<u64> {
    ensure!(!label.trim().is_empty(), "label must not be empty");
    ensure!(!duration.is_zero(), "duration must be positive");
    let mut descriptor_buffer = vec![0; MAX_REPORT_DESCRIPTOR_SIZE];
    let (descriptor, descriptor_error) = match handle.get_report_descriptor(&mut descriptor_buffer)
    {
        Ok(size) => (Some(descriptor_buffer[..size].to_vec()), None),
        Err(error) => (None, Some(error.to_string())),
    };
    write_event(
        &mut output,
        &Event::Header {
            schema: SCHEMA_VERSION,
            tool_version: env!("CARGO_PKG_VERSION").to_owned(),
            label,
            device: device.metadata.clone(),
            descriptor,
            descriptor_error,
        },
    )?;
    output.flush()?;

    let start = Instant::now();
    let mut buffer = vec![0; MAX_REPORT_BYTES];
    let mut previous = Vec::new();
    let mut reports = 0;
    let mut last_flush = Instant::now();
    while start.elapsed() < duration {
        let remaining_ms = duration
            .saturating_sub(start.elapsed())
            .as_millis()
            .clamp(1, 100) as i32;
        let size = handle
            .read_timeout(&mut buffer, remaining_ms)
            .context("HID read failed; capture is incomplete (device may have disconnected)")?;
        if size == 0 {
            continue;
        }
        let data = &buffer[..size];
        let elapsed_us = start.elapsed().as_micros() as u64;
        write_event(
            &mut output,
            &Event::Report {
                sequence: reports,
                elapsed_us,
                data: data.to_vec(),
            },
        )?;
        reports += 1;
        if data != previous {
            on_change(elapsed_us, data);
            previous.clear();
            previous.extend_from_slice(data);
        }
        if last_flush.elapsed() >= Duration::from_secs(1) {
            output.flush()?;
            last_flush = Instant::now();
        }
    }
    write_event(
        &mut output,
        &Event::End {
            elapsed_us: start.elapsed().as_micros() as u64,
            reports,
        },
    )?;
    output.flush()?;
    Ok(reports)
}
