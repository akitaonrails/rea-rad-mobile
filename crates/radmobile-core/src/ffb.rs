//! Synthetic force feedback for Rad Mobile, by
//! [`rea_launcher::ffb::SyntheticFfb`] with [`Tuning::ARCADE`] (this
//! launcher's numbers, moved to the SDK) from the game's [`Telemetry`].
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

use rea_launcher::ffb::SyntheticFfb;
pub use rea_launcher::ffb::{CarState, FfbOutput, FfbSettings, Side, Tuning, test_pattern};

/// Rad Mobile's speed at full throttle on the flat (internal units).
pub const TOP_SPEED: f32 = 430.0;
/// The barriers of the first section, where MAME reports none.
const FIRST_BARRIER: f32 = 600.0;

/// This close to a barrier the car is touching it (it bounces back
/// from the barrier by up to about this much).
pub const TOUCHING: f32 = 45.0;

/// What MAME reports every frame.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Telemetry {
    pub speed: f32,
    /// Across the road; negative is left.
    pub lateral: f32,
    /// The car's heading; positive points left. Its angle to the road on
    /// a straight; through a bend it turns along with the road.
    pub heading: f32,
    pub playing: bool,
    /// MAME's frame count, one a frame.
    pub frame: u32,
    /// Where this section's barriers stop the car (left is negative).
    /// Both 0 when unknown.
    pub barrier_left: f32,
    pub barrier_right: f32,
    /// Where the road surface ends; the barriers may stand further out.
    pub edge_left: f32,
    pub edge_right: f32,
}

impl Telemetry {
    /// The barriers (left, right), or the first section's when unknown.
    pub fn barriers(&self) -> (f32, f32) {
        if self.barrier_left < self.barrier_right {
            (self.barrier_left, self.barrier_right)
        } else {
            (-FIRST_BARRIER, FIRST_BARRIER)
        }
    }

    /// The road's edges (left, right), never outside the barriers.
    pub fn road(&self) -> (f32, f32) {
        let (l, r) = self.barriers();
        if self.edge_left < self.edge_right {
            (self.edge_left.max(l), self.edge_right.min(r))
        } else {
            (l, r)
        }
    }

    /// The barrier the car is touching, if any.
    pub fn touching(&self) -> Option<Side> {
        let (l, r) = self.barriers();
        if self.lateral >= r - TOUCHING {
            Some(Side::Right)
        } else if self.lateral <= l + TOUCHING {
            Some(Side::Left)
        } else {
            None
        }
    }

    /// The engine's view of the car: the speed as a share of
    /// [`TOP_SPEED`] (600 units/s of speed loss, the hit threshold, is
    /// [`Tuning::ARCADE`]'s 600/430 top speeds a second).
    pub fn car_state(&self) -> CarState {
        CarState {
            racing: self.playing,
            speed: self.speed / TOP_SPEED,
            contact: self.touching(),
        }
    }
}

/// The force feedback engine for a race session.
#[derive(Debug, Clone)]
pub struct FfbEngine(SyntheticFfb);

impl Default for FfbEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl FfbEngine {
    pub fn new() -> Self {
        Self(SyntheticFfb::new(Tuning::ARCADE))
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
        self.0
            .update(dt, steer, telemetry.map(|t| t.car_state()), s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hit_threshold_is_600_units_a_second() {
        assert!((Tuning::ARCADE.hit_decel * TOP_SPEED - 600.0).abs() < 1e-3);
        let t = Telemetry {
            speed: 215.0,
            lateral: 590.0,
            playing: true,
            ..Default::default()
        };
        assert_eq!(
            t.car_state(),
            CarState {
                racing: true,
                speed: 0.5,
                contact: Some(Side::Right)
            }
        );
    }

    #[test]
    fn the_barriers_move_with_the_section() {
        let narrow = |lateral| Telemetry {
            lateral,
            barrier_left: -380.0,
            barrier_right: 380.0,
            ..Default::default()
        };
        assert_eq!(narrow(-375.0).touching(), Some(Side::Left));
        assert_eq!(narrow(300.0).touching(), None);
        // without MAME's limits, the first section's
        assert_eq!(Telemetry::default().barriers(), (-600.0, 600.0));
    }

    #[test]
    fn the_road_never_reaches_past_the_barriers() {
        let wide = Telemetry {
            barrier_left: -1000.0,
            barrier_right: 1100.0,
            edge_left: -380.0,
            edge_right: 350.0,
            ..Default::default()
        };
        assert_eq!(wide.road(), (-380.0, 350.0));
        let odd = Telemetry {
            barrier_left: -380.0,
            barrier_right: 380.0,
            edge_left: -600.0,
            edge_right: 600.0,
            ..Default::default()
        };
        assert_eq!(odd.road(), (-380.0, 380.0));
    }
}
