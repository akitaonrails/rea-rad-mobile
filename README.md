# Rad Mobile

A launcher for Sega's **Rad Mobile** (1990, System 32) on Windows and
Linux, in the spirit of [Wanszai](https://github.com/wanszai)'s arcade
ports: one package with a bundled, patched MAME, a gamepad- and
wheel-driven menu, modern video options, and serious steering-wheel
support with force feedback.

**No ROMs are included.** Bring your own Rad Mobile romset.

## Features

| | |
|---|---|
| Video | Desktop, 1080p, 1440p or 4K rendering; fullscreen or window; 4:3, stretch, or **true widescreen** (the game draws more of the road at 16:9, 16:10 or ~21:9); Sharp, Smooth, CRT and CRT Deluxe filters; brightness; vsync |
| Wheels | Logitech, Thrustmaster, Fanatec (CSL DD, CSL Elite…), MOZA and generic wheels; steering lock (180–900°, Full; 270° default) computed from the wheel's rotation; separate pedal devices; a setup wizard for pedals and buttons; pedal calibration and invert |
| Force feedback | Synthetic, from the car's state read out of the game: speed-dependent centring and damping, barrier scrape, crash jolts; strength and invert; Wanszai's effect keepalive; gamepad rumble |
| Gamepads | Stick and trigger deadzone and curve (Linear … Cubic) |
| Arcade settings | Difficulty, starting timer, rival and police car speed, free play, credits, attract sound — written into the game's EEPROM |
| ROMs | Found by content under any file name; old sets without the factory EEPROM work |

## Using it

1. Download the package for your system and unpack it.
2. Put your Rad Mobile zip in `roms/` (or set an extra folder in the menu).
3. Run `RadMobile.exe` (Windows) or `./RadMobile` (Linux).

Command line: `--play` (straight into the game, for frontends),
`--windowed`, `--data DIR`, `--check-roms`, `--list-devices`,
`--print-mame-command`, `--mame-args "…"`. See `RadMobile --help`.

The player guide is [packaging/README-player.txt](packaging/README-player.txt).

## How it works

```
 wheel / pedals / pad ──SDL2──▶ launcher ──shared memory──▶ MAME (patched)
        ▲                       │  steering lock, curves,      │ "REA Bridge" joystick
        └── force feedback ◀────┘  pedal calibration, FFB  ◀───┘ rea_speed / rea_lateral /
             (DirectInput / evdev)                                rea_playing outputs
```

- The launcher owns every input device, as Wanszai's wrappers do. MAME
  sees one virtual joystick fed through a memory-mapped bridge file
  (`-joystickprovider reabridge`), so device numbering, deadzones and
  platform differences never reach the game.
- MAME publishes the car's speed, lateral position and game state
  (`-output reabridge`), and the launcher turns them into forces.
- Before each run the launcher rebuilds the romset under MAME's names,
  writes the operator settings into the EEPROM (always the Upright
  cabinet: the Deluxe cabinet needs a motor board MAME lacks), and writes
  the controller file and the widescreen layout.

Details: [docs/RESEARCH.md](docs/RESEARCH.md) (Wanszai study, EEPROM map,
widescreen), [docs/WANSZAI_WHEELS.md](docs/WANSZAI_WHEELS.md),
[docs/RAM_MAP.md](docs/RAM_MAP.md), [docs/MAME.md](docs/MAME.md),
[docs/RUNTIME_NOTES.md](docs/RUNTIME_NOTES.md).

## Building

Needs Rust (1.95+), a C++20 compiler and SDL2 development files for
MAME; for Windows, MinGW-w64, LLD and Rust's `x86_64-pc-windows-gnu`
target (the scripts use the `gaming` distrobox from
[distrobox-gaming](https://github.com/akitaonrails/distrobox-gaming)
when the host lacks them); Docker for the portable Linux build.

```sh
git submodule update --init --depth 1 vendor/mame
scripts/build-mame.sh linux          # and/or: windows
scripts/build-launcher.sh linux      # and/or: windows
scripts/build-linux-portable.sh      # Linux release binaries for older distros
scripts/package.sh linux             # and/or: windows  → dist/pkg/
```

During development `cargo run -p radmobile-launcher` finds MAME in
`dist/mame-<os>/`.

### Checks

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
RADMOBILE_ROM_DIR=/path/to/roms cargo test -p radmobile-core -- --ignored
```

`scripts/dev/fake-wheel.py` creates a virtual force-feedback wheel on
Linux (`/dev/uinput`) and logs every effect the launcher sends; with it
the whole path can be tested without hardware (docs/RUNTIME_NOTES.md).

## Licence

The launcher is MIT OR Apache-2.0. The bundled MAME is GPL-2.0-or-later
(see [THIRD_PARTY.md](THIRD_PARTY.md)).
