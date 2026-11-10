# How Wanszai's wrappers handle wheels and force feedback

Static analysis (Ghidra through REA) of the PC builds of Sega Rally
1.0.0c (SR), Virtua Racing 1.0.3 (VR), Ridge Racer Collection 1.1.1 (RRC)
and Scud Race 0.0.17. SR, VR and RRC share one input/FFB layer. Tags:
**[C]** confirmed from code, **[I]** inferred, **[U]** unknown. Nothing was
copied; this is the behaviour we re-implement in
`crates/radmobile-core/src/wheel/` and `ffb.rs`.

## 1. Known wheels [C]

40-byte rows: `vid, pid, name, steer_axis, gas_axis, brake_axis,
rotation_deg, shift_up_button, shift_down_button`. Axis codes X, Y, Z, RX,
RY, RZ, SL0, SL1.

| VID:PID | Model | steer / gas / brake | rotation | up / down |
|---|---|---|---|---|
| 046D:C298, C299, C29A, C29B, C24F, C262, C266, C26E | Logitech DFP, G25, DFGT, G27, G29, G920, G923 PS, G923 Xbox | X / Y / RZ | 900 | 4 / 5 |
| 044F:B677, B67F, B669, B696 | Thrustmaster T150, TMX, TX, T248 | X / RZ / Y | 900 | 0 / 1 |
| 044F:B66E, B684 | Thrustmaster T300RS, T-GT | X / RZ / Y | 1080 | 0 / 1 |
| 0EB7:0E03, 0005, 0020 | Fanatec CSL Elite, CSL Elite (PS4), CSL DD / GT DD Pro | X / Y / RZ | 1080 | 0 / 1 |
| 346E:0005 | MOZA R3 | X / Z / RZ | 900 | 13 / 12 |
| — | generic | X / Y / RZ | 900 | 0 / 1 |

Every row defaults to separate pedals that rest at the top of the axis
(`gas_invert = brake_invert = 1`). No per-model FFB quirks are stored.

## 2. Steering [C]

- Raw: `steer = clamp(axis / 32767, −1, 1)`; no centre offset, deadzone or
  invert for wheels. Autocentre is switched off.
- Steering lock: `s = clamp(steer × (rotation/2) / (lock/2), −1, 1)`;
  **FULL** uses the raw value. Values {180, 270, 360, 540, 900, FULL},
  default **270**, cycled in that order.
- Gamepad stick: `m = |x| ≤ dz ? 0 : (|x| − dz)/(1 − dz)` with sign, then
  `y = L·m³ + (1 − L)·m`, `L = linearity/100`. Labels: 0 Linear, 1–20
  Slight, 21–40 Mild, 41–60 Medium, 61–80 Heavy, 81–100 Cubic.

## 3. Pedals [C]

- Separate: `p = (raw + 32768)/65535`, inverted when flagged.
- Combined: `v = raw/32767` (negated when inverted); gas = max(v, 0),
  brake = max(−v, 0).
- Calibrated (wizard rest/peak, when |peak − rest| > 1000):
  `p = clamp((raw − rest)/(peak − rest), 0, 1)`.
- A fixed 2 % pedal deadzone: `out = min(1, (p − 0.02)/0.98)`.
- Gamepad triggers: deadzone (0–64 of 255) and the same cubic blend.
- Pedals on a separate USB device are pinned by instance GUID, then by
  VID/PID; reopen attempts are throttled to every 2 s.

## 4. Setup wizard [C]

Steering is never detected (it comes from the table). The wizard asks for
throttle, brake, shift paddles, d-pad and face buttons:

- It watches the wheel and up to three other controllers; each device must
  give 10 good reads before a baseline is taken.
- An axis capture arms when `|cur − baseline| > 16000`, tracks the peak,
  and completes on release (`|cur − start| < 8000`), recording rest, peak
  and direction.
- Timeouts: 10 s per axis, 5 s per button (a timeout skips the step).
- `combined = 1` when both pedals turn out to be the same axis.
- The result is saved in `wheel_di.ini`; hand edits survive until the
  wizard runs again.

## 5. Force feedback

- **Effects [C]:** a constant force plus friction (RRC adds a damper). No
  spring: autocentre off. Constant magnitude `f × 10000`; conditions use
  `k × 10000`. Parameters are re-sent only when they change.
- **Revive [C]:** effect status checked every 250 ms; restarted at most
  every 500 ms. **Keepalive:** VR/RRC re-send with START every 150 ms.
- **Strength [C]:** {Off, 25, 50, 75, 100} %, default 50; invert flips
  the sign.
- **Synthetic FFB (RRC) [C]**, for games without a motor:
  - `target = speed > 50 ? steer_locked × 0.02145 (scaled) : 0`,
    rate-limited, so the wheel is pulled to centre harder at speed.
  - Road impacts add `sin(g · 2π) × 0.4` kicks; collisions give a jolt of
    `(clamp(g/1500) × 0.5 + 0.5) × 0.8` held for 4 frames.
  - Jolts are suppressed until the car has been at speed for 60 frames.
- **VR drive-board decode [C]:** a 120 ms slew limit on the output.
- **Scud Race [I]:** a speed-dependent damper (0.05 → 0.30 over
  0–200 km/h), friction 0.04, impact max 0.9 with a 0.12 s decay, centre
  0.1, drift max 0.45.

## 6. Hot-plug [C]

A lost wheel falls back to the gamepad until re-plugged; with no wheel,
devices are rescanned at most every 2 s on a device change. Pedal and
button devices reconnect independently.

## 7. Shifter [C]

Sequential (paddles), H-pattern latched (press to engage, stays until
another gear), H-pattern held (release for neutral). Rad Mobile has no
gearbox input, so the launcher only keeps the wiring (paddles map to the
Light and Wiper buttons by default).

## 8. Settings defaults [C]

| Setting | Default | Values |
|---|---|---|
| Input device | Controller | Controller / Wheel |
| FFB strength | 50 % | Off, 25, 50, 75, 100 |
| Invert FFB | off | |
| Rumble | 100 % | Off / On |
| Steering lock | 270° | 180, 270, 360, 540, 900, Full |
| Stick deadzone | 3000 of 32767 | 0–16000, step 500 |
| Stick linearity | 0 | 0–100, step 10 |
| Trigger deadzone | 12 of 255 | 0–64, step 4 |
| Trigger linearity | 0 | 0–100, step 10 |

Menu order (WHEEL SETTINGS): input device, FFB strength, invert FFB,
steering lock, wheel button setup, wheel bindings, then (with a wheel)
gear shifter setup, invert accel, invert brake. Status line: "WHEEL
DETECTED", "NO WHEEL FOUND", "USING CONTROLLER (NO WHEEL)".
