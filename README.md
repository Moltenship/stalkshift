# StalkShift

An open-source Rust bridge between **MOZA Multi-function Stalks** and **Euro Truck Simulator 2** on Windows.

**Status: hardware discovery milestone.** The repository currently provides a working HID recording CLI and offline capture validation. USB discovery and report capture have been exercised on one real MOZA device; see [hardware observations](docs/hardware-observations.md). It does **not yet control ETS2**, contain a game plugin, or have a verified MOZA button map. The first complete version targets all controls described in [the plan](PLAN.md), including D/N/R/P and optional speed-limit cruise adjustment.

## Try the diagnostics

Requires Windows x64 and the Rust MSVC build prerequisites. The toolchain is pinned in `rust-toolchain.toml`; Cargo fetches locked dependencies. Build:

```powershell
cargo build --release --locked -p stalkshift
.\target\release\stalkshift.exe list
```

Connect the stalks directly through USB. `list` displays only MOZA `346e:0024` interfaces. This ID is a starting point from the existing community bridge and must be confirmed on actual hardware. Multiple HID collections may appear; choose an explicit index, and list again after reconnecting.

```powershell
.\target\release\stalkshift.exe record --device 0 --label "indicators: centre-left-centre-right-centre" --seconds 15 --output captures/indicators-01.jsonl
.\target\release\stalkshift.exe inspect captures/indicators-01.jsonl
```

Recording begins immediately. Move the named control during the recording. The recorder sends no output or feature reports and makes no changes to Pit House, drivers or game configuration. It preserves all received input reports, including duplicates. Existing recordings are never overwritten.

Without a device:

```powershell
cargo run --locked -p stalkshift -- inspect fixtures/synthetic-transition.jsonl
```

Expected: 4 reports, 2 changes, changed byte offset `{1}`. The fixture is synthetic and carries **no real MOZA mapping**.

See [the capture procedure](docs/hid-capture.md) for the full measurement checklist, file format and limitations. Raw captures stay in ignored `captures/`; review evidence before deliberately adding hardware fixtures to the repository.

## Development

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

| Crate | Responsibility |
|---|---|
| `stalkshift` | CLI commands and recording workflow |
| `stalkshift-hid` | Windows HID discovery and read-only report capture |
| `stalkshift-capture` | Versioned JSONL format and streaming offline validation |

Next: record the real device, implement its decoder, and prove SCS semantic input with a minimal Rust DLL. The proposed game bridge will use the official SCS Input and Telemetry APIs. HID access and IPC waits will stay outside game callbacks.

## Project scope

First complete version: indicators, lighting, wipers, hazards, horn, cruise controls, D/N/R/P, parking brake and optional speed-limit cruise adjustment. Development proceeds through tested milestones; this diagnostic build is not that full version. ATS is a later target.

MIT licensed. Independent community project; not affiliated with MOZA or SCS Software. [MOZA Truck Stalk Bridge](https://github.com/JacKJodel23/MOZA-Truck-Stalk-Bridge) is the behavior reference; this implementation is developed independently.
