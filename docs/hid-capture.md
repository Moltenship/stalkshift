# Recording the MOZA stalks

This procedure collects evidence for a decoder. No button bit positions are assumed.

## Before recording

1. Connect the stalks directly through USB.
2. Note Windows, ETS2, Pit House and stalk firmware versions, plus the selected Pit House adaptation mode. The HID `release_number` is a USB descriptor field, not a guaranteed firmware version.
3. Close ETS2 during discovery. Record the Pit House mode; do not change mode halfway through a capture series.
4. Run `stalkshift list`. If no interfaces are found, check USB, HidHide access, and the actual VID/PID. The tool does not automatically change those settings.
5. Choose the interface index whose reports respond to the controls. Multiple collections can require separate capture series. Indices are local to enumeration; run `list` again after reconnecting.

## Capture series

Use a distinct file and descriptive `--label` for each row. Start from a documented baseline. Hold each position for about two seconds, return to baseline between actions, and repeat the sequence twice. Increase `--seconds` if needed. Recording starts immediately when the command prints its recording message.

| Label | Actions |
|---|---|
| `baseline` | Touch nothing for 10 seconds |
| `indicators` | Centre → left → centre → right → centre |
| `headlights` | Pull/release, push/release; record hold versus latch behavior |
| `light-ring` | Each detent forward and back |
| `left-rocker` | Press, hold, release |
| `gear-selector` | Each D/N/R/P position in both directions |
| `right-pull` | Pull/release in every mechanically available position |
| `right-overtravel` | Push below D, hold, release |
| `wiper-wheel` | Small thumbwheel in the right stalk's rectangular recess beside MIST/OFF/INT/LO/HI; rotate through its positions in both directions |
| `rear-ring` | Ring at the end of the right stalk marked REAR; each detent and spring-loaded action from each available baseline |
| `cruise` | End button, pull, up and down, including holds |
| `combinations` | Indicator + lights; selector + pull; other mechanically possible combinations |
| `startup-left` | Connect with indicator already left; start recorder without moving, then move to centre |

The baseline and startup captures can legitimately receive no reports. That is evidence of missing initial-state observability, not proof that all controls are off. HIDAPI's Windows native backend may not provide every descriptor operation; a descriptor error is retained in the header rather than aborting input capture. If needed, collect Windows HID capabilities separately in the next milestone.

Example:

```powershell
.\target\release\stalkshift.exe record --device 0 --label "indicators centre-left-centre-right-centre; mode=direct" --seconds 20 --output captures/indicators-01.jsonl
.\target\release\stalkshift.exe inspect captures/indicators-01.jsonl
```

Do not unplug during a normal capture. For a deliberate disconnect test, expect an error and a file without an end record. Reconnect, list again and begin a new file. The discovery recorder does not auto-reconnect, so separate physical sessions cannot silently merge.

## Format v1

UTF-8 JSON Lines: one `header`, zero or more `report` records, one `end`.

- Header: schema, tool version, label, VID/PID, usage, interface, product and USB release fields; report descriptor or its retrieval error. No device path or serial number is intentionally recorded. Labels and backend error strings should still be reviewed before sharing.
- Report: consecutive zero-based `sequence`, monotonic microseconds from recording start, and `data` as an array of bytes. These are the bytes returned by HIDAPI, including a report ID when applicable. No bytes are stripped, interpreted or deduplicated.
- End: elapsed microseconds and total report count. Missing end means interrupted/incomplete, even if earlier reports remain useful for manual analysis.

`inspect` checks schema, record order, timestamps, report bounds and footer count. It reports byte changes across consecutive reports, not semantic button states. If an interface multiplexes report IDs, comparisons can cross report types; use the descriptor before interpreting offsets. It streams the file with a bounded JSON-line allocation.

Console output shows only changes; the file contains every received report. Timestamps describe host read completion, not the physical USB polling instant. A slow disk or terminal can affect observation timing. These recordings are for protocol discovery, not latency certification.
