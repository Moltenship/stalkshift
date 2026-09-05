# ETS2 control integration

This developer document records the implementation and game-test history. For installation and everyday use, read the player guides in [English](../README.md) or [Russian](../README.ru.md). Version 1.0 implements the measured control set in MOZA "Multi function key switch direct" mode; the acceptance table below distinguishes individual confirmations from unverified cases.

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

The plugin registers 21 bool inputs, listed in `stalkshift-protocol/src/controls.rs`, including `horn`, `flasher4way`, `gear0`, `geardrive`, `gearreverse`, `parkingbrake`, `cruiectrl`, `cruiectrlres`, `cruiectrlinc` and `cruiectrldec`. Those spellings match `semantical.*` entries in the local ETS2 controls file. Registration does not prove game behavior.

Seven bool channels cover indicators, lights, wipers and parking brake. Four numerical channels add cruise target, navigation speed limit, truck speed and displayed gear. The numeric callbacks use the SDK float/s32 types and each-frame delivery, reject non-finite/out-of-range speeds and expire samples after 500 ms. Missing numeric data disables dependent actions, without disabling indicators and lights. The bool wiper channel cannot prove animation speed or cycle count.

| Physical control | Requested game input |
|---|---|
| Left / centre / right indicator | `lblinkerh` / neither / `rblinkerh` |
| Light ring OFF / parking / low beams | `lightoff` / `lightpark` / `lighton` |
| Front thumbwheel OFF / INT / LO / HI | `wipers0` / `wipers1` / `wipers2` / `wipers3` |
| Front thumbwheel MIST | Confirm OFF, request low continuous `wipers2`, wait for enabled telemetry, then request OFF; visual single-sweep acceptance required |
| Left lever towards driver | Hold `lighthorn` until release |
| Left lever away from driver | `hblight` press/release; game handles the toggle |
| Right selector bottom to top | D / N / R / P; one 150 ms gear command per observed position change |
| P | Neutral and one parking-brake press only if telemetry reports brake off; at most one second waiting for that telemetry |
| P to R, N or D | Release parking brake once if telemetry reports it on; wait at most one second for missing telemetry. Startup in a lower position does not release it. An earlier toggle must settle before another is requested. |
| Right lever towards driver | Hold normal `horn` until release |
| Right lever below lowest detent | One `parkingbrake` press per entry |
| REAR spring movement from OFF | One `flasher4way` press; upper-context spring movement is unassigned |
| Cruise ON/OFF spring rotary | One `cruiectrl` press |
| Cruise lever towards driver | `cruiectrlres` if target is zero, otherwise `cruiectrl`; unknown target suppresses action |
| Cruise up / down | `cruiectrlinc` / `cruiectrldec`, 150 ms presses every 300 ms while held |
| Left small spring switch (circle/fog symbol) | Toggle automatic speed-limit adjustment; off by default |

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

The CLI's HID reader and async pipe server run outside the game. The DLL has one IPC worker and an isolated C ABI boundary. Game callbacks do no filesystem or pipe I/O: lifecycle/telemetry updates use atomics, and the input callback takes a nonblocking snapshot. The worker never calls the SCS API. Each frame emits at most 21 bool events, releasing all inactive inputs before asserting requested ones. The command validator rejects multiple positions in the same control group.

Protocol v3 uses local pipe `\\.\pipe\stalkshift-controls-v3`; v1/v2 builds are incompatible. Frames are 56 bytes: `STSF` at 0, version u16 at 4, kind u8 at 6, reserved zero at 7, value u64 at 8, session u64 at 16, sequence u64 at 24, epoch u64 at 32. Four i32 values at 40/44/48/52 contain cruise target, limit and speed in mm/s, then displayed gear; `i32::MIN` means unavailable. Command replies always clear these fields to unavailable. All integers are little-endian.

Command bits 0?20 match SDK input order, 21 requests MIST, 22 requests P, 23 requests contextual cruise pull and 24 toggles automatic adjustment. Status bit 0 is readiness, 1?7 validity, 8?14 bool values, 15?35 emitted inputs, and 36 automatic adjustment enabled. Bool channel order is listed in `controls.rs`. Unknown/conflicting bits, invalid versions and stale sessions/epochs are rejected.

Automatic adjustment requires kilometres and the game's 5 km/h cruise step. It acts only on an already active cruise target and positive fresh speed limit, aims for a reachable 5 km/h value at or below the limit, waits for each target change, and stops after one second without acknowledgement. It does not repeat a refused step or oscillate after overshoot. A manual cruise action disables the mode; an unexpected target change from another input also yields control. Missing limits suspend adjustment. Pause, reconnect and lease expiry reset the mode to off. The left spring switch enables it again.

The minimal ABI declarations match the official SDK's x64 size/alignment assertions. The mock-host probe loads the actual PE DLL, checks its exact four exports, callback registration, failed-init rollback, bounded shutdown and both API lifecycle orders. It is not a replacement for real ETS2 tests.

## Manual acceptance

On 2026-09-05, restarting the game exposed a reconnect loop: the reused named-pipe server could deliver buffered status frames from the previous client. The bridge now drops and recreates that server after a disconnect, with bounded recovery and first-instance protection. A Windows regression test leaves an old frame unread and verifies five distinct client sessions; it failed on the old implementation. The operator confirmed restored indicator and wiper control after restarting the bridge. The automatic recovery fix still needs a full game-restart acceptance check.

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
| Horn hold/release; REAR hazard toggle and upper no-op | Implemented; combined game acceptance pending |
| D/N/R/P and bottom-overtravel parking toggle | Implemented; automatic/sequential gearbox acceptance pending |
| Cruise toggle, resume/cancel and held speed repeat | Implemented; combined game acceptance pending |
| Optional speed-limit adjustment and manual override | Implemented; combined game acceptance pending |

During the light test, `truck.light.beam.high` changed with latched high-beam toggles but stayed unchanged during operator-confirmed flashes. Do not use that channel alone as acknowledgement that `lighthorn` is visibly working.

The current truck was identified by the operator as the standard Mercedes-Benz New Actros StreamSpace. The first MIST attempt used a one-input-frame intermittent request and produced no visible wipe. It was replaced with the acknowledged low-speed sequence above. On 2026-09-05, the operator confirmed that the corrected build produces one sweep in the requested OFF → MIST → OFF retest. This confirms MIST on this truck; other trucks and frame rates remain untested. Normal wiper activation/OFF telemetry worked, but visual differences between all three running modes were not yet clearly confirmed. SCS defines animation duration and delay in the truck's [interior data](https://modding.scssoft.com/wiki/Documentation/Engine/Units/accessory_interior_data); the bool SDK channel cannot inspect those settings.

1. Left → centre → right → centre: compare physical lever, game lamps, and logical telemetry.
2. Pause/resume: inputs release, stale commands are not replayed; re-arm by moving the stalk.
3. With an indicator active, stop the bridge: the virtual input is released; restart and re-arm.
4. With an indicator active, disconnect/reconnect USB: no retained virtual hold; reconnect recovers and requires re-arming.
5. Test keyboard/wheel indicators, steering auto-cancellation, loading a save and changing truck. Record differences rather than assuming compatibility from the semantic mapping alone.
