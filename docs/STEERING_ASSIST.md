# Stick steering: what went wrong and how it works now

Rad Mobile was built for a wheel. Playing it with a gamepad stick took
three releases to get right. This note records why, so nobody repeats the
same mistakes. The code is `crates/radmobile-core/src/assist.rs`; the RAM
addresses are in `docs/RAM_MAP.md`.

## How the game steers

- **The wheel sets a turn rate, not an angle.** While the wheel is off
  centre the car's heading (`0x201F30`, positive points left) keeps
  changing. Centred, the heading stays where it is. A car left at an angle
  keeps drifting across the road until it hits a barrier. This is
  authentic (MAME Testers 03230); with a real wheel you counter-steer
  without thinking about it.
- **The heading is not relative to the road.** On a straight it is the
  angle to the road. Through a bend it turns with the road: it reached
  about 400 in the first long left bend while the car held its line.
- **The car moves across the road** (`lateral`, `0x201F46`, negative is
  left) each game tick by about *road heading − car heading*. The road's
  heading is not in RAM anywhere.
- **The game updates every other frame.** The lateral changes on one
  frame of two, so movement is measured against the value two frames back.
- **Barriers move.** Each section has its own limits (`0x20F4D4` left,
  `0x20F4D6` right) and road edges (`0x20F4D0`, `0x20F4D2`): ±600 in the
  first section, ±380 after the first checkpoint, −1000/+1100 with edges
  −380/350 in a wide stretch. The game clamps the lateral to the barriers
  (code at `0x71e9b`).
- **Large jumps in the steering input are ignored** (MAME Testers 08052):
  a sample more than 0x40 from the last is dropped, so a stick snapping
  back to centre in one frame left the car turning.

## What went wrong, release by release

### 0.1.0: the car kept turning after the stick was released

A stick goes from full lock to centre in one frame. The game dropped that
sample as a glitch (the 08052 rule above), so it kept the last off-centre
value and the car kept turning.

**Fix (MAME patch):** the ADC feeding the game moves at most 0x30 a
sample, about as fast as a real wheel, so no sample is ever dropped.

### 0.1.1: the assist fought the bends and switched itself off

0.1.1 added an assist that turned the stick into a target heading
(`−stick × 48`) and steered the car toward it. Two bugs:

1. **It steered against every bend.** It treated heading 0 as "straight
   down the road", but in a bend the right heading is the bend's, not 0.
   With the stick centred it pulled the car back to 0, so in a left bend
   the car slid to the outside (right) barrier and the stick could not
   bring it back: full left only asked for 48, far short of the ~400 the
   bend needed.
2. **It switched off at the first checkpoint.** It took "a player's game
   is running" from `0x2018AF`, which clears at the first checkpoint. From
   there the stick drove the raw wheel (and wheels lost force feedback).
   The flag that holds until game over is `0x20F00A` (set by Start at
   `0x66eae`).

### 0.2.0: the assist learns the bend

The stick now asks for a **sideways speed across the road**, not a
heading: centred holds the car's line, full lock is a quick lane change.
Every game tick:

1. **Learn the bend.** The car moved `across` = lateral − lateral two
   frames back. Since across ≈ bend − heading, the bend's heading is about
   `heading + across`. The estimate takes half of each new measurement
   (`BEND_RATE`). Nothing is learnt while touching a barrier: there the
   car cannot move whatever its heading, and learning would teach the
   assist that the car's own heading is the bend's.
2. **Choose a sideways speed.**
   - Stick pushed: `stick × 48` units a tick (`ACROSS_AT_FULL_STICK`).
   - Stick centred (|stick| < 0.02): wait until the car has settled
     (moving less than 4 a tick), remember that line (`hold`), then pull
     back toward it at 0.1 of the distance, at most 8 a tick. The held
     line stays 60 inside the road's edges, so where the road narrows the
     car moves in with it. Capturing the line at release instead of once
     settled made small taps barely move the car in a bend.
3. **Steer.** Target heading = bend − sideways speed. The wheel is
   `−0.035 × (target − heading)`, clamped to ±1.
4. **Never steer into a barrier.** Within 45 of a barrier (`TOUCHING`):
   the wheel never turns toward it. A stick pushed away from it gets at
   least that much wheel, because the bend estimate may be stale after
   time spent on the barrier. A centred stick turns away by at least 0.3
   (`AWAY`).

The barriers and road edges come from MAME (`rea_bound0..3`); ±600 is the
fallback while they are unknown. Matching measurements to game ticks
needs MAME's own vblank count (`rea_frame`): the bridge's `mame_seq` ticks
twice a frame, and using it made the car oscillate.

Wheels never use the assist.

## The "still broken" report that was 0.1.1

After 0.2.0 shipped, a report came in: pinned to the right barrier in a
left bend; braking, the car could turn left slowly, but accelerating
pushed it back. The screenshot was from Round 1 with 15 seconds on the
clock, past the first checkpoint. The launcher log
(`<data>/launcher.log`, first line of each start) showed **0.1.1** had
run: the gaming box's install was still pinned to 0.1.1 by its Ansible
role. That was 0.1.1's checkpoint bug above, not a new one. A headless
run of 0.2.0's logic in the same bend came off the right barrier and
across the road in under a second at full left; a local 0.2.0 build then
felt right in play.

**Before chasing a gameplay report, check the version** in the first
line of the player's `launcher.log`.

## How to test steering changes

- **Unit tests** in `assist.rs` simulate bends, taps, barriers and
  narrowing roads.
- **Headless MAME driven from Python** through the bridge (`.work/bend`,
  `.work/wall`): a script writes steering into the shared memory, reads
  `rea_*` outputs every `rea_frame`, and runs a per-frame policy (a Python
  copy of the assist plus a stick schedule such as "right 1600–1680, then
  full left"). It runs at full speed with `-video none -nothrottle`. The
  first long left bend runs from about frame 1500 to 2200 after the race
  starts.
- **End to end:** the launcher with `--play` and `scripts/dev/fake-pad.py`
  (a virtual Xbox 360 pad), with a MAME Lua script logging RAM.

## Known limits

- The end of the first long bend can brush the inside barrier for a
  moment.
- A car let go right next to a barrier holds a line only about 15 outside
  the touching zone (60 in from a barrier when the road edges are unknown),
  so it can brush the barrier now and then. Pushing the stick still
  pulls it off.
- A collision that throws the car sideways can upset the bend estimate
  for a few ticks.
