use std::fs::File;
use std::io::BufReader;
use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};

#[cfg(windows)]
mod bridge;

#[derive(Parser)]
#[command(
    version,
    about = "StalkShift — open MOZA stalk bridge and USB diagnostics for ETS2 and ATS"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// List connected MOZA stalk HID interfaces (Windows only).
    List,
    /// Record raw input reports; does not send output/feature reports to the device.
    Record {
        /// Interface index shown by list. Run list again after reconnecting.
        #[arg(long)]
        device: usize,
        /// New JSONL file. Existing files are never overwritten.
        #[arg(long)]
        output: PathBuf,
        /// Describe the control positions or movement being recorded.
        #[arg(long, value_parser = nonempty_label)]
        label: String,
        /// Recording duration in seconds (1–3600).
        #[arg(long, default_value_t = 15, value_parser = clap::value_parser!(u64).range(1..=3600))]
        seconds: u64,
    },
    /// Validate a capture and summarize changed raw bytes, without a device.
    Inspect { file: PathBuf },
    /// Decode an existing capture with the measured direct-mode indicator profile.
    DecodeIndicators { file: PathBuf },
    /// Connect the measured direct-mode MOZA controls to the StalkShift game plugin.
    Bridge {
        #[arg(long)]
        device: usize,
        /// Optional time limit for diagnostics. Otherwise runs until Ctrl+C.
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..))]
        seconds: Option<u64>,
    },
}

fn nonempty_label(value: &str) -> Result<String, String> {
    if value.trim().is_empty() || value.len() > 256 {
        return Err("label must contain 1–256 bytes of nonblank text".to_owned());
    }
    Ok(value.to_owned())
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::List => list(),
        Command::Bridge { device, seconds } => {
            #[cfg(windows)]
            {
                bridge::run(device, seconds)
            }
            #[cfg(not(windows))]
            {
                let _ = (device, seconds);
                anyhow::bail!("The game bridge currently requires Windows x64")
            }
        }
        Command::Record {
            device,
            output,
            label,
            seconds,
        } => record(device, output, label, seconds),
        Command::DecodeIndicators { file } => {
            let mut decoder = stalkshift_core::DirectIndicatorDecoder::default();
            println!("Offline direct-mode indicator decode. Initial position: Unknown.");
            let input = BufReader::new(
                File::open(&file).with_context(|| format!("open {}", file.display()))?,
            );
            let summary = stalkshift_capture::visit_reports(input, |elapsed_us, data| {
                if let Some(position) = decoder.feed(data)? {
                    println!("{:9.3} s  {position:?}", elapsed_us as f64 / 1_000_000.0);
                }
                Ok(())
            })?;
            println!(
                "Validated {} reports. Final observed position: {:?}",
                summary.reports,
                decoder.position()
            );
            Ok(())
        }
        Command::Inspect { file } => {
            let summary = stalkshift_capture::inspect(BufReader::new(
                File::open(&file).with_context(|| format!("open {}", file.display()))?,
            ))?;
            println!("Label: {}", summary.label);
            println!(
                "Complete: {} reports, {} changes, {:.3} s",
                summary.reports,
                summary.changes,
                summary.elapsed_us as f64 / 1_000_000.0
            );
            println!("Report lengths: {:?}", summary.report_lengths);
            println!(
                "Changed byte offsets (zero-based): {:?}",
                summary.changed_byte_offsets
            );
            if summary.reports == 0 {
                println!(
                    "No reports received. This does not confirm the device state; try moving a control or another interface."
                );
            }
            Ok(())
        }
    }
}

#[cfg(windows)]
fn list() -> Result<()> {
    let devices = stalkshift_hid::discover()?;
    if devices.is_empty() {
        println!(
            "No MOZA stalk interfaces found (346e:0024). Connect USB and run list again. If connected, check HidHide and the actual device ID."
        );
    }
    for (index, device) in devices.iter().enumerate() {
        let meta = &device.metadata;
        println!(
            "[{index}] {:04x}:{:04x} usage={:04x}:{:04x} interface={} release={:04x} product={:?}",
            meta.vendor_id,
            meta.product_id,
            meta.usage_page,
            meta.usage,
            meta.interface_number,
            meta.release_number,
            meta.product
        );
    }
    Ok(())
}

#[cfg(windows)]
fn record(index: usize, output: PathBuf, label: String, seconds: u64) -> Result<()> {
    use std::fs::OpenOptions;
    use std::io::BufWriter;
    use std::time::Duration;

    let devices = stalkshift_hid::discover()?;
    let device = devices
        .get(index)
        .context("MOZA interface index not found; connect USB and run list")?;
    let handle = device.open()?;
    if let Some(parent) = output.parent().filter(|path| !path.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent)?;
    }
    let file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .with_context(|| {
            format!(
                "create {} (choose a new filename if it exists)",
                output.display()
            )
        })?;
    println!(
        "Recording interface {index}: {label:?} for {seconds} seconds. Move the selected control now."
    );
    let mut writer = BufWriter::new(file);
    let result = stalkshift_hid::record(
        device,
        &handle,
        &mut writer,
        label,
        Duration::from_secs(seconds),
        |elapsed, data| {
            let hex: Vec<_> = data.iter().map(|byte| format!("{byte:02x}")).collect();
            println!("{:9.3} s  {}", elapsed as f64 / 1_000_000.0, hex.join(" "));
        },
    );
    // Preserve buffered evidence on read failure; no End will be written.
    use std::io::Write;
    writer.flush()?;
    let count = result?;
    println!("Saved {count} reports to {}", output.display());
    if count == 0 {
        println!(
            "No input reports received; try moving a control or selecting another listed interface."
        );
    }
    Ok(())
}

#[cfg(not(windows))]
fn list() -> Result<()> {
    anyhow::bail!("HID access currently supports Windows only; inspect works on this platform")
}

#[cfg(not(windows))]
fn record(_: usize, _: PathBuf, _: String, _: u64) -> Result<()> {
    anyhow::bail!("HID access currently supports Windows only; inspect works on this platform")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn record_requires_explicit_device_label_and_output() {
        for arguments in [
            vec!["stalkshift", "record"],
            vec![
                "stalkshift",
                "record",
                "--device",
                "0",
                "--output",
                "test.jsonl",
            ],
            vec![
                "stalkshift",
                "record",
                "--label",
                "left",
                "--output",
                "test.jsonl",
            ],
        ] {
            assert!(Cli::try_parse_from(arguments).is_err());
        }
    }

    #[test]
    fn rejects_invalid_recording_duration_and_blank_labels() {
        for seconds in ["0", "3601", "-1"] {
            assert!(
                Cli::try_parse_from([
                    "stalkshift",
                    "record",
                    "--device",
                    "0",
                    "--output",
                    "x",
                    "--label",
                    "left",
                    "--seconds",
                    seconds
                ])
                .is_err()
            );
        }
        assert!(nonempty_label("  ").is_err());
    }
}
