# StalkShift

**Use your MOZA Multi-function Stalks in Euro Truck Simulator 2.**

Turn the lights on, select a gear, use the wipers and control cruise directly from your stalks. StalkShift is free and open source.

English · [Русский](README.ru.md)

**[Download for Windows](https://github.com/Moltenship/stalkshift/releases/download/v1.0.0/StalkShift-1.0.0-windows-x64.zip)** · [All controls](docs/controls-en.md) · [Report a problem](https://github.com/Moltenship/stalkshift/issues)

## What you need

- A Windows 64-bit PC with Euro Truck Simulator 2 installed.
- MOZA Multi-function Stalks connected directly by USB.
- MOZA Pit House, with the stalks set to **Multi function key switch direct**.

Version 1.0 targets ETS2. Game testing used ETS2 1.60.1.7 with standard Mercedes-Benz New Actros and Scania trucks. Other game versions, truck mods, American Truck Simulator and H-pattern shifters have not been verified. For gear selection, use an automatic or sequential transmission setting in ETS2.

## Install

1. [Download StalkShift 1.0](https://github.com/Moltenship/stalkshift/releases/download/v1.0.0/StalkShift-1.0.0-windows-x64.zip). Right-click the downloaded ZIP, choose **Extract All**, and keep the extracted folder somewhere convenient.
2. Close ETS2 and any running StalkShift window.
3. Open **Install.cmd** in the extracted folder. Choose English or Russian. The installer finds ETS2 in your Steam libraries. If it asks for the game folder, find it through Steam → ETS2 → Manage → Browse local files.
4. Choose the profile you play with. This removes existing stalk button assignments that could trigger an action twice. Your keyboard, wheel and pedal assignments stay in place. Enter **0** to skip this step if you prefer to remove stalk assignments yourself in ETS2.
5. Wait for the installation confirmation. If Windows reports access denied, right-click **Install.cmd** and choose **Run as administrator**.

Keep the whole extracted folder together. Installation adds the StalkShift game plugin and saves backups before changing existing files. You do not need to install programming tools or assign every stalk movement yourself.

## Play

1. Connect the stalks by USB. In Pit House, select **Multi function key switch direct**. Turn off any keyboard emulation for these controls, and close other stalk bridge programs.
2. Open **Start.cmd**. Leave its window open while playing; you can minimize it.
3. Start ETS2. Accept the third-party SDK notification if it appears, load your truck and turn on the ignition.
4. Move the controls you want to use. For example, move the indicator out of centre and back, then choose the light and wiper positions you need.

After loading, pausing or reconnecting USB, release spring-loaded controls and move position switches again. StalkShift waits for a new movement instead of restoring a gear or held button on its own. Before first using the REAR ring for hazards, turn it to the upper fixed position and back to OFF.

Open **Start.cmd** each time you want to play. Close its window when you finish.

## Your controls at a glance

| Control | What it does |
|---|---|
| Left stalk up / down / centre | Indicators / off |
| Left light ring | Lights off / parking lights / low beam |
| Left stalk towards you / away from you | Flash headlights while held / toggle high beam |
| Small front-wiper wheel on the right stalk | OFF / intermittent / slow / fast; MIST gives one sweep |
| Right stalk fixed positions, bottom to top | D / N / R / P |
| Right stalk towards you | Horn while held |
| Right stalk below D | Toggle parking brake once |
| REAR ring, spring turn down from OFF | Toggle hazard lights once |
| Small cruise stalk ON/OFF spring ring | Turn cruise on / off |
| Small cruise stalk towards you / up / down | Resume or cancel / increase / decrease cruise speed |
| Small spring switch beside the left light ring | Turn speed-limit cruise adjustment on / off |

Selecting **P** puts the truck in neutral and applies the parking brake. Moving from **P to R, N or D releases the parking brake** if it is on. Pulling the right stalk while pushing it below D sounds the horn and toggles the parking brake together.

To try cruise, drive at about **40–50 km/h**, release the brake and clutch, turn the little **ON/OFF ring** once and release it. Then release the accelerator. The truck should hold speed. This ring rotates and springs back; it is not an end button.

**[Read the full control guide](docs/controls-en.md)** for each movement, cruise adjustment and what happens after a pause.

## If something does not work

| Problem | What to try |
|---|---|
| Nothing responds | Keep Start.cmd open, check USB and the Pit House mode, then load the cab. The StalkShift window shows `ready=true` when the game is connected and accepting controls. Release and move the controls again. |
| An action happens twice, or turns on then off | Run Install.cmd again and select your profile to remove duplicate stalk assignments. Disable Pit House keyboard emulation and close other stalk bridges. |
| Cruise does not hold speed | Try it on the road at 40–50 km/h with brake and clutch released. Compare with ETS2's cruise key, normally C. If C also fails, check the game's cruise settings and driving conditions. |
| Hazards do not respond | Turn REAR to the upper fixed position, back to OFF, then into the spring-loaded position below OFF. |
| Wiper speeds look the same | The truck determines the available animations. Try a standard truck with the ignition on. |
| Start asks you to install again | Open Install.cmd from the same folder as Start.cmd. Do not mix files from different versions. |

Still stuck? [Open an issue](https://github.com/Moltenship/stalkshift/issues) in English or Russian. Include your ETS2 version, truck, Pit House mode, the movement you made and what happened. Session logs are in `%LOCALAPPDATA%\StalkShift\logs`, which you can paste into File Explorer's address bar. Review a log before sharing it.

## Update or remove

To update, close ETS2 and StalkShift, extract the new release, and run its **Install.cmd**. Use **Start.cmd** from that new folder afterwards.

To remove StalkShift, close ETS2 and StalkShift, then open **Uninstall.cmd**. It removes the StalkShift plugin. It restores the profile assignments backed up during installation only if you have not edited them since. Other plugins stay installed. Backups and logs remain in `%LOCALAPPDATA%\StalkShift`.

## Development

Want to build or contribute? See the [development guide](docs/development.md). Player instructions are above; building from source is optional.

StalkShift uses the [MIT license](LICENSE). It is an independent project, not an official MOZA or SCS Software product. See [third-party notices](THIRD_PARTY_NOTICES.md).
