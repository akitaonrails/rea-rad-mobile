//! Synthetic force feedback for Rad Mobile.
//!
//! The arcade had no steering motor, so, like Wanszai's Ridge Racer 1/2,
//! the force is made from the car's state (docs/WANSZAI_WHEELS.md §5,
//! docs/RAM_MAP.md):
//!
//! - **Centring:** a spring toward the centre that stiffens with speed
//!   (the cabinet's own spring when stopped), slew-limited like VR's 120 ms.
//! - **Damping and friction:** Scud Race's speed-dependent damper and a
//!   little friction, so the wheel has weight.
//! - **Barrier scrape:** with the car pressed against the side of the road
//!   the wheel buzzes and is pushed away from the wall.
//! - **Collisions:** a sudden loss of speed gives a jolt held for four
//!   frames, as in Ridge Racer Collection.
//!
//! Outside a player's race (attract mode, test menu) only the gentle
//! centring spring remains.

/// Rad Mobile's speed at full throttle on the flat (internal units).
pub const TOP_SPEED: f32 = 430.0;
/// Lateral position where the car meets the barriers (they hold it
/// between about -600 and 600).
pub const BARRIER_LATERAL: f32 = 555.0;

/// What MAME reports every frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Telemetry {
    pub speed: f32,
    /// Across the road; negative is left.
    pub lateral: f32,
    /// The car's angle to the road; positive points left.
    pub heading: f32,
    pub playing: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FfbSettings {
    /// 0 (off) ..= 100.
    pub strength: u8,
    pub invert: bool,
    /// Gamepad rumble, 0 ..= 100.
    pub rumble: u8,
}

impl Default for FfbSettings {
    fn default() -> Self {
        Self {
            strength: 50,
            invert: false,
            rumble: 100,
        }
    }
}

/// Forces for the device, each already scaled by the strength setting.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct FfbOutput {
    /// −1 pushes the wheel left, +1 right.
    pub constant: f32,
    /// 0..1 damper coefficient.
    pub damper: f32,
    /// 0..1 friction coefficient.
    pub friction: f32,
    /// 0..1 for gamepads.
    pub rumble: f32,
}

const SPRING_STOPPED: f32 = 0.25;
const SPRING_TOP: f32 = 1.0;
const SPRING_IDLE: f32 = 0.2;
const DAMPER_STOPPED: f32 = 0.05;
const DAMPER_TOP: f32 = 0.30;
const FRICTION: f32 = 0.04;
/// Full-scale change of the constant force takes 120 ms.
const SLEW_PER_SEC: f32 = 1.0 / 0.120;
/// Speed loss (units per second) that counts as a hit; hard braking
/// stays well under 150.
const HIT_DECEL: f32 = 600.0;
const JOLT_HOLD_SEC: f32 = 4.0 / 60.0;
const SCRAPE_FORCE: f32 = 0.3;
const SCRAPE_BUZZ: f32 = 0.2;
const SCRAPE_HZ: f32 = 14.0;
/// Speed is smoothed over this time before the hit test.
const SPEED_WINDOW_SEC: f32 = 0.05;
/// Jolts stay off for the first second of a race: the speed jumps from
/// the attract demo's to zero when a game starts (Ridge Racer Collection
/// waits 60 frames the same way).
const JOLT_ARM_SEC: f32 = 1.0;

#[derive(Debug, Clone, Default)]
pub struct FfbEngine {
    constant: f32,
    smoothed_speed: Option<f32>,
    jolt: f32,
    jolt_left: f32,
    phase: f32,
    racing_for: f32,
}

impl FfbEngine {
    pub fn new() -> Self {
        Self::default()
    }

