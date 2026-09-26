# RWOLF

A clean-room reimplementation of **Wolfenstein 3D** built with the
[Bevy](https://bevyengine.org) engine. It reads the original `WL6` data files
that **you** supply and plays the original maps, textures and digitised sounds
through a from-scratch software raycaster.

> **No game content is included.** RWOLF ships no Wolfenstein 3D data and no
> id Software code. You must provide your own legally obtained copy of the
> registered (WL6) files. *Wolfenstein 3D* is a trademark of its respective
> owners; this project is unofficial and is not affiliated with or endorsed by
> them.

## AI disclaimer

This project was written **entirely by an AI coding agent** (DeepSeek V4.1
Flash, running in OpenCode), driven by a series of natural-language prompts
from the repository owner. The owner specified the features and reviewed and
tested the results; the AI wrote all source code, tests and documentation.

The prompting was a sequence of short, concrete English requests — for example
*"reimplement Wolfenstein 3D in Bevy so it loads the original WL6 data"*,
followed by targeted fixes and features such as *"it crashes opening a door"*,
*"the turn direction is reversed"*, *"build a level-select UI accessible with
Esc"*, *"add a config where the user supplies their own game folder"* and
*"create a Flatpak with install instructions"*. For each prompt the AI chose an
approach, implemented it, built it and reported back.

As with any AI-generated code, treat it as unreviewed: it may contain bugs.
There is no warranty.

## Game data

RWOLF does not include any game data. If no data is found, it opens an in-app
**folder picker** — navigate to the folder that contains `VSWAP.WL6` and press
**USE THIS FOLDER** (the choice is saved). You can also point it at your files
by copying `wolf3d-bevy.toml.example` to `wolf3d-bevy.toml` and editing
`data_dir`:

```toml
data_dir = "/path/to/WOLF3D"
```

Alternatively set the `WOLF3D_DATA_DIR` environment variable. If neither is
set, the engine looks in `./data`, `./WOLF3D` and `~/Downloads/WOLF3D`.

Expected files:

```
AUDIOHED.WL6  AUDIOT.WL6   GAMEMAPS.WL6  MAPHEAD.WL6
VGAHEAD.WL6   VGADICT.WL6  VGAGRAPH.WL6  VSWAP.WL6
```

## Build and run

```sh
cargo run --release
```

### Flatpak

A Flatpak manifest lives in [`flatpak/`](flatpak/). With `flatpak-builder`
installed:

```sh
flatpak/build.sh                                  # build + install
flatpak/setup-config.sh /path/to/WOLF3D           # point it at your data
flatpak run io.github.wynplusplus.wolf3dbevy
```

See [`flatpak/README.md`](flatpak/README.md) for details.

## Gameplay

* Original enemies (guards, officers, SS, dogs, mutants, the episode bosses
  and the Pac-Man ghosts) driven by a port of the original's tile-based AI:
  eight-way chase/dodge pathfinding, per-enemy chase speeds, probabilistic
  shooting, the dog's leap-and-bite, and sight/sound that propagates through
  floor areas. Ambush actors only wake when they see you.
* A persistent high-score table, a single save slot (the menu's SAVE/LOAD or
  `F5`/`F9`), the death cam, and the "Get Psyched!" intro.
* Locked doors and keys, treasure and health pickups, weapons and ammo.
* **Pushable secret walls** that slide two tiles, and **secret floors** reached
  from the secret elevator (with the original's `ElevatorBackTo` routing and
  15,000-point bonus).
* An end-of-floor **intermission** with kill / secret / treasure ratios, time
  and score bonus, a **game-over** screen, and lives.
* Enemies open doors and alert nearby allies, and doors refuse to close on the
  player.

## Controls

| Action | Key |
| --- | --- |
| Move / strafe | `W` `A` `S` `D` |
| Turn | mouse, or `←` `→` / `Q` `E` |
| Run | `Shift` |
| Fire | left mouse, or `Ctrl` |
| Use / open door / push wall | `Space` |
| Weapons | `1` `2` `3` `4` |
| Level-select menu | `Esc` |
| Cycle difficulty (menu) | `D` |
| Save / load game | `F5` / `F9` (or the menu's SAVE/LOAD) |
| High scores (menu) | `SCORES` button |
| Game-folder picker | `F` (or the `FILES` button in the menu) |

## License

MIT. See [`LICENSE`](LICENSE). No game assets are included or licensed here.
