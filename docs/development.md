# Development

[Player guide EN](../README.md) · [Инструкция для игроков RU](../README.ru.md)

StalkShift is a Rust workspace for Windows x64. The toolchain is pinned in `rust-toolchain.toml`; builds use the MSVC target and locked Cargo dependencies. Install Rust and the Visual Studio C++ build tools with a Windows SDK to build from source. Python 3.11 or newer is used only for development checks and packaging.

## Build and verify

Close ETS2, ATS and any running StalkShift bridge before rebuilding, probing the DLL or testing the installer. The mock DLL host must not connect to a live bridge.

```powershell
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo build --release --workspace --locked
python scripts/probe_plugin.py target/release/stalkshift_plugin.dll
python scripts/package-release.py
python scripts/test-installer.py target/dist/StalkShift-1.1.0-windows-x64.zip
```

The installer tests use isolated fake ETS2/ATS folders under `target`, including paths with spaces, game selection, units, independent installs/removal, migration from 1.0, read-only controls, repeated installation, restoration, user edits, rollback and damaged packages. They never edit a real game. The DLL probe exercises both game IDs, rejects unknown IDs, and checks both initialization/shutdown orders. The Windows CI job runs these checks and uploads the player ZIP and its SHA-256 checksum.

## Game support and cruise units

The DLL accepts the official SDK identifiers `eut2` and `ats`, using the shared input ABI and truck telemetry channels. The same bridge and DLL serve both games; run one simulator at a time.

The player installer writes `bin/win_x64/plugins/stalkshift-cruise-unit.txt` containing `kmh` or `mph`. The DLL resolves this relative to the game executable, not its working directory. With no valid setting, ETS2 defaults to km/h and ATS to mph. Both modes require the game to use a five-unit cruise step. SDK speeds remain SI values; only target rounding and expected step acknowledgements differ. Unit selection survives input resets without changing protocol v3.

Install records are separate `install-ets2.json` and `install-ats.json` files under `%LOCALAPPDATA%\StalkShift`. Updating ETS2 migrates the old `install.json` record and preserves its backup references. The old record is retired so a 1.0 uninstaller cannot remove the new installation. Settings files and profiles edited after installation are preserved during uninstall.

Automated ATS checks do not constitute in-game verification. No ATS gameplay acceptance was performed for 1.1.0.

## Components

| Crate | Purpose |
|---|---|
| stalkshift-capture | Read, write and inspect recorded HID reports |
| stalkshift-hid | Windows USB HID enumeration and reading |
| stalkshift-core | Decode the measured MOZA control reports |
| stalkshift-protocol | Bridge messages, input gating and control behavior |
| stalkshift-plugin | SCS telemetry and input DLL |
| stalkshift | Diagnostics and bridge application |

The bridge and plugin use protocol v3. Install matching EXE and DLL builds together. The plugin checks game readiness and input leases; the bridge resets control decoders on session changes. See [integration details and test history](game-integration.md), [hardware observations](hardware-observations.md) and [capture format](hid-capture.md).

## Local installation and diagnostics

The developer installer copies the locally built plugin. End users should use the packaged Install.cmd instead.

```powershell
.\scripts\install-plugin.ps1 -GameDirectory 'C:\Program Files (x86)\Steam\steamapps\common\Euro Truck Simulator 2'
.\target\release\stalkshift.exe list
.\target\release\stalkshift.exe bridge --device 0
```

Choose the actual device index from `list`. The measured MOZA collection is VID 346e / PID 0024, interface 2, usage 0001:0004. The supported mode is `Multi function key switch direct`. Do not assume another mode or firmware has the same layout.

```powershell
.\target\release\stalkshift.exe record --device 0 --label 'indicators centre-left-centre-right-centre' --seconds 15 --output captures/indicators.jsonl
.\target\release\stalkshift.exe inspect captures/indicators.jsonl
.\target\release\stalkshift.exe decode-indicators fixtures/moza/direct-indicators.jsonl
```

Sanitized hardware fixtures exercise ten recorded control sequences. Preserve actual reports, including slips made during capture; do not relabel an unmeasured action. Private captures, save files, profile backups, logs and build output must stay out of Git.

## Release

Set the workspace version, update Cargo.lock and the EN/RU player guides, then run the checks above. Packaging writes `version.json` with the source commit and binary hashes. Create the release package from the final committed source, not a dirty checkout.

Publish the ZIP and SHA256SUMS.txt from the successful Windows CI run for the exact tagged commit. The ZIP includes localized installation, launch and removal scripts, offline player guides, the application and plugin, and license notices. End users need neither Rust nor Python.

Release notes belong in `docs/releases/`. Hardware capture and automated checks do not establish compatibility with every truck or game version. Record user game confirmations separately in the integration history.
