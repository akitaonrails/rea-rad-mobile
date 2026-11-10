# Rad Mobile work RAM (radm / radmu)

V60 work RAM is 64 KB at `0x200000` (little-endian). Found by dumping RAM
from headless MAME runs driven through the bridge with scripted inputs
(`.work/scripts/ramdump.lua`, `drive.py`) and correlating with the inputs
and the picture. Published by our MAME patch (`segas32_state::rea_update_telemetry`, every vblank, `radm`/`radmu` only) as the outputs `rea_speed`, `rea_lateral` and `rea_playing`, which reach the launcher through `-output reabridge`. (MAME 0.289 Lua can no longer create outputs, so this lives in the driver.)

| Address | Type | Meaning | Evidence |
|---|---|---|---|
| `0x201F44` | u16 | Car speed, internal units, 0 to ≈430 at top speed | Rises smoothly on throttle (no gear-shift sawtooth), drops on brake, collapses on a wall hit; correlation 0.98 with the HUD km/h. Not linear with the HUD, so we only use it normalised. `0x20F4A0` holds a copy. |
| `0x207F44` | s16 | Car lateral position (negative left of centre) | Integrates the steering input; saturates around ±2700 against the barriers. |
| `0x2018AF` | u8 | Player game in progress (non-zero) | Set the frame Start is pressed, cleared at game over; zero through every attract demo race. |

Not found (yet): road curvature, surface type. Collisions are derived from
sudden speed loss; barrier scraping from the lateral position at its limit.
