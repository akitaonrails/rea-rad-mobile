//! Force feedback on the wheel through SDL's haptic API (DirectInput on
//! Windows, evdev on Linux), set up the way Wanszai's wrappers do it
//! (docs/WANSZAI_WHEELS.md §5): a constant force plus damper and friction
//! conditions, autocentre off, parameters sent only when they change, the
//! effect restarted every 150 ms (firmwares that drop an idle effect), and
//! its status checked every 250 ms.

use std::ptr::NonNull;
use std::time::{Duration, Instant};

use radmobile_core::ffb::FfbOutput;
use sdl2::sys;

const KEEPALIVE: Duration = Duration::from_millis(150);
const REVIVE_CHECK: Duration = Duration::from_millis(250);
/// Changes smaller than this are not worth a USB transfer.
const EPSILON: f32 = 1.0 / 512.0;

pub struct Haptic {
    dev: NonNull<sys::SDL_Haptic>,
    constant: Option<i32>,
    damper: Option<i32>,
    friction: Option<i32>,
    sent: FfbOutput,
    last_keepalive: Instant,
    last_check: Instant,
    pub description: String,
}

// SAFETY: used only on the input thread that opened it.
unsafe impl Send for Haptic {}

impl Haptic {
    /// Open force feedback on a joystick, if it has any.
    pub fn open(joystick: *mut sys::SDL_Joystick) -> Option<Self> {
        // SAFETY: the joystick is open on this thread; nulls are handled
        unsafe {
            if sys::SDL_JoystickIsHaptic(joystick) != 1 {
                return None;
            }
            let dev = NonNull::new(sys::SDL_HapticOpenFromJoystick(joystick))?;
            let h = dev.as_ptr();
            let caps = sys::SDL_HapticQuery(h);
            crate::log::line(&format!("ffb: capabilities {caps:#06x}"));
            if caps & sys::SDL_HAPTIC_AUTOCENTER != 0 {
                sys::SDL_HapticSetAutocenter(h, 0);
            }
            if caps & sys::SDL_HAPTIC_GAIN != 0 {
                sys::SDL_HapticSetGain(h, 100);
            }
            let make = |kind: u32| -> Option<i32> {
                if caps & kind == 0 {
                    return None;
                }
                let mut effect = effect_for(kind, 0.0);
                let id = sys::SDL_HapticNewEffect(h, &mut effect);
                if id < 0 {
                    crate::log::line(&format!(
                        "ffb: effect {kind:#x} refused: {}",
                        super::sdl::error()
                    ));
                }
                (id >= 0).then(|| {
                    sys::SDL_HapticRunEffect(h, id, 1);
                    id
                })
            };
            let constant = make(sys::SDL_HAPTIC_CONSTANT);
            let damper = make(sys::SDL_HAPTIC_DAMPER);
            let friction = make(sys::SDL_HAPTIC_FRICTION);
            if constant.is_none() {
                sys::SDL_HapticClose(h);
                return None;
            }
            let description = format!(
                "constant{}{}",
                if damper.is_some() { " + damper" } else { "" },
                if friction.is_some() {
                    " + friction"
                } else {
                    ""
                }
            );
            let now = Instant::now();
            Some(Haptic {
                dev,
                constant,
                damper,
                friction,
                sent: FfbOutput::default(),
                last_keepalive: now,
                last_check: now,
                description,
            })
        }
    }

