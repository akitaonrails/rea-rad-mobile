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
| Stick release (MAME Testers 08052) | Scripted stick eased to full left then released in one frame, old and new MAME | Old: the game keeps steering (heading 42 → 103 after release). New: steering stops at release |
| Stick steering assist (history: docs/STEERING_ASSIST.md) | Launcher `--play` with `scripts/dev/fake-pad.py` (virtual Xbox 360 pad), race telemetry logged by a MAME Lua script | 0.1 s taps move the car about a lane and it holds that line. Pinned to the outside barrier of the first long left bend, 0.7 s of full left takes it across and it holds its line through the bend at top speed (0.1.1: stuck at the barrier). Where the road narrows after the first checkpoint, the held line moves in with it. The end of the first bend can still brush the inside barrier for a moment |
| Bend behaviour | Headless MAME driven from Python through the bridge (`.work/bend`) | Full lock takes the car from one barrier to the other in that bend; 0.1.1's assist steered against the bend (it capped the angle at 48 and pulled it back to 0) |
| Race clock | `REA_RADM_TIME_SCALE=200`, RAM logged | Step `0x888` → `0x444`; the timer loses a second every two; checkpoint bonuses unchanged in seconds |
| Player game flag | RAM over a race through the first checkpoint | `0x2018AF` clears at the first checkpoint (so 0.1.1 dropped the assist and FFB there); `0x20F00A` holds until after game over |
| Light and Wiper from a pad or wheel | Headless MAME, bridge button bits pressed from Python, Lua logging port `:mainpcb:P1_A` | 0.2.0: the port never changed (`ff`); the fields listened to Space and Left Shift only. With the game-specific controller section: `fd` for Light, `fb` for Wiper |
| Button remapping | Launcher (windowed) with `scripts/dev/fake-pad.py` (d-pad and A driving the menu) and `scripts/dev/fake-wheel.py` | Pad: LIGHTS remapped to R3 (`pad_light=r3`; the A press that opened the capture was not taken). Wheel: START to button 10 (`btn_start=wheel:9`), ACCELERATOR to the brake pedal's axis, which then left the brake |
| Menu backdrop | Launcher under Xvfb (Wayland unset) with a scratch data folder | The ROM found, MAME runs unseen and the treated intro skyline is saved in about 6 s (including the romset rebuild); the menu draws it |
| Cabinet bezel | MAME snapshot through the `Cabinet` view (`-snapview Cabinet -snapsize 1920x1080`) with the launcher's layout | The 4:3 picture sits in the frame's hole, cabinet art on both sides (shaders are not in snapshots) |
| Linux package | `dist/pkg/…-linux-x86_64.tar.gz` (bookworm build, glibc ≥ 2.36) on Arch, with the virtual wheel | Wheel, FFB (1414 force updates in one race), Exit button; menu renders |

## Not verified yet

- A physical wheel on Linux. The owner's Fanatec CSL DD ran on Windows
  (0.4.0, issues #1 and #2): steering, pedals and force feedback work.
  Its forces pulled away from the centre there. SDL gives the direction
  a force comes *from* (SDL_haptic.h), and we sent "from the east", so
  every push came out mirrored. Since 0.5.0 forces come "from the west"
  (`input/haptic.rs`), and INVERT FFB stays off by default. SDL keeps
  that meaning on every platform; a Linux run on hardware is still to do.
- The CRT shaders' look (x11grab under Xvfb does not capture MAME's GL
  window; the chains load without errors).
- Windows below 100 % volume. One Windows run was silent at
  `-volume -2` and audible at `-volume 0` (#1). MAME 0.289 only
  multiplies the mix by the gain (0.79 at −2 dB, `emu/sound.cpp`), and
  the same build on Linux measured exactly that (RMS 704 against 895).
  Since 0.5.0, full volume passes no `-volume` at all, the same as the
  audible run.

## Windows wheels

A Fanatec CSL DD shows up twice on Windows, once per HID collection, and
only one reports input. The launcher prefers the collection that takes
force feedback; WHEEL on the WHEEL page (`wheel_device`, a device path
from `--list-devices`) picks one by hand. Devices with no axes that share
a model with another device are a gamepad's spare interfaces and are
skipped (an 8BitDo pad shows up three times).

## Leaving the game

Esc, the BACK TO MENU wheel button, or Back+Start on a pad sends MAME's
UI_CANCEL; MAME exits without confirmation and the launcher returns to
its menu. If the launcher is killed, MAME is killed with it (SIGKILL via
`PR_SET_PDEATHSIG` on Linux, a kill-on-close job object on Windows).
