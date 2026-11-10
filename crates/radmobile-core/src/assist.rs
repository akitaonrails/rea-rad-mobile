//! Steering assist for gamepads.
//!
//! Rad Mobile's wheel sets how fast the car turns, and the car keeps its
//! angle to the road when the wheel is centred again (authentic: MAME
//! Testers 03230). With a wheel you counter-steer without thinking; with a
//! stick every tap leaves the car drifting toward a barrier.
//!
//! The assist turns the stick into an angle to the road instead: centred
//! means "straight down the road", full lock a steady lane change. It
//! steers the game's wheel toward that angle from the heading MAME reports
//! (`rea_heading`, docs/RAM_MAP.md), the idea of the "auto-handling"
//! control type in Sega's own Power Drift port. Bends still throw the car
//! outward, as they do for a wheel player: brake and hold the stick into
//! them. Wheels never use it.

use crate::ffb::{BARRIER_LATERAL, Telemetry};

/// The car's angle to the road at full stick, in the game's units. On a
/// straight the car moves sideways by half its heading a frame (measured
/// at every racing speed), so full stick moves it 24 units a frame: centre
/// line to barrier in under half a second.
pub const HEADING_AT_FULL_STICK: f32 = 48.0;

/// Wheel per unit of heading error; tuned in the running game so the car
/// settles in about a quarter of a second without swinging back.
pub const GAIN: f32 = 0.035;

/// Closer to a barrier than this, the car is touching it.
const NEAR_WALL: f32 = BARRIER_LATERAL - 15.0;

/// The wheel position (-1 left ..= 1 right) that brings the car to the
/// angle the stick asks for.
pub fn steer(stick: f32, t: &Telemetry) -> f32 {
    // the heading is positive when the car points left of the road
    let target = -stick.clamp(-1.0, 1.0) * HEADING_AT_FULL_STICK;
    let wheel = (-GAIN * (target - t.heading)).clamp(-1.0, 1.0);
    // never steer into a barrier the car is touching: one bounced off it
    // already points away and is let go
    if t.lateral >= NEAR_WALL {
        wheel.min(0.0)
    } else if t.lateral <= -NEAR_WALL {
        wheel.max(0.0)
    } else {
        wheel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn racing(lateral: f32, heading: f32) -> Telemetry {
        Telemetry {
            speed: 430.0,
            lateral,
            heading,
            playing: true,
        }
    }

    #[test]
    fn centred_stick_on_a_straight_car_leaves_the_wheel_centred() {
        assert_eq!(steer(0.0, &racing(0.0, 0.0)), 0.0);
    }

    #[test]
    fn a_car_pointing_left_is_steered_right() {
        assert!(steer(0.0, &racing(0.0, 20.0)) > 0.0);
        assert!(steer(0.0, &racing(0.0, -20.0)) < 0.0);
    }

    #[test]
    fn a_stick_push_from_straight_turns_that_way_hard() {
        assert_eq!(steer(-1.0, &racing(0.0, 0.0)), -1.0);
        assert_eq!(steer(1.0, &racing(0.0, 0.0)), 1.0);
    }

    #[test]
    fn at_the_asked_angle_the_wheel_is_centred() {
        let half = 0.5 * HEADING_AT_FULL_STICK;
        assert_eq!(steer(-0.5, &racing(0.0, half)), 0.0);
    }

    #[test]
    fn it_never_steers_into_a_barrier_it_touches() {
        // at the right barrier, pointing left (away): left alone
        assert_eq!(steer(0.0, &racing(590.0, 13.0)), 0.0);
        assert_eq!(steer(0.0, &racing(-590.0, -13.0)), 0.0);
        // but the stick can still steer away from it
        assert!(steer(-1.0, &racing(590.0, 0.0)) < 0.0);
    }

    /// The game on a straight, as measured: every other frame the heading
    /// changes by about 0.12 per ADC step of wheel off centre (128 steps a
    /// side), the wheel reaches the ADC at most 48 steps a frame, the car
    /// moves sideways by the heading of that frame, and the launcher sees
    /// it all a frame late.
    fn drive(stick: impl Fn(u32) -> f32, frames: u32) -> Vec<(f32, f32)> {
        let (mut heading, mut lateral, mut adc) = (0.0_f32, 0.0_f32, 0.0_f32);
        let mut seen = racing(0.0, 0.0);
        let mut out = Vec::new();
        for f in 0..frames {
            let want = steer(stick(f), &seen) * 128.0;
            adc += (want - adc).clamp(-48.0, 48.0);
            if f % 2 == 0 {
                heading -= 0.12 * adc;
                lateral = (lateral - heading).clamp(-600.0, 600.0);
            }
            seen = racing(lateral, heading);
            out.push((heading, lateral));
        }
        out
    }

    #[test]
    fn a_tap_moves_the_car_over_and_then_it_runs_straight() {
        let g = drive(|f| if (10..16).contains(&f) { -1.0 } else { 0.0 }, 200);
        let (heading, lateral) = g[199];
        assert!(
            lateral < -50.0 && lateral > -400.0,
            "about a lane: {lateral}"
        );
        assert_eq!(lateral, g[150].1, "then straight");
        assert!(heading.abs() < 1.0);
    }

    #[test]
    fn holding_the_stick_settles_on_its_angle_without_swinging() {
        let g = drive(|_| -1.0, 40);
        let peak = g.iter().map(|g| g.0).fold(f32::MIN, f32::max);
        assert!(peak < HEADING_AT_FULL_STICK * 1.3, "overshoot {peak}");
        let end = g[39].0;
        assert!((end - HEADING_AT_FULL_STICK).abs() < 4.0, "settles: {end}");
    }

    #[test]
    fn quick_stick_moves_do_not_run_away() {
        let flick = |f: u32| match f % 40 {
            0..6 => -1.0,
            20..26 => 1.0,
            _ => 0.0,
        };
        let g = drive(flick, 400);
        let widest = g.iter().map(|g| g.1.abs()).fold(0.0, f32::max);
        assert!(widest < 400.0, "no runaway: {widest}");
    }
}
