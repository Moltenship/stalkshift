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

## Light ring, front-wiper thumbwheel and beam lever

The next capture session used the same USB interface and direct mode. The operator confirmed that the light test rotated the left light ring. Unrecorded up/down movements of the right main lever were separate. For front wipers, use the small thumbwheel in the rectangular recess beside MIST/OFF/INT/LO/HI on the right main stalk, not the end ring marked REAR. See the [manufacturer photo](https://mozaracing.com/cdn/shop/files/MOZA-Multi-function-Stalks-4-scaled.jpg?v=1750067211&width=2200) and the reference bridge's [physical mapping](https://github.com/JacKJodel23/MOZA-Truck-Stalk-Bridge/blob/main/docs/STALK_MAPPING_EN.md).

| Control | Report prefix (remaining bytes zero) | Observed behavior |
|---|---|---|
| Light ring OFF | `01 00 00` | Approximately 150 ms position pulse |
| Parking lights | `02 00 00` | Approximately 150 ms position pulse |
| Low beams | `04 00 00` | Approximately 150 ms position pulse |
| Front wiper MIST | `00 20 00` | Position pulse; zero does not mean leaving MIST |
| Front wiper OFF | `00 40 00` | Position pulse |
| Front wiper INT | `00 80 00` | Position pulse |
| Front wiper LO | `00 00 01` | Position pulse |
| Front wiper HI | `00 00 02` | Position pulse |
| Left lever towards driver | `20 00 00` | Held until release |
| Release from towards driver | `10 00 00`, then zero | Release pulse |
| Left lever away from driver | `08 00 00` | Held until release to zero |

The beam lever was reported to return by itself rather than remaining latched. Unlike the position decoders, beam holds are released when their bits clear. Startup/reconnect requires a neutral beam report before a new press can act.

Reviewed captures preserve every report and timestamp:

- [Light ring](../fixtures/moza/direct-light-ring.jsonl): 456 reports over 90 seconds. Parking → low → parking → off, twice.
- [Front-wiper thumbwheel](../fixtures/moza/direct-wiper-wheel.jsonl): 614 reports over 120 seconds. The operator reported slips in the first pass. The second pass cleanly records INT → LO → HI → LO → INT → OFF → MIST; its final return to OFF happened after recording ended. The fixture label records these limitations, and replay must finish in MIST rather than inventing the missing OFF.
- [Beam lever](../fixtures/moza/direct-beam-lever.jsonl): 610 reports over 120 seconds. Two towards-driver holds and three away-driver holds were observed, with intervening releases. The file also contains light-ring movements. The label describes the requested two-pass procedure; the replay test checks the actual extra press and all releases.

These captures establish hardware events; a HID replay alone does not verify game modes or a complete single MIST sweep. See [game acceptance](game-integration.md).

## Right main lever: 2026-09-05

The operator confirmed that the position below the lowest fixed detent requires holding and returns on release. Pulling towards the driver was recorded separately, twice at each of the three lower fixed detents.

| Physical action | First three report bytes | Observed behavior |
|---|---|---|
| Lowest fixed detent | `00 00 10` | Approximately 150 ms pulse |
| Next fixed detent up | `00 00 20` | Approximately 150 ms pulse |
| Third fixed detent | `00 00 40` | Approximately 150 ms pulse |
| Highest fixed detent | `00 00 80` | Approximately 150 ms pulse |
| Push below lowest fixed detent | `00 00 08` | Held; release produces lowest-detent pulse |
| Pull towards driver | `00 00 04` | Held in all three tested detents; release clears bit |

- [Main lever capture](../fixtures/moza/direct-right-main.jsonl): 911 reports over 180 seconds. The requested recording originally included pulls, but the operator actually moved through the fixed positions and held bottom overtravel without pulling. The fixture label describes the corrected account; the recording begins partway through the position sequence.
- [Pull capture](../fixtures/moza/direct-right-pull.jsonl): 608 reports over 120 seconds. An accidental bottom-overtravel hold at 37.073 seconds precedes the six intentional towards-driver holds. Both are preserved, rather than labeling the accidental hold as horn input.

The intended D/N/R/P, horn and parking-brake assignments are not yet game-verified. No simultaneous pull/overtravel combination was requested in these recordings.

## REAR ring: 2026-09-05

The operator confirmed downward spring action from OFF, upward rotation into the other fixed position, then upward spring action and repeats. The [reviewed capture](../fixtures/moza/direct-rear-ring.jsonl) contains 612 reports over 120 seconds, including extra presses. Its last observed fixed position is upper, despite the original request to finish in OFF.

| Physical action | First three report bytes | Observed behavior |
|---|---|---|
| OFF / return from lower spring action | `00 04 00` | Approximately 150 ms pulse |
| Upper fixed position / return from upper spring action | `00 08 00` | Approximately 150 ms pulse |
| Either spring action | `00 10 00` | Held until release; direction is not encoded in this bit |

Three lower spring holds return to OFF; four upper spring holds return to the upper fixed position. The first lower hold is brief (about 0.62 seconds); later lower holds last about three seconds. A decoder must retain fixed-position context to distinguish the intended hazard action from the unassigned upper action. Zero reports alone do not establish that context, and an initially held spring action must not invent an OFF baseline. No in-game hazard result has been claimed yet.

## Cruise ON/OFF rotary switch: 2026-09-05

The operator identified the control on the separate cruise stalk as an ON/OFF rotary switch with spring return, similar in movement to REAR but without a latched position. It is not an end button. The [reviewed capture](../fixtures/moza/direct-cruise-on-off.jsonl) contains 902 reports over 180 seconds: one brief activation at 66.932 seconds, followed by two intentional holds at 126.304–128.609 and 131.952–134.351 seconds.

The active report is `00 00 00 01 00 00 00 00` (bit 24). Release clears this bit. Despite the original recording instructions, no towards-driver, upward or downward cruise-stalk movements were performed in this file. The fixture label corrects that distinction; raw reports and timestamps are unchanged. Game cruise toggling remains unverified.

## Cruise stalk directions: 2026-09-05

The [direction capture](../fixtures/moza/direct-cruise-directions.jsonl) contains 607 reports over 120 seconds. The operator confirmed the requested order: towards driver twice, up twice, down twice, leaving the ON/OFF rotary switch untouched. All six holds have distinct releases to zero; actual holds last approximately 1.6–2.5 seconds rather than the requested three seconds.

| Movement | First four report bytes | Intended game assignment, not yet verified |
|---|---|---|
| Towards driver | `00 00 00 08` | Resume when cruise is inactive; cancel when active |
| Up | `00 00 00 04` | Increase cruise target, repeating while held |
| Down | `00 00 00 02` | Decrease cruise target, repeating while held |

These are held signals, not latched-position pulses. Initial held inputs after a new session must wait for release and a fresh movement before causing a cruise action.

## Left small spring switch: 2026-09-05

The operator described circle and crossed-line lamp markings beside the small switch on the left main stalk, and confirmed spring return. The [capture](../fixtures/moza/direct-left-switch.jsonl) contains 452 reports over 90 seconds. Three active intervals use `40 00 00 00 00 00 00 00` (bit 6), followed by zero releases. The first lasts about 0.71 seconds, then 2.26 and 2.97 seconds. The intended assignment is toggling automatic speed-limit cruise adjustment; game acceptance is pending.
