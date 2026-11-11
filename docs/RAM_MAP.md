# Rad Mobile work RAM (radm / radmu)

V60 work RAM is 64 KB at `0x200000` (little-endian). Found by dumping RAM
from headless MAME runs driven through the bridge with scripted inputs
(`.work/scripts/ramdump.lua`, `drive.py`) and correlating with the inputs
and the picture. Published by our MAME patch (`segas32_state::rea_update_telemetry`, every vblank, `radm`/`radmu` only) as the outputs `rea_speed`, `rea_lateral`, `rea_heading`, `rea_playing`, `rea_bound0..3` (the section's limits below) and `rea_frame` (a vblank count), which reach the launcher through `-output reabridge`. (MAME 0.289 Lua can no longer create outputs, so this lives in the driver.)

| Address | Type | Meaning | Evidence |
|---|---|---|---|
| `0x201F44` | u16 | Car speed, internal units, 0 to ≈430 at top speed | Rises smoothly on throttle (no gear-shift sawtooth), drops on brake, collapses on a wall hit; correlation 0.98 with the HUD km/h. Not linear with the HUD, so we only use it normalised. `0x20F4A0` holds a copy. |
| `0x201F46` | s16 | Car lateral position (negative left of centre) | Moves by the heading of the same frame every other frame on straights (lateral step = -heading per game tick); held near +-560..600 by the barriers, where speed collapses. Replaces `0x207F44`, used until 0.1.0, which swings to +-4000 and decays while the car stays at a wall (a view offset, not the position). |
| `0x201F30` | s16 | Car heading (positive points left) | On a straight, the angle to the road: it changes only while the wheel is off centre (the wheel is a turn rate) and stays put when it is centred. Through a bend it turns with the road (it reached about 400 in the first long left bend while the car held its line), so the car moves sideways by the road's heading minus this one, a tick at a time. The road's heading is not in RAM; the launcher's assist learns it from the two (`crates/radmobile-core/src/assist.rs`). |
| `0x20F4D4` / `0x20F4D6` | s16 | Left / right barrier of the current section | The lateral is clamped to them (code at `0x71e9b`). ±600 in the first section, ±380 after the first checkpoint, −1000/+1100 in a wide stretch later. |
| `0x20F4D0` / `0x20F4D2` | s16 | Left / right edge of the road surface | Inside or equal to the barriers (±350..380 where the barriers stand at ±1000). |
| `0x20F00A` | u8 | Player game in progress (`0xFF`) | Set when Start is pressed (code at `0x66eae`), cleared after the game-over screens; zero through attract demo races. It also picks the player's clock step at a round start (below). Replaces `0x2018AF`, used until 0.1.1, which clears at the first checkpoint. |
| `0x20F4B8` | u8 | Race timer, BCD seconds | Starts at the EEPROM's starting timer (default `0x45`). |
| `0x20F4B6` / `0x20F4B4` | u16 | Timer sub-second counter / its step | Each game tick (two frames) the step is taken off the counter and a second off the timer when it wraps (code at `0x67646`): 30 ticks a second at the default step `0x888`. The step comes from a table at `0x66693` (`0x7a8`..`0x908`) indexed by difficulty (`0x20F520`) when a round starts. Our patch divides it for the RACE CLOCK setting (`REA_RADM_TIME_SCALE`). |

## Steering input (code)

The steering ADC is read at `0x7031C` once a frame. At `0x70333` the game
drops a sample more than 0x40 away from the last accepted one and keeps the
old value (a glitch filter for a real wheel's potentiometer). A gamepad
stick snapping back to centre is such a jump, so the game keeps steering:
MAME Testers 08052, closed as "not a bug". Accepted values are then scaled
by 74/64 around 0x80 (`0x70365`). Our patch rate-limits the steering sent
to the ADC to 0x30 a sample (docs/MAME.md).

Not found (yet): the road's own heading, surface type. Collisions are derived from
sudden speed loss; barrier scraping from the lateral position at its limit.
