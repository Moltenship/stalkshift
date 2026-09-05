use anyhow::{Context, Result, ensure};
use stalkshift_core::{
    DirectBeamDecoder, DirectIndicatorDecoder, DirectLightDecoder, DirectWiperDecoder,
};
use stalkshift_protocol::{
    CHANNEL_NAMES, FLASH_INPUT, HIGH_BEAM_INPUT, INPUT_NAMES, Kind, PIPE_NAME, Packet, READY,
    indicator_inputs, light_inputs, observed, pipe, sent_bit, wiper_inputs,
};
use std::io;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

const HID_LEASE: Duration = Duration::from_secs(1);

fn print_status(status: Packet) {
    let telemetry: Vec<_> = CHANNEL_NAMES
        .iter()
        .enumerate()
        .map(|(index, name)| {
            let state = match observed(status.value, index) {
                None => "unknown",
                Some(true) => "on",
                Some(false) => "off",
            };
            format!("{name}={state}")
        })
        .collect();
    let sent: Vec<_> = INPUT_NAMES
        .iter()
        .enumerate()
        .filter(|(index, _)| status.value & sent_bit(*index) != 0)
        .map(|(_, name)| *name)
        .collect();
    println!(
        "Game: ready={} {} | plugin inputs: {}",
        status.value & READY != 0,
        telemetry.join(" "),
        if sent.is_empty() {
            "none".into()
        } else {
            sent.join(", ")
        }
    );
}

#[derive(Default)]
struct BridgeState {
    decoder: DirectIndicatorDecoder,
    lights: DirectLightDecoder,
    wipers: DirectWiperDecoder,
    beams: DirectBeamDecoder,
    last_report: Option<Instant>,
    binding: Option<(u64, u64)>,
}
impl BridgeState {
    fn reset(&mut self) {
        self.decoder.reset();
        self.lights.reset();
        self.wipers.reset();
        self.beams.reset();
        self.last_report = None;
    }
    fn disconnected(&mut self) {
        self.reset();
        self.binding = None;
    }
    fn feed(&mut self, bytes: &[u8], now: Instant) -> Result<bool> {
        if self
            .last_report
            .is_some_and(|last| now.saturating_duration_since(last) >= HID_LEASE)
        {
            self.reset();
        }
        self.last_report = Some(now);
        let indicators = self.decoder.feed(bytes)?.is_some();
        let lights = self.lights.feed(bytes)?.is_some();
        let wipers = self.wipers.feed(bytes)?.is_some();
        let beams = self.beams.feed(bytes)?;
        Ok(indicators || lights || wipers || beams)
    }
    fn reply(&mut self, status: Packet, now: Instant) -> Packet {
        let binding = Some((status.session, status.epoch));
        if self.binding != binding {
            self.reset();
            self.binding = binding;
            println!(
                "Game connection/state changed. Move each control to synchronize its position."
            );
        }
        if self
            .last_report
            .is_none_or(|last| now.saturating_duration_since(last) >= HID_LEASE)
        {
            self.reset();
        }
        let desired = if status.value & READY != 0 {
            indicator_inputs(self.decoder.position())
                | light_inputs(self.lights.position())
                | wiper_inputs(self.wipers.position())
                | (u32::from(self.beams.flash()) * FLASH_INPUT)
                | (u32::from(self.beams.high_beam_pressed()) * HIGH_BEAM_INPUT)
        } else {
            0
        };
        status.reply(desired)
    }
}

