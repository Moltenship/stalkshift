# ETS2 control integration

This prototype implements indicators, the three-position light ring, flash/high beams and front wipers. The supported device profile is the measured MOZA "Multi function key switch direct" mode. New light/wiper acceptance is in progress; the rest of the first-version feature set remains in the plan.

## Build and install

Build with the pinned Windows x64 Rust toolchain and validate the actual DLL:

```powershell
cargo build --release --workspace --locked
python scripts/probe_plugin.py target/release/stalkshift_plugin.dll
```

Close ETS2 completely, then install the separate plugin:

```powershell
.\scripts\install-plugin.ps1 -GameDirectory 'C:\Program Files (x86)\Steam\steamapps\common\Euro Truck Simulator 2'
```

The installer copies only `stalkshift_plugin.dll` into `bin/win_x64/plugins`, validates the copied checksum, and backs up an existing StalkShift DLL into the local ignored `backups/` directory. It does not overwrite other plugins or change `controls.sii`. If Windows denies writing to the game directory, run this installation command with sufficient permissions; the bridge itself runs as the normal user.

Distributions of the DLL must include `THIRD_PARTY_NOTICES.md` and `third-party/scs-sdk-LICENSE.txt` alongside the project's `LICENSE`.

## Run

```powershell
.\target\release\stalkshift.exe list
.\target\release\stalkshift.exe bridge --device 0
```

Start ETS2, select the intended profile, acknowledge its SDK plugin notification if shown, and enter the truck. Wait for the bridge to report `ready=true`. Move each position control to establish its state; the first movement of the indicator does not establish the light or wiper position. Release the beam lever before making a new press. Turn the truck's electrics on before testing lamps and wipers.

The plugin registers `lblinkerh`, `rblinkerh`, `lightoff`, `lightpark`, `lighton`, `wipers0` through `wipers3`, `lighthorn` and `hblight`. These semantic mixes were found in both local profiles of ETS2 1.60.1.7. The official SCS SDK identifies a `semantical.*` reference as evidence of likely support; actual mode behavior is checked in the game. Registration alone does not establish compatibility.

The bridge displays six telemetry channels separately from the eleven virtual inputs: logical indicators, parking lights, low beams, wipers enabled, and high beams. The official SDK exposes only a bool for wipers, so this cannot confirm speed or the number of completed sweeps. Hardware assignments come from the [reviewed captures](hardware-observations.md), not from guessed USB button numbers.

| Physical control | Requested game input |
|---|---|
| Left / centre / right indicator | `lblinkerh` / neither / `rblinkerh` |
| Light ring OFF / parking / low beams | `lightoff` / `lightpark` / `lighton` |
| Front thumbwheel OFF / INT / LO / HI | `wipers0` / `wipers1` / `wipers2` / `wipers3` |
| Front thumbwheel MIST | Confirm OFF, request low continuous `wipers2`, wait for enabled telemetry, then request OFF; visual single-sweep acceptance required |
| Left lever towards driver | Hold `lighthorn` until release |
| Left lever away from driver | `hblight` press/release; game handles the toggle |

MIST is consumed once while the thumbwheel remains there. A new observed non-MIST command re-arms it. Its DLL state machine first requests OFF, then uses low continuous speed to avoid an intermittent delay, waits for enabled telemetry and requests OFF again. Each acknowledgement phase is bounded to one second; a missing acknowledgement cancels the attempt without repeating it. Pause, disconnect or lease expiry retains bounded OFF cleanup for available input callbacks. Telemetry acknowledgement is not by itself proof of exactly one complete animation cycle on every truck or frame rate.

Avoid mapping the same physical stalk buttons directly to indicators in ETS2 or enabling Pit House's keyboard adaptation at the same time. The installer does not change these settings. Other wheel and keyboard bindings remain in place; their behavior with a registered held-indicator input must also be checked in the game.

## Disconnects, pause and startup

- MOZA reports movement pulses rather than persistent lever positions in the measured mode. Startup position is unknown until a position pulse arrives.
- New pipe sessions and game activity changes clear all remembered positions. Move each position control again after pausing/resuming, loading, losing focus if it deactivates input, or reconnecting. Beam presses require observing release first.
- The bridge uses a one-second lease on received HID reports. A read failure clears the state immediately and attempts to reopen the uniquely matching interface. If several devices match, it waits instead of selecting one arbitrarily.
- The DLL independently expires commands after 600 ms without a valid update. Pipe operations have 300 ms deadlines; timed-out partial frames terminate the connection. A new session or readiness epoch rejects delayed commands from the old state.
- Inactive/paused game state and unavailable subscribed telemetry disable normal output. Inputs are released at the next available input callback, with bounded OFF cleanup for interrupted MIST. A paused or suspended game cannot be forced to execute callbacks. Releasing a virtual input is not the same as forcing every persistent game light/wiper mode off.
- Ctrl+C or closing the bridge disconnects the pipe. DLL shutdown joins its bounded worker before unloading; it never leaves plugin code running in a detached thread.