    /// Advance by `dt` seconds. `steer` is the wheel's position within the
    /// steering lock (−1..1); `telemetry` is `None` when MAME is not
    /// reporting (not running, or another game).
    pub fn update(
        &mut self,
        dt: f32,
        steer: f32,
        telemetry: Option<Telemetry>,
        s: &FfbSettings,
    ) -> FfbOutput {
        let dt = dt.clamp(0.0, 0.1);
        let steer = steer.clamp(-1.0, 1.0);
        let racing = telemetry.filter(|t| t.playing);

        let (target, damper, friction, scrape) = match racing {
            Some(t) => {
                let speed = (t.speed / TOP_SPEED).clamp(0.0, 1.0);
                let spring = SPRING_STOPPED + (SPRING_TOP - SPRING_STOPPED) * speed;
                let mut target = -steer * spring;
                let scrape = if t.lateral.abs() >= BARRIER_LATERAL && speed > 0.05 {
                    speed
                } else {
                    0.0
                };
                if scrape > 0.0 {
                    self.phase = (self.phase + dt * SCRAPE_HZ).fract();
                    let buzz = (self.phase * std::f32::consts::TAU).sin() * SCRAPE_BUZZ * scrape;
                    target += -t.lateral.signum() * SCRAPE_FORCE * scrape + buzz;
                }
                (
                    target,
                    DAMPER_STOPPED + (DAMPER_TOP - DAMPER_STOPPED) * speed,
                    FRICTION,
                    scrape,
                )
            }
            None => (-steer * SPRING_IDLE, DAMPER_STOPPED, FRICTION, 0.0),
        };

        self.detect_hit(dt, steer, racing);

        let max_step = SLEW_PER_SEC * dt;
        self.constant += (target - self.constant).clamp(-max_step, max_step);

        let mut constant = self.constant;
        let mut rumble = scrape * 0.6;
        if self.jolt_left > 0.0 {
            self.jolt_left -= dt;
            constant = self.jolt;
            rumble = rumble.max(self.jolt.abs());
        }

        let gain = f32::from(s.strength.min(100)) / 100.0;
        let sign = if s.invert { -1.0 } else { 1.0 };
        FfbOutput {
            constant: (constant * gain * sign).clamp(-1.0, 1.0),
            damper: (damper * gain).clamp(0.0, 1.0),
            friction: (friction * gain).clamp(0.0, 1.0),
            rumble: (rumble * f32::from(s.rumble.min(100)) / 100.0).clamp(0.0, 1.0),
        }
    }

