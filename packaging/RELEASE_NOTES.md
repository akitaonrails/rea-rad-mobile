Rad Mobile launcher with a bundled, patched MAME 0.289 (Sega System 32 only).

## Changes in 0.3.0

- Gamepads and wheels now turn on the lights and the wiper. MAME's Rad
  Mobile driver fixed both buttons to keyboard keys (Space, Left Shift),
  so only the keyboard reached them and Round 3's "Turn on the light"
  could not be answered. Pad: X or LB lights, Y or RB wiper.
- New **GAMEPAD BUTTONS** (CONTROLLER page) and **WHEEL BUTTONS** (WHEEL
  page): pick a control, press its new button, trigger or pedal. Every
  cabinet control can move, pedals included, and a button taken for one
  control leaves its old one. RESET TO DEFAULTS and RESET WHEEL TO
  AUTOMATIC go back. Back + Start always returns to the menu.
- Wheels the launcher does not know get the generic shift paddles
  (buttons 1 and 2) as lights and wiper, as known wheels already did.

## Changes in 0.2.0

- Gamepad steering reworked: in ASSISTED mode the stick moves the car
  across the road, and letting go keeps the car's line, through bends
  too. The game's bends turn the car's heading, which 0.1.1's assist
  fought, so it pinned the car to the outside barrier and full stick
  could not bring it back. The assist now learns each bend from how the
  car moves and uses each section's real barriers (they move in to ±380
  after the first checkpoint).
- The assist and force feedback no longer switch off at the first
  checkpoint (the bundled MAME read the wrong "game in progress" flag).
- New **RACE CLOCK** setting (GAME page): 1.5x, 2x or 3x longer seconds,
  start time and checkpoint bonuses alike.
- New **PRESET** setting (GAME page): EASY (lowest difficulty, slowest
  rivals and police, 60 seconds, 2x clock) or ARCADE (factory settings).

## Changes in 0.1.1

- Gamepad steering fixed: letting go of the stick after steering hard no
  longer leaves the car turning. Rad Mobile ignores steering readings that
  jump faster than a real wheel can turn (MAME Testers 08052); the bundled
  MAME now feeds it the stick at wheel speed.
- New **STICK STEERING** option on the CONTROLLER page. ASSISTED (default):
  the stick sets the car's angle and centring it drives straight down the
  road. ARCADE: the stick is the cabinet wheel, as on the real machine.
  Brake and steer into sharp bends either way.
- Wheels: the barrier-scrape force now uses the car's real position.

**No ROMs are included.** Put your own Rad Mobile zip in `roms/` after unpacking.

| Package | Run |
|---|---|
| `RadMobile-*-windows-x86_64.zip` | `RadMobile.exe` |
| `RadMobile-*-linux-x86_64.tar.gz` | `./RadMobile` (glibc 2.36 or newer) |

Steering wheels with force feedback, true widescreen, up to 4K and CRT
filters; see `README.txt` in the package. Force feedback has been tested
with a virtual wheel, not yet on physical hardware.

The MAME source changes are in `licenses/mame-0289-rea-rad-mobile.patch`
inside each package (GPL-2.0-or-later).
