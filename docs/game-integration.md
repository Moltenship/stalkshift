# ETS2 indicator integration

This milestone implements indicators only. The supported device profile is the measured MOZA "Multi function key switch direct" mode. The rest of the first-version feature set remains in the plan.

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

Start ETS2, select the intended profile, acknowledge its SDK plugin notification if shown, and enter the truck. Wait for the bridge to report `ready=true`. Move the lever out of centre and back to centre once to establish its position, then test left/centre/right.

The plugin registers only `lblinkerh` and `rblinkerh`. These semantic mixes must be present in the game's control configuration. They were found in both local profiles of ETS2 1.60.1.7 during development. The bridge displays the requested virtual inputs separately from the actual logical indicator telemetry, so sending a command is not confused with the game applying it.

Avoid mapping the same physical stalk buttons directly to indicators in ETS2 or enabling Pit House's keyboard adaptation at the same time. The installer does not change these settings. Other wheel and keyboard bindings remain in place; their behavior with a registered held-indicator input must also be checked in the game.

## Disconnects, pause and startup

- MOZA reports movement pulses rather than persistent lever positions in the measured mode. Startup position is unknown until a position pulse arrives.
- New pipe sessions and game activity changes clear the bridge's remembered position. Move the stalk through centre again after pausing/resuming, loading, losing focus if it deactivates input, or reconnecting.
- The bridge uses a one-second lease on received HID reports. A read failure clears the state immediately and attempts to reopen the uniquely matching interface. If several devices match, it waits instead of selecting one arbitrarily.
- The DLL independently expires commands after 600 ms without a valid update. Pipe operations have 300 ms deadlines; timed-out partial frames terminate the connection. A new session or readiness epoch rejects delayed commands from the old state.
- Inactive/paused game state and unavailable indicator telemetry disable output. Both inputs are released at the next available input callback. A paused or suspended game cannot be forced to execute callbacks.
- Ctrl+C or closing the bridge disconnects the pipe. DLL shutdown joins its bounded worker before unloading; it never leaves plugin code running in a detached thread.

## Implementation boundaries

The CLI's HID reader and async pipe server run outside the game. The DLL has one IPC worker and an isolated C ABI boundary. Game callbacks do no filesystem or pipe I/O: lifecycle/telemetry updates use atomics, and the input callback takes a nonblocking snapshot. The worker never calls the SCS API. Each frame emits at most two bool events, releasing the opposite side before asserting the desired one.

The fixed 32-byte protocol is `STSF`, version `u16`, kind `u8`, value `u8`, session `u64`, sequence `u64`, epoch `u64`; integers are little-endian. The plugin sends status, and the bridge replies with the same session/sequence/epoch. Command values are unknown=0, centre=1, left=2, right=3. Status bits expose readiness, validity/values of logical indicator telemetry, and the last virtual input snapshot. The pipe accepts local clients only and permits one bridge instance.

The minimal ABI declarations match the official SDK's x64 size/alignment assertions. The mock-host probe loads the actual PE DLL, checks its exact four exports, callback registration, failed-init rollback, bounded shutdown and both API lifecycle orders. It is not a replacement for real ETS2 tests.

## Manual acceptance

On 2026-09-05, ETS2 1.60.1.7 loaded both StalkShift APIs and reported the expected logical indicator changes and centre releases. The operator then identified that the original HID recording had reversed direction labels. The decoder, fixture annotation and replay expectations were corrected together; physical direction acceptance must use the corrected build. This observation does not yet verify pause, USB reconnection, keyboard coexistence or steering cancellation.

1. Left → centre → right → centre: compare physical lever, game lamps, and logical telemetry.
2. Pause/resume: inputs release, stale commands are not replayed; re-arm by moving the stalk.
3. With an indicator active, stop the bridge: the virtual input is released; restart and re-arm.
4. With an indicator active, disconnect/reconnect USB: no retained virtual hold; reconnect recovers and requires re-arming.
5. Test keyboard/wheel indicators, steering auto-cancellation, loading a save and changing truck. Record differences rather than assuming compatibility from the semantic mapping alone.
