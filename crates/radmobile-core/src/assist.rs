//! Steering assist for gamepads.
//!
//! Rad Mobile's wheel sets how fast the car turns, and the car keeps its
//! angle when the wheel is centred again (authentic: MAME Testers 03230).
//! With a wheel you counter-steer without thinking; with a stick every tap
//! leaves the car drifting toward a barrier.
//!
//! The assist turns the stick into a sideways speed across the road
//! instead: centred holds the car's line, full lock a quick lane change.
//! The car moves sideways by its heading (`rea_heading`, docs/RAM_MAP.md)
//! minus the heading the road asks for: zero on a straight, growing
//! through a bend. That bend heading is not in RAM, so the assist learns
//! it from how the car actually moves. It then steers the game's wheel
//! toward the heading that gives the asked-for sideways speed. This is
//! the idea of the "auto-handling" control type in Sega's own Power Drift
//! port. Wheels never use it.

use crate::ffb::{Side, Telemetry};

/// Sideways speed at full stick, in lateral units a game tick (the game
/// updates every other frame): centre line to barrier in under half a
/// second.
pub const ACROSS_AT_FULL_STICK: f32 = 48.0;

/// Wheel per unit of heading error; tuned in the running game so the car
/// settles in about a quarter of a second without swinging back.
pub const GAIN: f32 = 0.035;

/// How much of each new measurement of the bend heading is taken.
const BEND_RATE: f32 = 0.5;

/// With the stick centred, the car is pulled back to where it was let go
/// at this fraction of the distance a tick, and at most `HOLD_MAX` a tick.
const HOLD_GAIN: f32 = 0.1;
const HOLD_MAX: f32 = 8.0;

/// Moving across less than this a tick, a car let go runs straight.
const SETTLED: f32 = 4.0;

/// A stick closer to centre than this counts as let go.
const CENTRED: f32 = 0.02;

/// The held line keeps this far inside the road's edges.
const HOLD_MARGIN: f32 = 60.0;

/// Against a barrier with the stick centred, the wheel turns away from it
/// at least this much.
const AWAY: f32 = 0.3;

/// The assist's memory over one race. Feed it every game frame with
/// [`Assist::observe`]; start a new one when the race is over.
#[derive(Debug, Clone, Default)]
pub struct Assist {
    /// The heading at which the car keeps its line on this piece of road.
    bend: f32,
    /// The two latest frames seen: frame number and lateral.
    recent: [Option<(u32, f32)>; 2],
    /// How far the car moved across the road in the last tick.
    across: f32,
    /// Where the car came to run straight after the stick was let go.
    hold: Option<f32>,
}

impl Assist {
    /// Learn from a game frame. Seeing the same frame again changes nothing.
    pub fn observe(&mut self, t: &Telemetry) {
        let frame = t.frame;
        if self.recent[1].is_some_and(|(f, _)| f == frame) {
            return;
        }
        // the lateral changes every other frame, so compare with two back
        let two_back = self
            .recent
            .iter()
            .flatten()
            .find(|(f, _)| frame.wrapping_sub(*f) == 2)
            .map(|&(_, lateral)| lateral);
        if let Some(before) = two_back {
            self.across = t.lateral - before;
            // a barrier stops the car whatever its heading: learn nothing
            let then = Telemetry {
                lateral: before,
                ..*t
            };
            if t.touching().is_none() && then.touching().is_none() {
                self.bend += BEND_RATE * (t.heading + self.across - self.bend);
            }
        }
        self.recent = [self.recent[1], Some((frame, t.lateral))];
    }

