# StalkShift controls

[Installation and help](../README.md) · English · [Русский](controls-ru.md)

These assignments apply to both ETS2 and ATS in version 1.1, with Pit House set to **Multi function key switch direct**. Keep StalkShift running while you play.

## Left stalk: indicators and lights

| Movement | Result |
|---|---|
| Indicator position left or right | The matching indicator stays on |
| Return to centre | Indicators off |
| Light ring OFF | Lights off |
| Light ring, small-light symbol | Parking lights |
| Light ring, headlight symbol | Low beam |
| Pull the stalk towards you and hold | Flash headlights until you release it |
| Push the stalk away and release | Toggle high beam once |

The beam movements spring back. Holding the stalk away does not repeatedly toggle high beam. Use low beam before testing high beam.

The small spring-loaded switch beside the light ring has a circle and a fog-light symbol. In StalkShift it controls **speed-limit cruise adjustment**, described below. It does not operate fog lights.

## Front wipers

Use the **small wheel beside MIST / OFF / INT / LO / HI on the right stalk**. The larger ring marked REAR has different assignments.

| Wheel position | Result |
|---|---|
| OFF | Stop the wipers |
| INT | Intermittent wiping |
| LO | Continuous slow wiping |
| HI | Continuous fast wiping |
| MIST | One sweep, then stop at the bottom |

Leaving the wheel in MIST does not repeat the sweep. Return to OFF and select MIST again for another sweep. The truck supplies the wiper animation and available speeds. Turn on the ignition; some trucks or mods may show fewer distinct speeds.

## Right main stalk: gears and parking brake

The four fixed positions, from **bottom to top**, are:

| Position | Result |
|---|---|
| D | Drive |
| N | Neutral |
| R | Reverse |
| P | Neutral and parking brake on, if it is not already on |

**Moving from P to R, N or D releases the parking brake if it is on.** Moving between D, N and R does not change the parking brake. Starting StalkShift with the lever already in a lower position does not release it automatically.

Use an automatic or sequential transmission setting. Select R or P while stationary.

Additional movements:

| Movement | Result |
|---|---|
| Pull towards you in D, N or R | Normal horn for as long as you hold it |
| Push down past the lowest fixed D position | Toggle the parking brake once |
| Hold below D | No further parking-brake toggles |
| Push below D and pull towards you together | Horn while held, plus one parking-brake toggle |

Pulling towards you is mechanically unavailable in the top P position. Do not force it. The combined movement has no separate assignment; both functions act independently. Release the downward movement before pressing again to toggle the brake again.

## REAR ring: hazard lights

Before first use after loading or a pause, turn REAR to its upper fixed position and back to OFF.

| Movement | Result |
|---|---|
| From OFF, turn down into the spring-loaded position and release | Toggle hazard lights once |
| Hold that spring-loaded position | No repeated toggles |
| Upper fixed position | No game action |
| Spring-loaded position above the upper fixed position | No game action |

The ring's OFF position selects which spring movement StalkShift recognizes. Returning the ring to OFF does not turn off active hazards. Use the lower spring movement again to turn them off. REAR does not operate rear wipers or washers in this release.

## Small cruise stalk

Use the **separate small cruise lever**, not the main right stalk. Its **ON/OFF control is a spring-loaded rotary ring**, not a button you push at the end.

| Movement | Result |
|---|---|
| Rotate ON/OFF and release | Turn cruise on at the current speed, or turn active cruise off |
| Pull towards you with cruise active | Cancel cruise |
| Pull towards you with cruise inactive | Resume the previously saved speed, if the game has one |
| Move up | Increase the selected cruise speed |
| Move down | Decrease the selected cruise speed |
| Hold up or down | Continue changing the selected speed in steps |

To start, drive on the road at about **40–50 km/h or 30 mph**, release the brake and clutch, rotate ON/OFF once and let it spring back. Release the accelerator. The truck should hold speed. Brake or use ON/OFF to cancel. The game decides when cruise is available and how large each speed step is.

If it does not engage, compare with the normal cruise key, usually **C**, under the same conditions. Cruise does not make a parked truck start driving. The main right stalk's pull movement sounds the horn; only the small cruise stalk's pull resumes or cancels cruise.

## Optional speed-limit cruise adjustment

This mode changes the **speed selected for an already active cruise control** to match the road limit supplied by the game's navigation. It starts **off**.

1. In Install.cmd, choose the same speed units as in your game. Set the game's cruise adjustment step to **5 km/h** for kilometres, or **5 mph** for miles. Both units are supported in either game.
2. Start normal cruise on the road. Make sure the navigator displays a speed limit.
3. Turn and release the little spring switch with the circle and fog-light symbol beside the **left light ring**.
4. StalkShift gradually changes the selected cruise speed toward the road limit, using a reachable five-unit step at or below that limit.

If you change the game's speed units later, close the game and StalkShift and run Install.cmd again to update the choice. The installer suggests km/h for ETS2 and mph for ATS; it does not change your game settings.

Turn the same switch again to disable the mode. Using a manual cruise control on the stalk also disables it. You can check its state in the StalkShift window: `auto=true` means on, `auto=false` means off. There is no separate in-game indicator for this mode.

If the road limit is unavailable, StalkShift waits. If the game refuses a speed change, adjustment stops until the limit changes or you switch the mode off and on again. An unexpected change to the cruise target also gives control back to you. After a pause or USB reconnect, the mode is off again.

This adjusts the cruise target. It does not steer, avoid traffic, or guarantee the truck will slow down before a sign. Keep controlling the truck and brake when needed. It never enables or resumes cruise by itself.

## Loading, pausing and reconnecting

When you open a menu, disconnect the stalks or lose the game connection, StalkShift releases held inputs. Indicators controlled by StalkShift turn off. It does not automatically restore a gear, horn, held button or automatic cruise adjustment when you return.

Once back in the cab:

- Release spring-loaded controls before pressing them again.
- Move the indicator, light ring, front-wiper wheel and gear selector again when you want to use them. For a control already in the desired position, move away and back.
- For hazards, turn REAR to the upper fixed position and back to OFF before using the lower spring movement.

Existing game states such as a manually applied parking brake or active hazards are not all cleared by opening a menu. Check the dashboard before moving off.