    fn detect_hit(&mut self, dt: f32, steer: f32, racing: Option<Telemetry>) {
        let Some(t) = racing else {
            self.smoothed_speed = None;
            self.racing_for = 0.0;
            return;
        };
        self.racing_for += dt;
        let prev = self.smoothed_speed.unwrap_or(t.speed);
        let alpha = if dt > 0.0 {
            (dt / SPEED_WINDOW_SEC).min(1.0)
        } else {
            0.0
        };
        let now = prev + (t.speed - prev) * alpha;
        self.smoothed_speed = Some(now);
        if dt <= 0.0 || self.jolt_left > 0.0 || self.racing_for < JOLT_ARM_SEC {
            return;
        }
        let decel = (prev - now) / dt;
        if decel > HIT_DECEL {
            let size = (decel / (HIT_DECEL * 4.0)).min(1.0);
            let magnitude = (size * 0.5 + 0.5) * 0.8;
            // throw the wheel against the way it is turned
            let dir = if steer.abs() < 0.05 {
                1.0
            } else {
                -steer.signum()
            };
            self.jolt = dir * magnitude;
            self.jolt_left = JOLT_HOLD_SEC;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DT: f32 = 1.0 / 250.0;

    fn run(
        engine: &mut FfbEngine,
        secs: f32,
        steer: f32,
        t: Option<Telemetry>,
        s: &FfbSettings,
    ) -> FfbOutput {
        let mut out = FfbOutput::default();
        for _ in 0..(secs / DT) as usize {
            out = engine.update(DT, steer, t, s);
        }
        out
    }

    fn racing(speed: f32) -> Option<Telemetry> {
        Some(Telemetry {
            speed,
            lateral: 0.0,
            heading: 0.0,
            playing: true,
        })
    }

    #[test]
    fn centring_stiffens_with_speed() {
        let s = FfbSettings {
            strength: 100,
            ..Default::default()
        };
        let slow = run(&mut FfbEngine::new(), 0.5, 0.5, racing(0.0), &s);
        let fast = run(&mut FfbEngine::new(), 0.5, 0.5, racing(TOP_SPEED), &s);
        assert!(
            slow.constant < 0.0 && fast.constant < 0.0,
            "pulls back to the left"
        );
        assert!((slow.constant + 0.125).abs() < 1e-3);
        assert!((fast.constant + 0.5).abs() < 1e-3);
        assert!(fast.damper > slow.damper);
    }

    #[test]
    fn the_force_is_slew_limited() {
        let s = FfbSettings {
            strength: 100,
            ..Default::default()
        };
        let mut e = FfbEngine::new();
        let first = e.update(0.01, 1.0, racing(TOP_SPEED), &s);
        assert!((first.constant + 0.01 * SLEW_PER_SEC).abs() < 1e-4);
    }

    #[test]
    fn strength_and_invert_scale_everything() {
        let full = run(
            &mut FfbEngine::new(),
            0.5,
            0.5,
            racing(200.0),
            &FfbSettings {
                strength: 100,
                ..Default::default()
            },
        );
        let half = run(
            &mut FfbEngine::new(),
            0.5,
            0.5,
            racing(200.0),
            &FfbSettings {
                strength: 50,
                ..Default::default()
            },
        );
        let inv = run(
            &mut FfbEngine::new(),
            0.5,
            0.5,
            racing(200.0),
            &FfbSettings {
                strength: 100,
                invert: true,
                rumble: 100,
            },
        );
        let off = run(
            &mut FfbEngine::new(),
            0.5,
            0.5,
            racing(200.0),
            &FfbSettings {
                strength: 0,
                ..Default::default()
            },
        );
        assert!((half.constant - full.constant / 2.0).abs() < 1e-4);
        assert!((half.damper - full.damper / 2.0).abs() < 1e-4);
        assert!((inv.constant + full.constant).abs() < 1e-4);
        assert_eq!(off.constant, 0.0);
        assert_eq!(off.damper, 0.0);
    }

    #[test]
    fn outside_a_race_only_the_idle_spring_remains() {
        let s = FfbSettings {
            strength: 100,
            ..Default::default()
        };
        let demo = Some(Telemetry {
            speed: TOP_SPEED,
            lateral: 600.0,
            heading: 0.0,
            playing: false,
        });
        let out = run(&mut FfbEngine::new(), 0.5, 1.0, demo, &s);
        assert!((out.constant + SPRING_IDLE).abs() < 1e-3);
        assert_eq!(out.rumble, 0.0);
        let none = run(&mut FfbEngine::new(), 0.5, 1.0, None, &s);
        assert_eq!(none, out);
    }

    #[test]
    fn a_crash_jolts_but_braking_does_not() {
        let s = FfbSettings {
            strength: 100,
            ..Default::default()
        };
        let mut e = FfbEngine::new();
        run(&mut e, 0.5, 0.3, racing(400.0), &s);
        // hard braking: about 80 units a second
        let mut speed = 400.0;
        for _ in 0..250 {
            speed -= 80.0 * DT;
            let out = e.update(DT, 0.3, racing(speed), &s);
            assert!(out.rumble == 0.0, "braking must not jolt");
        }
        // a wall: 250 units lost in a tenth of a second
        let mut jolted = false;
        for _ in 0..25 {
            speed = (speed - 10.0).max(50.0);
            let out = e.update(DT, 0.3, racing(speed), &s);
            jolted |= out.rumble > 0.5 && out.constant < -0.4;
        }
        assert!(jolted, "the hit throws the wheel against the turn");
    }

    #[test]
    fn a_new_race_does_not_jolt() {
        let s = FfbSettings {
            strength: 100,
            ..Default::default()
        };
        let mut e = FfbEngine::new();
        // the attract demo drives fast, then a player's race starts from zero
        run(
            &mut e,
            1.0,
            0.0,
            Some(Telemetry {
                speed: 400.0,
                lateral: 0.0,
                heading: 0.0,
                playing: false,
            }),
            &s,
        );
        let out = run(&mut e, 0.5, 0.0, racing(0.0), &s);
        assert_eq!(out.rumble, 0.0);
        assert!(out.constant.abs() < 0.01);
    }

    #[test]
    fn scraping_the_barrier_pushes_away_and_buzzes() {
        let s = FfbSettings {
            strength: 100,
            ..Default::default()
        };
        let right_wall = Some(Telemetry {
            speed: 300.0,
            lateral: 580.0,
            heading: 0.0,
            playing: true,
        });
        let out = run(&mut FfbEngine::new(), 0.3, 0.0, right_wall, &s);
        assert!(out.rumble > 0.3);
        let mut e = FfbEngine::new();
        let mut sum = 0.0;
        for _ in 0..250 {
            sum += e.update(DT, 0.0, right_wall, &s).constant;
        }
        assert!(
            sum / 250.0 < -0.1,
            "on average pushed left, away from the wall"
        );
    }
}