    /// The wheel position (-1 left ..= 1 right) for the stick (-1 ..= 1).
    pub fn steer(&mut self, stick: f32, t: &Telemetry) -> f32 {
        let stick = stick.clamp(-1.0, 1.0);
        let centred = stick.abs() < CENTRED;
        let across = if centred {
            // let go: straighten up, then keep the line the car settles on
            if self.hold.is_none() && self.across.abs() < SETTLED {
                self.hold = Some(t.lateral);
            }
            // on the road, even where it narrows after the car was let go
            let (left, right) = t.road();
            self.hold.map_or(0.0, |hold| {
                let hold = hold.clamp(
                    left + HOLD_MARGIN,
                    (right - HOLD_MARGIN).max(left + HOLD_MARGIN),
                );
                (HOLD_GAIN * (hold - t.lateral)).clamp(-HOLD_MAX, HOLD_MAX)
            })
        } else {
            self.hold = None;
            stick * ACROSS_AT_FULL_STICK
        };
        // the car moves right (lateral up) when it points right of the
        // bend heading, and the heading is positive pointing left
        let target = self.bend - across;
        let wheel = (-GAIN * (target - t.heading)).clamp(-1.0, 1.0);
        // at a barrier, never steer into it. Nothing is learnt there, so
        // what the assist knows of the bend may be stale: a stick pushed
        // away gets at least that much wheel, and a centred one eases off
        match t.touching() {
            Some(Side::Right) => wheel.min(0.0).min(if centred { -AWAY } else { stick }),
            Some(Side::Left) => wheel.max(0.0).max(if centred { AWAY } else { stick }),
            None => wheel,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn racing(lateral: f32, heading: f32) -> Telemetry {
        at(0, lateral, heading)
    }

    fn at(frame: u32, lateral: f32, heading: f32) -> Telemetry {
        Telemetry {
            speed: 430.0,
            lateral,
            heading,
            playing: true,
            frame,
            ..Default::default()
        }
    }

    #[test]
    fn centred_stick_on_a_straight_car_leaves_the_wheel_centred() {
        let mut a = Assist::default();
        assert_eq!(a.steer(0.0, &racing(0.0, 0.0)), 0.0);
    }

    #[test]
    fn a_car_pointing_left_on_a_straight_is_steered_right() {
        let mut a = Assist::default();
        assert!(a.steer(0.0, &racing(0.0, 20.0)) > 0.0);
        let mut a = Assist::default();
        assert!(a.steer(0.0, &racing(0.0, -20.0)) < 0.0);
    }

    #[test]
    fn a_stick_push_turns_that_way_hard() {
        let mut a = Assist::default();
        assert_eq!(a.steer(-1.0, &racing(0.0, 0.0)), -1.0);
        assert_eq!(a.steer(1.0, &racing(0.0, 0.0)), 1.0);
    }

    #[test]
    fn it_learns_the_bend_heading_from_how_the_car_moves() {
        let mut a = Assist::default();
        // pointing 100 left, yet holding its line: the road bends left
        for f in 0..20 {
            a.observe(&at(f, 200.0, 100.0));
        }
        assert!((a.bend - 100.0).abs() < 1.0, "bend {}", a.bend);
        // so a centred stick leaves the wheel alone
        assert!(a.steer(0.0, &racing(200.0, 100.0)).abs() < 0.05);
    }

    #[test]
    fn the_same_frame_twice_is_learnt_once() {
        let mut a = Assist::default();
        a.observe(&at(0, 0.0, 50.0));
        a.observe(&at(2, 0.0, 50.0));
        let once = a.bend;
        a.observe(&at(2, 0.0, 50.0));
        assert_eq!(a.bend, once);
    }

    #[test]
    fn nothing_is_learnt_against_a_barrier() {
        let mut a = Assist::default();
        for f in 0..20 {
            a.observe(&at(f, 590.0, 80.0));
        }
        assert_eq!(a.bend, 0.0);
    }

    #[test]
    fn at_a_barrier_it_steers_away_never_into_it() {
        // already pointing away: still eased off
        let mut a = Assist::default();
        assert_eq!(a.steer(0.0, &racing(590.0, 40.0)), -AWAY);
        let mut a = Assist::default();
        assert_eq!(a.steer(0.0, &racing(-590.0, -40.0)), AWAY);
        // even when the stick pushes into it
        assert!(a.steer(1.0, &racing(590.0, -40.0)) <= 0.0);
    }

    #[test]
    fn a_stick_pushed_off_a_barrier_gets_that_much_wheel() {
        // a stale bend says "turn right", the player says left
        let mut a = Assist {
            bend: -200.0,
            ..Default::default()
        };
        assert_eq!(a.steer(-0.8, &racing(590.0, 0.0)), -0.8);
    }

    #[test]
    fn the_held_line_moves_in_where_the_road_narrows() {
        // let go at -474, then the road narrows to +-350 inside +-380 barriers
        let mut a = Assist {
            hold: Some(-474.0),
            ..Default::default()
        };
        let narrow = Telemetry {
            barrier_left: -380.0,
            barrier_right: 380.0,
            edge_left: -350.0,
            edge_right: 350.0,
            ..racing(-300.0, 0.0)
        };
        assert!(a.steer(0.0, &narrow) >= 0.0, "not toward the barrier");
    }

    #[test]
    fn barriers_are_where_mame_says() {
        let mut a = Assist::default();
        let narrow = Telemetry {
            barrier_left: -380.0,
            barrier_right: 380.0,
            ..racing(370.0, 40.0)
        };
        assert_eq!(a.steer(0.0, &narrow), -AWAY);
    }

    #[test]
    fn a_car_let_go_on_a_barrier_eases_off_it_and_stays_off() {
        let g = drive(|_| 0.0, straight, 590.0, 300);
        assert!(g[299].1 < 555.0, "still at {}", g[299].1);
        assert!(g[200..].iter().all(|g| g.1 < 555.0));
    }

    /// The game as measured: every other frame the heading changes by
    /// about 0.12 per ADC step of wheel off centre (128 steps a side) plus
    /// the bend's own pull, and the car moves sideways by the heading the
    /// road asks for minus its heading (stopped by the barriers). The wheel
    /// reaches the ADC at most 48 steps a frame, and the launcher sees it
    /// all a frame late.
    struct Game {
        heading: f32,
        lateral: f32,
        road: f32,
        adc: f32,
    }

    /// `bend(frame)` is how much the road's heading grows a tick; the
    /// game turns the car along by half of it.
    fn drive(
        stick: impl Fn(u32) -> f32,
        bend: impl Fn(u32) -> f32,
        start: f32,
        frames: u32,
    ) -> Vec<(f32, f32)> {
        let mut g = Game {
            heading: 0.0,
            lateral: start,
            road: 0.0,
            adc: 0.0,
        };
        let mut a = Assist::default();
        let mut seen = at(0, start, 0.0);
        let mut out = Vec::new();
        for f in 0..frames {
            a.observe(&seen);
            let want = a.steer(stick(f), &seen) * 128.0;
            g.adc += (want - g.adc).clamp(-48.0, 48.0);
            if f % 2 == 0 {
                g.road += bend(f);
                g.heading += -0.12 * g.adc + 0.5 * bend(f);
                g.lateral = (g.lateral + g.road - g.heading).clamp(-600.0, 600.0);
            }
            seen = at(f + 1, g.lateral, g.heading);
            out.push((g.heading, g.lateral));
        }
        out
    }

    fn straight(_: u32) -> f32 {
        0.0
    }

    /// A long left bend from frame 40 on, as at the start of Los Angeles.
    fn left_bend(f: u32) -> f32 {
        if f >= 40 { 1.5 } else { 0.0 }
    }

    #[test]
    fn a_tap_moves_the_car_over_and_then_it_runs_straight() {
        let g = drive(
            |f| if (10..16).contains(&f) { -1.0 } else { 0.0 },
            straight,
            0.0,
            200,
        );
        let (heading, lateral) = g[199];
        assert!(
            lateral < -50.0 && lateral > -400.0,
            "about a lane: {lateral}"
        );
        assert!((lateral - g[150].1).abs() < 2.0, "then straight");
        assert!(heading.abs() < 1.0);
    }

    #[test]
    fn a_centred_stick_follows_a_bend() {
        let g = drive(|_| 0.0, left_bend, 0.0, 400);
        let widest = g.iter().map(|g| g.1.abs()).fold(0.0, f32::max);
        assert!(widest < 150.0, "thrown wide: {widest}");
    }

    #[test]
    fn a_tap_in_a_bend_moves_a_lane_and_holds_it() {
        let g = drive(
            |f| if (100..106).contains(&f) { -1.0 } else { 0.0 },
            left_bend,
            0.0,
            400,
        );
        let lateral = g[399].1;
        assert!(lateral < -50.0 && lateral > -400.0, "a lane: {lateral}");
        assert!((lateral - g[300].1).abs() < 20.0, "and holds it");
    }

    #[test]
    fn the_stick_pulls_the_car_off_the_outside_barrier_of_a_bend() {
        // pressed against the right barrier in a left bend: push left
        let g = drive(|f| if f >= 100 { -1.0 } else { 0.0 }, left_bend, 560.0, 160);
        assert!(g[159].1 < 0.0, "still at {}", g[159].1);
    }

    #[test]
    fn holding_the_stick_moves_across_at_a_steady_speed_without_swinging() {
        let g = drive(|_| -1.0, straight, 500.0, 24);
        let step = g[22].1 - g[20].1;
        assert!(
            (step + ACROSS_AT_FULL_STICK).abs() < 6.0,
            "across a tick {step}"
        );
    }

    #[test]
    fn quick_stick_moves_do_not_run_away() {
        let flick = |f: u32| match f % 40 {
            0..6 => -1.0,
            20..26 => 1.0,
            _ => 0.0,
        };
        let g = drive(flick, left_bend, 0.0, 400);
        let widest = g.iter().map(|g| g.1.abs()).fold(0.0, f32::max);
        assert!(widest < 400.0, "no runaway: {widest}");
    }
}
