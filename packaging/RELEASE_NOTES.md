Rad Mobile launcher with a bundled, patched MAME 0.289 (Sega System 32 only).

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
