# Rad Mobile work RAM (radm / radmu)

V60 work RAM is 64 KB at `0x200000` (little-endian). Found by dumping RAM
from headless MAME runs driven through the bridge with scripted inputs
(`.work/scripts/ramdump.lua`, `drive.py`) and correlating with the inputs
and the picture. Published by our MAME patch (`segas32_state::rea_update_telemetry`, every vblank, `radm`/`radmu` only) as the outputs `rea_speed`, `rea_lateral`, `rea_heading` and `rea_playing`, which reach the launcher through `-output reabridge`. (MAME 0.289 Lua can no longer create outputs, so this lives in the driver.)

| Address | Type | Meaning | Evidence |
|---|---|---|---|
| `0x201F44` | u16 | Car speed, internal units, 0 to ≈430 at top speed | Rises smoothly on throttle (no gear-shift sawtooth), drops on brake, collapses on a wall hit; correlation 0.98 with the HUD km/h. Not linear with the HUD, so we only use it normalised. `0x20F4A0` holds a copy. |
| `0x201F46` | s16 | Car lateral position (negative left of centre) | Moves by the heading of the same frame every other frame on straights (lateral step = -heading per game tick); held near +-560..600 by the barriers, where speed collapses. Replaces `0x207F44`, used until 0.1.0, which swings to +-4000 and decays while the car stays at a wall (a view offset, not the position). |
| `0x201F30` | s16 | Car heading relative to the road (positive points left) | Changes only while the wheel is off centre (the wheel is a turn rate) and stays put when it is centred; it stays 0 with no steering input. In sharp bends the car is thrown outward whatever the heading, so the simple lateral relation holds on straights only. |
| `0x2018AF` | u8 | Player game in progress (non-zero) | Set the frame Start is pressed, cleared at game over; zero through every attract demo race. |

## Steering input (code)

The steering ADC is read at `0x7031C` once a frame. At `0x70333` the game
drops a sample more than 0x40 away from the last accepted one and keeps the
old value (a glitch filter for a real wheel's potentiometer). A gamepad
stick snapping back to centre is such a jump, so the game keeps steering:
MAME Testers 08052, closed as "not a bug". Accepted values are then scaled
by 74/64 around 0x80 (`0x70365`). Our patch rate-limits the steering sent
to the ADC to 0x30 a sample (docs/MAME.md).

Not found (yet): road curvature, surface type. Collisions are derived from
sudden speed loss; barrier scraping from the lateral position at its limit.
