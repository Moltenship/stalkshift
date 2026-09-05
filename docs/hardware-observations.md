# Initial MOZA USB observations

Measured on 2026-09-05 with the StalkShift diagnostic executable on Windows x64. These are observations from one device, not a supported firmware/mode matrix. The operator confirmed the requested movement sequence and reported "Multi function key switch direct 1.4.0.30 pit house". We record the mode as "Multi function key switch direct" and 1.4.0.30 as the reported Pit House version; the separate device firmware version is unknown.

## Discovery

| Field | Observed value |
|---|---|
| Product | MOZA Multi-function Stalk |
| USB VID:PID | `346e:0024` |
| HID usage page / usage | `0001:0004` (Generic Desktop / Joystick) |
| HID interface number | `2` |
| USB release field | `0100` (not a verified firmware version) |
| Matching enumerated collections | One |
| Input report length returned by HIDAPI | 8 bytes |

The descriptor returned by the Windows native backend is:

```text
05 01 09 04 a1 01 05 09 19 01 29 20 15 00 25 01 75 01 95 20 81 02 c0
```

It describes 32 one-bit button inputs with no explicit Report ID. HIDAPI reconstructs this descriptor from Windows preparsed data, so it is not claimed to be a raw USB descriptor dump. Keep all eight observed report bytes until the payload/padding convention is verified.

## Baseline and indicator movement recording

A five-second baseline contained 25 reports, all `00 00 00 00 00 00 00 00`. Physical positions were not annotated; zero is **not** assigned the meaning "centre".

The operator was asked to perform centre → left → centre → right → centre twice. A 60-second capture contained 306 reports with these values:

| Bytes | Count |
|---|---|
| `00 00 00 00 00 00 00 00` | 298 |
| `80 00 00 00 00 00 00 00` (right event) | 2 |
| `00 01 00 00 00 00 00 00` (centre event) | 4 |
| `00 02 00 00 00 00 00 00` (left event) | 2 |

Nonzero reports arrived in this order, repeated twice:

```text
80 00 ... → 00 01 ... → 00 02 ... → 00 01 ...
```

Each nonzero report was followed by zero about 150 ms later. During the first ETS2 test, the operator reported having reversed left and right during the original recording: the physical left position activated the game's right indicator with the original mapping. The corrected recording sequence is right → centre → left → centre twice, and the assignments above reflect that correction. Recorded report bytes and timestamps were not altered. The recorder timestamps host read completion. These measurements do not establish behavior in other Pit House modes or firmware versions.

## Consequences for the decoder

- An all-zero report cannot be treated as a centred physical lever in this recording mode. Releasing a pulse and moving the lever to centre are distinct events.
- The implemented `DirectIndicatorDecoder` latches positions from movement events, with initial state **unknown** until an appropriate event arrives. It resets on explicit disconnect/reset or invalid input; pulse release does not cancel the indicator.
- Test whether another Pit House mode exposes persistent positions or an initial-state query before choosing the production device protocol.
- Extend confirmation with startup/reconnect tests while the lever is already displaced. The decoder now also drives the [indicator bridge and SCS plugin](game-integration.md); game verification is documented separately from this capture.
- The [confirmed indicator fixture](../fixtures/moza/direct-indicators.jsonl) preserves all 306 reports and their timestamps. Only its header label was updated after operator confirmation; it contains no device path or serial number. Original local captures remain in ignored `captures/`.

Replay the measured sequence:

```powershell
cargo run --locked -p stalkshift -- decode-indicators fixtures/moza/direct-indicators.jsonl
```

Expected transitions: right → centre → left → centre, twice. Seven core tests cover pulse release, duplicate reports, startup uncertainty, resets, invalid/conflicting inputs, unrelated buttons and this real capture.
