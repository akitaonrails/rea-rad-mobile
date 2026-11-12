# Research

What this launcher builds on, what we found, and what we decided.

## Wanszai's arcade wrappers (the inspiration)

Wanszai (github.com/wanszai) publishes free PC / Xbox wrappers for Sega and
Namco racers: Virtua Racing (Model 1), Sega Rally Championship (Model 2),
Ridge Racer Collection (System 22) and Scud Race (a decompilation port).
The repositories hold only READMEs; the binaries are on the release pages.
We studied the PC builds statically (strings and Ghidra through REA); no
code was copied. Details of the wheel stack: [WANSZAI_WHEELS.md](WANSZAI_WHEELS.md).

| Wrapper | Core | Notes |
|---|---|---|
| Virtua Racing 1.0.3 | MAME (model1) embedded in a custom D3D11 frontend | 16:9 toggle, CRT filters, render scale, wheel wizard, FFB |
| Sega Rally 1.0.0c | MAME **0.150** embedded (`srallyc`, refuses other romsets) | drive-board bytes → FFB, H-shifter, EEPROM seeded to Free Play |
| Ridge Racer Collection 1.1.1 | System 22 emulator | "synthetic FFB" for games that never had a motor |
| Scud Race 0.0.17 | decompilation, SDL2 | "set your wheel to 270° in its own software" |

What they have in common, and what we reproduce:

- **One package, one executable, bring your own ROM.** The ROM is found by
  content, checked, and the player is told exactly what is wrong
  ("ROM NOT FOUND", "ROM FAILED VERIFICATION … wrong or incomplete romset").