pub fn run(index: usize, seconds: Option<u64>) -> Result<()> {
    let devices = stalkshift_hid::discover()?;
    let selected = devices
        .get(index)
        .context("MOZA interface not found; run list")?;
    let identity = selected.identity();
    let first_handle = selected.open()?;
    let state = Arc::new(Mutex::new(BridgeState::default()));
    let stop = Arc::new(AtomicBool::new(false));
    let start = Instant::now();
    pipe::runtime()?.block_on(async {
        let mut server = pipe::server(PIPE_NAME).context("create StalkShift pipe; another bridge may already be running")?;
        let reader_state = state.clone(); let reader_stop = stop.clone();
        let reader = std::thread::Builder::new().name("stalkshift-hid".into()).spawn(move || {
            let mut handle = Some(first_handle);
            let mut buffer = vec![0; stalkshift_capture::MAX_REPORT_BYTES];
            while !reader_stop.load(Ordering::Relaxed) {
                if handle.is_none() {
                    if let Ok(devices) = stalkshift_hid::discover() {
                        // Match the original interface shape, not an enumeration index that can shift.
                        let matching: Vec<_> = devices.iter().filter(|device| device.identity() == identity).collect();
                        if matching.len() == 1 { handle = matching[0].open().ok(); }
                    }
                    if handle.is_none() { std::thread::sleep(Duration::from_millis(200)); continue; }
                    println!("MOZA reconnected. Move each control to synchronize its position.");
                }
                match handle.as_ref().expect("handle opened above").read_timeout(&mut buffer, 50) {
                    Ok(0) => {},
                    Ok(size) => {
                        let Ok(mut state) = reader_state.lock() else { break };
                        match state.feed(&buffer[..size], Instant::now()) {
                            Ok(true) => println!("Stalk: indicator={:?} lights={:?} wipers={:?} flash={} high-beam-press={}", state.decoder.position(), state.lights.position(), state.wipers.position(), state.beams.flash(), state.beams.high_beam_pressed()),
                            Ok(false) => {},
                            Err(error) => { state.reset(); eprintln!("Invalid HID input: {error}"); }
                        }
                    }
                    Err(error) => {
                        if let Ok(mut state) = reader_state.lock() { state.reset(); }
                        handle = None; eprintln!("MOZA disconnected/read failed: {error}");
                    }
                }
            }
            if let Ok(mut state) = reader_state.lock() { state.reset(); }
        })?;
        println!("StalkShift is running. Start ETS2 and enter the truck, then move each control to synchronize.");
        println!("Indicators, light modes and front wipers are enabled. Ctrl+C stops the bridge; the plugin releases inputs on connection loss.");
        let expired = || seconds.is_some_and(|seconds| start.elapsed() >= Duration::from_secs(seconds));
        let result: Result<()> = async {
            while !expired() {
                match tokio::time::timeout(Duration::from_millis(100), server.connect()).await {
                    Err(_) => continue,
                    Ok(result) => result.context("accept game plugin connection")?,
                }
                println!("Game plugin connected.");
                let mut previous_status = None;
                let mut previous_sequence = None;
                let mut connection_session = None;
                while !expired() {
                    let exchange: Result<()> = async {
                        let status = pipe::receive(&mut server).await?;
                        ensure!(status.kind == Kind::Status, "expected plugin status");
                        ensure!(connection_session.is_none_or(|session| session == status.session), "session changed inside connection");
                        ensure!(previous_sequence.is_none_or(|sequence| status.sequence > sequence), "out-of-order plugin status");
                        connection_session = Some(status.session); previous_sequence = Some(status.sequence);
                        if previous_status != Some(status.value) {
                            print_status(status);
                            previous_status = Some(status.value);
                        }
                        let reply = state.lock().map_err(|_| io::Error::other("HID state poisoned"))?.reply(status, Instant::now());
                        pipe::send(&mut server, reply).await?;
                        Ok(())
                    }.await;
                    if let Err(error) = exchange { eprintln!("Game connection reset: {error}"); break; }
                }
                if let Ok(mut state) = state.lock() { state.disconnected(); }
                let _ = server.disconnect();
            }
            Ok(())
        }.await;
        stop.store(true, Ordering::Relaxed);
        reader.join().map_err(|_| io::Error::other("HID reader failed"))?;
        result
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn controls_latch_independently_and_connection_change_clears_every_group() {
        let mut state = BridgeState::default();
        let now = Instant::now();
        let mut status = Packet {
            kind: Kind::Status,
            value: READY,
            session: 1,
            epoch: 1,
            sequence: 0,
        };
        assert_eq!(state.reply(status, now).value, 0);
        state.feed(&[4, 0, 0, 0, 0, 0, 0, 0], now).unwrap();
        state.feed(&[0; 8], now).unwrap();
        assert_eq!(state.reply(status, now).value, 1 << 4);
        state.feed(&[0, 2, 0, 0, 0, 0, 0, 0], now).unwrap();
        assert_eq!(state.reply(status, now).value, (1 << 4) | 1);
        status.epoch += 1;
        assert_eq!(state.reply(status, now).value, 0);
        state.feed(&[0, 1, 0, 0, 0, 0, 0, 0], now).unwrap();
        assert_eq!(
            state.reply(status, now).value,
            0,
            "indicator re-arm must not restore stale lights"
        );
    }
    #[test]
    fn handshake_and_usb_silence_require_new_physical_event() {
        let mut state = BridgeState::default();
        let now = Instant::now();
        let status = Packet {
            kind: Kind::Status,
            value: READY,
            session: 1,
            epoch: 1,
            sequence: 0,
        };
        state.feed(&[0, 2, 0, 0, 0, 0, 0, 0], now).unwrap();
        assert_eq!(state.reply(status, now).value, 0);
        state.feed(&[0, 2, 0, 0, 0, 0, 0, 0], now).unwrap();
        assert_eq!(state.reply(status, now).value, 1);
        assert_eq!(state.reply(status, now + HID_LEASE).value, 0);
        state.feed(&[0; 8], now + HID_LEASE).unwrap();
        assert_eq!(state.reply(status, now + HID_LEASE).value, 0);
    }
}
