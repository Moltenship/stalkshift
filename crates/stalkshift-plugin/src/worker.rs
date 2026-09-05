use crate::state::shared;
use stalkshift_protocol::{INTERVAL, Kind, PIPE_NAME, pipe};
use std::io;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use tokio::net::windows::named_pipe::ClientOptions;

struct Worker {
    stop: Arc<AtomicBool>,
    thread: JoinHandle<()>,
}
static WORKER: Mutex<Option<Worker>> = Mutex::new(None);

fn next_session() -> u64 {
    static COUNT: AtomicU64 = AtomicU64::new(1);
    let clock = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    (clock ^ (u64::from(std::process::id()) << 32) ^ COUNT.fetch_add(1, Ordering::Relaxed)).max(1)
}

pub fn start() -> io::Result<()> {
    let mut slot = WORKER
        .lock()
        .map_err(|_| io::Error::other("worker state poisoned"))?;
    if slot.is_some() {
        return Err(io::Error::other("worker already started"));
    }
    let runtime = pipe::runtime()?;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let thread = std::thread::Builder::new()
        .name("stalkshift-ipc".into())
        .spawn(move || {
            runtime.block_on(async {
                while !worker_stop.load(Ordering::Relaxed) {
                    if let Ok(mut client) = ClientOptions::new().open(PIPE_NAME) {
                        let session = next_session();
                        {
                            let Ok(mut state) = shared().lock() else {
                                return;
                            };
                            state.gate.connect(session);
                        }
                        let mut sequence = 0_u64;
                        while !worker_stop.load(Ordering::Relaxed) {
                            let status = match shared().lock() {
                                Ok(mut state) => state.status(sequence),
                                Err(_) => return,
                            };
                            if pipe::send(&mut client, status).await.is_err() {
                                break;
                            }
                            let Ok(command) = pipe::receive(&mut client).await else {
                                break;
                            };
                            if command.kind != Kind::Command
                                || command.session != status.session
                                || command.sequence != status.sequence
                                || command.epoch != status.epoch
                            {
                                break;
                            }
                            if let Ok(mut state) = shared().lock() {
                                state.refresh();
                                // A pause can advance the epoch after status was sent; reject that old response.
                                state.gate.accept(command, Instant::now());
                            } else {
                                return;
                            }
                            sequence = sequence.wrapping_add(1);
                            tokio::time::sleep(INTERVAL).await;
                        }
                    }
                    if let Ok(mut state) = shared().lock() {
                        state.gate.disconnect();
                    }
                    tokio::time::sleep(INTERVAL).await;
                }
            });
            if let Ok(mut state) = shared().lock() {
                state.gate.disconnect();
            }
        })?;
    *slot = Some(Worker { stop, thread });
    Ok(())
}

pub fn stop() {
    let worker = WORKER
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .take();
    if let Some(worker) = worker {
        worker.stop.store(true, Ordering::Relaxed);
        // Every operation has a deadline; join before DLL unload, never detach DLL code.
        let _ = worker.thread.join();
    }
}
