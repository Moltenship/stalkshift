# StalkShift

An open-source Rust bridge between **MOZA Multi-function Stalks** and **Euro Truck Simulator 2** on Windows.

**Status: control bridge prototype for ETS2 1.60.1.7.** Indicators, the light ring, flash/high beams and front wipers are implemented for the measured MOZA direct mode. Indicators, pause/USB recovery, light modes and beam operation have been confirmed in the game. A single MIST sweep is confirmed on the standard Mercedes-Benz New Actros StreamSpace; visual confirmation of the other wiper speeds is still pending. A Rust DLL connects through the official SCS Input and Telemetry APIs. Horn, hazards, cruise and gears are not implemented yet. The first complete version still targets all controls described in [the plan](PLAN.md), including D/N/R/P and optional speed-limit cruise adjustment.

## Try the controls in ETS2

```powershell
cargo build --release --workspace --locked
python scripts/probe_plugin.py target/release/stalkshift_plugin.dll
```

Close ETS2, install the plugin, and start the bridge:

```powershell
.\scripts\install-plugin.ps1 -GameDirectory 'C:\Program Files (x86)\Steam\steamapps\common\Euro Truck Simulator 2'
.\target\release\stalkshift.exe bridge --device 0
```

Enter the truck and wait for `ready=true`, then move each position control to synchronize it. Release the beam lever before pressing it. The indicator, light ring and front-wiper thumbwheel send movement pulses, so each requires re-arming after pause/reconnect. The small front-wiper thumbwheel is beside MIST/OFF/INT/LO/HI on the right stalk; the end ring marked REAR is a different control. See [game integration](docs/game-integration.md) for assignments, disconnect behavior and acceptance checks. Install the matching application and DLL together: this build uses protocol v2 and does not connect to the old indicator-only DLL.

## Try the diagnostics

Requires Windows x64 and the Rust MSVC build prerequisites. The toolchain is pinned in `rust-toolchain.toml`; Cargo fetches locked dependencies. Build:

```powershell
cargo build --release --locked -p stalkshift
.\target\release\stalkshift.exe list
```

Connect the stalks directly through USB. `list` displays only MOZA `346e:0024` interfaces, confirmed on the measured device. Multiple HID collections may appear; choose an explicit index, and list again after reconnecting.

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

Replay the measured direct-mode indicator sequence without hardware:

```powershell
cargo run --locked -p stalkshift -- decode-indicators fixtures/moza/direct-indicators.jsonl
```

The decoder produces right → centre → left → centre twice. The operator corrected the original recording's direction labels during in-game testing; the recorded bytes are unchanged. Zero reports between the 150 ms pulses do not cancel the physical position. At startup/reconnect, position remains unknown until an event is observed.

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
| `stalkshift-core` | Measured direct-mode indicator, light-ring, wiper-wheel and beam decoders |
| `stalkshift-protocol` | Bounded named-pipe protocol, sessions, readiness epochs and input leases |
| `stalkshift-plugin` | Windows x64 SCS Input/Telemetry DLL with bounded input dispatch |

The core tests replay all four reviewed hardware captures, including the operator's documented corrections. Protocol tests cover independent controls, invalid/conflicting commands, session changes, lease expiry and MIST interruption. HID access and IPC waits stay outside game callbacks. The actual DLL is probed against a mock SCS host for ABI layout, callback registration, failure rollback and repeated initialization/shutdown. Remaining game checks are recorded in [the acceptance table](docs/game-integration.md).

## Project scope

First complete version: indicators, lighting, wipers, hazards, horn, cruise controls, D/N/R/P, parking brake and optional speed-limit cruise adjustment. Development proceeds through tested milestones; this diagnostic build is not that full version. ATS is a later target.

MIT licensed; see [third-party notices](THIRD_PARTY_NOTICES.md) for SCS SDK declarations. Independent community project; not affiliated with MOZA or SCS Software. [MOZA Truck Stalk Bridge](https://github.com/JacKJodel23/MOZA-Truck-Stalk-Bridge) is the behavior reference; this implementation is developed independently.