## Implementation boundaries

The CLI's HID reader and async pipe server run outside the game. The DLL has one IPC worker and an isolated C ABI boundary. Game callbacks do no filesystem or pipe I/O: lifecycle/telemetry updates use atomics, and the input callback takes a nonblocking snapshot. The worker never calls the SCS API. Each frame emits at most eleven bool events, releasing all inactive inputs before asserting requested ones. The command validator rejects multiple positions in the same control group.

Protocol v2 uses the local pipe `\\.\pipe\stalkshift-controls-v2`. Install the matching executable and DLL together; v1 indicator builds are deliberately incompatible. Frames are 40 bytes: `STSF` at 0, version `u16` at 4, kind `u8` at 6, zero reserved byte at 7, value `u32` at 8, four zero reserved bytes at 12, session `u64` at 16, sequence `u64` at 24, epoch `u64` at 32. Integers are little-endian. The plugin sends status, and the bridge echoes session/sequence/epoch in its command reply. The pipe accepts local clients only and permits one bridge instance.

Command bits 0–10 follow the input order listed in `stalkshift-protocol/src/controls.rs`; bit 11 requests MIST and is not an SDK input. Zero requests no controls. Status bit 0 is readiness; bits 1–6 are telemetry validity, 7–12 are telemetry values, and 13–23 are last input values. Telemetry order is left indicator, right indicator, parking lights, low beams, wipers enabled, high beams. Reserved fields and unknown/conflicting command bits are rejected.

The minimal ABI declarations match the official SDK's x64 size/alignment assertions. The mock-host probe loads the actual PE DLL, checks its exact four exports, callback registration, failed-init rollback, bounded shutdown and both API lifecycle orders. It is not a replacement for real ETS2 tests.

## Manual acceptance

On 2026-09-05, ETS2 1.60.1.7 loaded both StalkShift APIs. The first game test exposed reversed direction labels in the original HID recording. After correcting the decoder, fixture annotation and replay expectations together, the operator confirmed that physical left/right positions and centre cancellation matched the game. Logical indicator telemetry independently confirmed both directions and releases across repeated movements. The tested configuration was direct USB, "Multi function key switch direct", reported Pit House 1.4.0.30, separate firmware unknown.

| Check | Result |
|---|---|
| Physical left/right and centre cancellation | Passed on the corrected build, operator and telemetry confirmation |
| Light ring OFF / parking / low | Passed: operator confirmation and repeated matching telemetry transitions |
| Flash hold/release and high-beam toggle | Passed visually; high-beam toggles also matched telemetry |
| Front wiper OFF / INT / LO / HI | Pending visual game acceptance of this milestone |
| MIST exactly one complete sweep | Passed on standard Mercedes-Benz New Actros StreamSpace: operator confirmed one sweep after the acknowledged low-speed fix |
| Pause/resume and fresh movement | Passed: outputs released, no stale hold on resume, fresh movement restored control |
| USB disconnect/reconnect | Passed: indicator released on disconnect, device reopened, fresh movement restored control |
| Stop/restart bridge with indicator held | Pending |
| Keyboard/wheel coexistence and steering cancellation | Pending |
| Loading saves and changing truck | Pending |

During the light test, `truck.light.beam.high` changed with latched high-beam toggles but stayed unchanged during operator-confirmed flashes. Do not use that channel alone as acknowledgement that `lighthorn` is visibly working.

The current truck was identified by the operator as the standard Mercedes-Benz New Actros StreamSpace. The first MIST attempt used a one-input-frame intermittent request and produced no visible wipe. It was replaced with the acknowledged low-speed sequence above. On 2026-09-05, the operator confirmed that the corrected build produces one sweep in the requested OFF → MIST → OFF retest. This confirms MIST on this truck; other trucks and frame rates remain untested. Normal wiper activation/OFF telemetry worked, but visual differences between all three running modes were not yet clearly confirmed. SCS defines animation duration and delay in the truck's [interior data](https://modding.scssoft.com/wiki/Documentation/Engine/Units/accessory_interior_data); the bool SDK channel cannot inspect those settings.

1. Left → centre → right → centre: compare physical lever, game lamps, and logical telemetry.
2. Pause/resume: inputs release, stale commands are not replayed; re-arm by moving the stalk.
3. With an indicator active, stop the bridge: the virtual input is released; restart and re-arm.
4. With an indicator active, disconnect/reconnect USB: no retained virtual hold; reconnect recovers and requires re-arming.
5. Test keyboard/wheel indicators, steering auto-cancellation, loading a save and changing truck. Record differences rather than assuming compatibility from the semantic mapping alone.
