# Initial MOZA USB observations

Measured on 2026-09-05 with the StalkShift diagnostic executable on Windows x64. These are observations from one device, not a supported firmware/mode matrix. Pit House mode, firmware version and operator confirmation of the movement sequence are still pending.

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
| `80 00 00 00 00 00 00 00` | 2 |
| `00 01 00 00 00 00 00 00` | 4 |
| `00 02 00 00 00 00 00 00` | 2 |

Nonzero reports arrived in this order, repeated twice:

```text
80 00 ... → 00 01 ... → 00 02 ... → 00 01 ...
```

Each nonzero report was followed by zero about 150 ms later. The sequence is consistent with three movement events, but left/centre/right assignments remain provisional until the operator confirms the sequence and device mode. The recorder timestamps host read completion.

## Consequences for the decoder

- An all-zero report cannot be treated as a centred physical lever in this recording mode. Releasing a pulse and moving the lever to centre are distinct events.
- If this mode is retained, the decoder will likely need to latch positions from observed movement events, with initial state **unknown** until an appropriate event arrives.
- Test whether another Pit House mode exposes persistent positions or an initial-state query before choosing the production device protocol.
- Confirm the mapping with isolated left, centre and right movements, then repeat startup/reconnect tests with the lever already displaced.
- The local raw captures remain in ignored `captures/`. No semantic MOZA fixture has been published yet.