    /// Send the new forces (only what changed) and keep the effects alive.
    pub fn apply(&mut self, out: &FfbOutput) {
        let h = self.dev.as_ptr();
        let now = Instant::now();
        // SAFETY: the device and effect ids are ours and valid
        unsafe {
            let update = |id: Option<i32>, kind: u32, new: f32, old: f32| {
                if let Some(id) = id
                    && (new - old).abs() > EPSILON
                {
                    let mut effect = effect_for(kind, new);
                    sys::SDL_HapticUpdateEffect(h, id, &mut effect);
                }
            };
            update(
                self.constant,
                sys::SDL_HAPTIC_CONSTANT,
                out.constant,
                self.sent.constant,
            );
            update(
                self.damper,
                sys::SDL_HAPTIC_DAMPER,
                out.damper,
                self.sent.damper,
            );
            update(
                self.friction,
                sys::SDL_HAPTIC_FRICTION,
                out.friction,
                self.sent.friction,
            );
            self.sent = *out;

            if now.duration_since(self.last_keepalive) >= KEEPALIVE {
                self.last_keepalive = now;
                if let Some(id) = self.constant {
                    sys::SDL_HapticRunEffect(h, id, 1);
                }
            }
            if now.duration_since(self.last_check) >= REVIVE_CHECK {
                self.last_check = now;
                for id in [self.damper, self.friction].into_iter().flatten() {
                    // 0 = stopped (or the query is unsupported, where running again is harmless)
                    if sys::SDL_HapticGetEffectStatus(h, id) == 0 {
                        sys::SDL_HapticRunEffect(h, id, 1);
                    }
                }
            }
        }
    }

    /// Release the wheel: no force at all.
    pub fn stop(&mut self) {
        self.apply(&FfbOutput::default());
        // SAFETY: the device is open
        unsafe { sys::SDL_HapticStopAll(self.dev.as_ptr()) };
    }
}

impl Drop for Haptic {
    fn drop(&mut self) {
        // SAFETY: we opened it; closing destroys the effects
        unsafe {
            sys::SDL_HapticStopAll(self.dev.as_ptr());
            sys::SDL_HapticClose(self.dev.as_ptr());
        }
    }
}

/// A long-running effect of `kind` with strength `v`: a level in −1..1 for
/// the constant force, a coefficient in 0..1 for the conditions.
/// SDL gives the direction a force comes *from* (SDL_haptic.h). A force
/// from the west pushes the wheel east, so a positive level pushes right,
/// as [`FfbOutput::constant`] means it. East (`[1, 0, 0]`) turned every
/// force around: centring pulled away from the centre (seen on a Fanatec
/// CSL DD on Windows).
const FROM_WEST: [i32; 3] = [-1, 0, 0];

fn effect_for(kind: u32, v: f32) -> sys::SDL_HapticEffect {
    // SAFETY: SDL_HapticEffect is plain data; zero is a valid starting point
    let mut e: sys::SDL_HapticEffect = unsafe { std::mem::zeroed() };
    let dir = sys::SDL_HapticDirection {
        type_: sys::SDL_HAPTIC_CARTESIAN as u8,
        dir: FROM_WEST,
    };
    if kind == sys::SDL_HAPTIC_CONSTANT {
        e.constant = sys::SDL_HapticConstant {
            type_: kind as u16,
            direction: dir,
            length: sys::SDL_HAPTIC_INFINITY,
            delay: 0,
            button: 0,
            interval: 0,
            level: (v.clamp(-1.0, 1.0) * 32767.0) as i16,
            attack_length: 0,
            attack_level: 0,
            fade_length: 0,
            fade_level: 0,
        };
    } else {
        let coeff = (v.clamp(0.0, 1.0) * 32767.0) as i16;
        e.condition = sys::SDL_HapticCondition {
            type_: kind as u16,
            direction: dir,
            length: sys::SDL_HAPTIC_INFINITY,
            delay: 0,
            button: 0,
            interval: 0,
            right_sat: [0xffff; 3],
            left_sat: [0xffff; 3],
            right_coeff: [coeff; 3],
            left_coeff: [coeff; 3],
            deadband: [0; 3],
            center: [0; 3],
        };
    }
    e
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_positive_force_comes_from_the_west_so_it_pushes_right() {
        let e = effect_for(sys::SDL_HAPTIC_CONSTANT, 0.5);
        // SAFETY: effect_for filled the constant variant
        let c = unsafe { e.constant };
        assert_eq!(c.direction.type_, sys::SDL_HAPTIC_CARTESIAN as u8);
        assert!(c.direction.dir[0] < 0, "from the west");
        assert!(c.level > 0);
    }
}
