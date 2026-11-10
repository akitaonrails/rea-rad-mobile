# Runtime notes

What has been verified running, how, and what has not.

## Verified (2026-11-10)

| What | How | Result |
|---|---|---|
| ROM identification | `--check-roms` and the ignored `real_roms` test against a MAME collection (read-only) | `radm_dup2.zip` recognised as Rad Mobile (World) among 23 `rad*` archives; set rebuilt under MAME 0.289 names |
| Optional factory EEPROM | MAME `-verifyroms` and real boots | "best available"; the game initialises a blank chip |
| EEPROM writer | Crafted Upright + Free Play image, CRC from `eeprom::crc16` | Boots to "Free play"; Start begins a race; no "Motor trouble" |
| Bridge (Linux) | Python and launcher writers, MAME `-joystickprovider reabridge -output reabridge` | MAME polls every frame; lamps and `rea_*` telemetry come back |
| Bridge (Windows) | Windows build under Wine | "REA Bridge" joystick connected; 100% speed |
| Wheel path end to end | `scripts/dev/fake-wheel.py` (virtual Fanatec CSL DD with FF) + `--play` + headless MAME | Wheel detected, profile applied, Start from a wheel button starts the race, throttle drives the car, steering into the barrier; constant force, damper and friction uploaded; 150 ms keepalive; jolts on hits; Exit button returns |
| Steering lock maths | Virtual wheel at 0.25 of 1080° with 270° lock | Full lock in the game; idle spring −0.1 at 50 % |
| True widescreen | Snapshots at margin 52 through the `Widescreen` view (1280×720) | Road and scenery fill 16:9, HUD centred |
| Startup | bgfx under Xvfb | Starts without MAME's warning box (System 32 is flagged "imperfect graphics") |
| Menu | `--screenshot` under Xvfb | Main and wheel pages; live input monitor follows the virtual wheel |
| Windows launcher | Wine: `--check-roms`, `--list-devices`, `--play` | Works; data next to the executable |
| Windows package | `dist/pkg/…-windows-x86_64.zip` unpacked, `--play` under Wine | Runs at 100 % with the bridge |
| Linux package | `dist/pkg/…-linux-x86_64.tar.gz` (bookworm build, glibc ≥ 2.36) on Arch, with the virtual wheel | Wheel, FFB (1414 force updates in one race), Exit button; menu renders |

## Not verified yet

- A physical wheel. The owner's Fanatec CSL DD was not connected during
  development; everything up to the evdev/DirectInput FF calls was
  exercised with the virtual wheel. First things to check on hardware:
  force direction (INVERT FFB exists for that), strength at 50 %, and
  whether the CSL DD's pedals are on the wheel base or a separate device
  (run WHEEL SETUP).
- The CRT shaders' look (x11grab under Xvfb does not capture MAME's GL
  window; the chains load without errors).
- Real Windows hardware (only Wine).

## Leaving the game

Esc, the BACK TO MENU wheel button, or Back+Start on a pad sends MAME's
UI_CANCEL; MAME exits without confirmation and the launcher returns to
its menu. If the launcher is killed, MAME is killed with it (SIGKILL via
`PR_SET_PDEATHSIG` on Linux, a kill-on-close job object on Windows).