- **The arcade's own settings are seeded.** Sega Rally writes a Free Play
  EEPROM into MAME's NVRAM on first boot (`[sr_seed] installed FREE PLAY
  eeprom`, `enforced FREE PLAY (coin cluster rewritten)`) and Virtua Racing
  patches its EEPROM for the aspect ratio.
- **Modern video:** resolution up to 4K, 16:9 or 4:3, CRT filters,
  brightness.
- **The wrapper owns the wheel**, not the emulator: DirectInput devices are
  read by the wrapper, a setup wizard writes `wheel_di.ini`, and the
  emulator only sees finished cabinet values. Settings: input device
  (wheel / controller), FFB strength (off–100 %), invert FFB, steering lock,
  invert accelerator / brake, wheel button binding, gear shifter setup,
  stick/trigger deadzone and linearity (Linear, Slight, Medium, Heavy,
  Cubic), rumble strength.
- **Known wheels by VID/PID**: Logitech DFP, G25, DFGT, G27, G29, G920,
  G923, Thrustmaster T150, TMX, T300RS, TX, T248, T-GT, Fanatec CSL Elite,
  CSL DD / GT DD Pro, MOZA R3, plus a generic fallback.
- **Separate pedal and shifter devices** pinned by GUID, then VID/PID.
- **FFB:** a constant-force effect plus friction; a "keepalive" that
  re-starts effects some firmwares drop; lost-device fallback to the pad
  until re-plug.

## The game

Rad Mobile (Sega, 1990), Sega System 32 (NEC V60). MAME driver
`sega/segas32.cpp`, sets `radm` (World) and `radmu` (US).

Controls: steering wheel (PADDLE), accelerator (PEDAL), brake (PEDAL2),
Light and Wiper buttons, Start, Coin. No gears. Outputs: Start, Lights and
Wiper lamps. The deluxe cabinet moved (a motor board with its own Z80,
`epr-13686`, not emulated); MAME's notes say the motors are not hooked up.

### ROM sets

MAME 0.289's `radm` requires `eeprom-radm.ic76` (128 bytes, the factory
EEPROM). Most romsets in circulation were dumped before it was added and
lack it, so MAME refuses them. Our patch marks it optional and fills the
region with `0xFF` (a blank chip); the game then initialises its own
defaults. Verified: a set without the file boots and plays.

### EEPROM (93C46, 128 bytes)

Two identical 64-byte copies (offset 0 and 64). Mapped by changing one
test-menu setting at a time and diffing MAME's NVRAM (`.work/` scripts):

| Offset | Size | Meaning |
|---|---|---|
| 0 | 4 | `SEGA` |
| 4 | 4 | `01 83 01 00` (constant) |
| 8 | 2 | CRC-16/XMODEM (poly 0x1021, init 0) of bytes 10..61, stored low byte first |
| 10 | 1 | Screen mode: `00` normal, `FF` flipped |
| 16 | 1 | Advertise (attract) sound: `00` off, `FF` on |
| 20 | 1 | Free play: `01` on |
| 21 | 1 | Cabinet: `00` Deluxe, `01` Upright |
| 22 | 1 | Credits to start (default 2) |
| 24 | 1 | Credits to continue (default 1) |
| 32 | 1 | Difficulty, 0..15 shown as 1..16 (default 7 = "8") |
| 33 | 1 | Starting timer, BCD seconds 40..60 (default `45`) |
| 34 | 1 | Rival car speed, 0..15 shown 1..16 (default 7) |
| 35 | 1 | Police car speed, 0..15 (default 7) |
| 60 | 2 | Changes to `01 00` after any setting is edited (kept as is) |

A blank-chip boot defaults to the **Deluxe** cabinet, which shows
"Motor trouble" in attract mode and refuses to start a game. The launcher
writes Upright (and Free Play by default) before every run. Verified: a
crafted image (Upright + Free Play, fresh CRC) boots, shows "Free play",
and Start begins a race.

## True widescreen

System 32 can already show 416 pixels (`$31FF00` bit 15), so the video
hardware draws a wider picture natively. Our patch (`REA_S32_WIDE_MARGIN`)
widens the visible area to 320 + 2 × margin, shifts every tilemap scroll,
the bitmap layer and the sprite origin by the margin so the original
picture stays centred, and stretches clip windows that touch the screen
edge out to the new edge. The text layer (the HUD) stays inside the
original 320 pixels.

Verified in races (headless snapshots): the road, buildings, traffic and
crowds fill both margins; the dashboard sprite is 320 wide, so the lower
corners show road. Objects the game culls at the original edge can pop in
at the margin edge. The option is labelled experimental.

Pixel aspect of the original picture: (4/3) / (320/224) = 0.9333. A target
aspect A needs margin = (A × 224 / 0.9333 − 320) / 2, e.g. 16:9 → 53.
The patch caps the margin at 96 (512 pixels, about 2.13:1).

## Force feedback

The arcade had no steering motor, so FFB is synthetic, as in Wanszai's
Ridge Racer 1/2: our MAME patch reads the car's state from work RAM and
publishes it as outputs; the launcher turns it into spring, damper,
constant force and rumble. RAM map: [RAM_MAP.md](RAM_MAP.md).

## CRT filters (Wanszai's Scud Race)

Wanszai's Scud Race 0.0.17 (PC) embeds the HLSL source of its CRT, whose
header calls it "in-house HLSL from Timothy Lottes' public-domain CRT
math", ported from his Sega Rally X360 work. Its menu has two switches:
CRT FILTER ("scanlines, phosphor mask and glow") and CURVATURE ("CRT
screen curve and vignette"). Three passes:

1. Bloom source at quarter resolution: `k = saturate((max(r,g,b) − 0.40) / 0.60)`, `c × k`.
2. Separable 9-tap Gaussian blur, H then V (weights 0.227027, 0.194595,
   0.121622, 0.054054, 0.016216).
3. Composite: optional barrel warp (`uv += uv × (|uv.yx| / (3,4))² × curvature`,
   black outside), a Gaussian beam per virtual scanline whose width runs
   from beam_min to beam_max with luma (`col × mix(1 − scan, 1, beam)`),
   a 3-pixel RGB phosphor mask in output pixels (aperture grille, or a
   shadow mask with alternate rows shifted 1.5 px), glow added, vignette
   `1 − |vd|² × vignette`, brightness.

The presets are a table in the code (`k_crt_presets`, six floats each;
read from the binary, not guessed):

| Preset | Scanlines | Beam | Mask | Mask strength | Glow |
|---|---|---|---|---|---|
| TRINITRON | 0.35 | 0.90–1.60 | aperture grille | 0.30 | 0.20 |
| ARCADE | 0.50 | 0.80–1.40 | shadow mask | 0.45 | 0.35 |

CURVATURE on is curvature 0.12 and vignette 0.25; Scud Race uses 384
virtual lines (Rad Mobile has 224). Our ARCADE CRT and TRINITRON CRT are
our own bgfx shaders of the same maths with these numbers (`bgfx/`).

## Decisions

| Item | Decision |
|---|---|
| Emulator | MAME 0.289, only `segas32.cpp` (`SUBTARGET=radmobile`), patched, bundled |
| Wheel path | Launcher reads devices through SDL2 and writes cabinet values into a shared-memory bridge; MAME reads it as one joystick (`-joystickprovider reabridge`) |
| FFB path | MAME outputs (`-output reabridge`) → launcher → SDL haptic (DirectInput on Windows, evdev on Linux) |
| Menu backdrop and bezel | Made on the player's machine: the backdrop is a snapshot of their own ROM taken by an unseen MAME run (`art::backdrop`), the bezel is our own drawing (`art::bezel`); nothing from the game is shipped |
| rea-launcher-sdk | Not a dependency (it targets PC games run under Wine); its coding and testing rules are followed |
