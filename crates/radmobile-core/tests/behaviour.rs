//! Golden snapshots of how the controls behave, taken with rea-rad-mobile
//! 0.5.0's own wheel, pad, wizard and force-feedback code before it moved
//! onto rea_launcher::wheel and ::ffb (the SDK study's step 7). The move
//! must leave these unchanged, or change them on purpose and say so.
//!
//! Numbers are rounded to 4 decimals. `REA_UPDATE_SNAPSHOTS=1` rewrites
//! the snapshots. Synthetic devices only.

use std::fmt::Write as _;

use radmobile_core::controls::{Action, Button};
use radmobile_core::ffb::{FfbEngine, FfbSettings, Telemetry};
use radmobile_core::pad_map::PadMap;
use radmobile_core::settings::Settings;
use radmobile_core::wheel::{
    self, DeviceId, DeviceState, PadState, PedalAxis, SteeringLock, WheelProfile, WheelSettings,
    wizard,
};
use rea_launcher::snapshot;

// The few calls whose shape the SDK changes, in one place.

fn guess(id: DeviceId, initial: &DeviceState) -> WheelProfile {
    wheel::guess(id, initial)
}

fn actions() -> Vec<Action> {
    Action::all().collect()
}

fn full_wizard(steer_axis: usize) -> wizard::Wizard {
    wizard::full(steer_axis)
}

fn default_pad_map() -> PadMap {
    radmobile_core::pad_map::layout()
}

// ---------------------------------------------------------------- devices

fn csl_dd() -> DeviceId {
    DeviceId {
        name: "FANATEC CSL DD".into(),
        vid: 0x0eb7,
        pid: 0x0020,
        guid: "03000000b70e00002000000000000000".into(),
    }
}

fn pedals() -> DeviceId {
    DeviceId {
        name: "FANATEC CSL Pedals".into(),
        vid: 0x0eb7,
        pid: 0x6204,
        guid: "03000000b70e00000462000000000000".into(),
    }
}

fn wheel_at(axes: &[i16], pressed: &[usize]) -> DeviceState {
    let mut buttons = vec![false; 16];
    for &b in pressed {
        buttons[b] = true;
    }
    DeviceState {
        axes: axes.to_vec(),
        buttons,
    }
}

fn f(v: f32) -> String {
    // -0.0000 and 0.0000 are the same answer
    let s = format!("{v:.4}");
    if s == "-0.0000" { "0.0000".into() } else { s }
}

fn pressed(buttons: u32) -> String {
    let names: Vec<&str> = Button::ALL
        .iter()
        .filter(|b| buttons & b.bit() != 0)
        .map(|b| b.key())
        .collect();
    format!("[{}]", names.join(","))
}

/// The wizard's profile with the pedals on a second device.
fn aux_profile() -> WheelProfile {
    let mut p = guess(csl_dd(), &wheel_at(&[0, 0, 0], &[]));
    let captures = wizard::Captures {
        gas: Some((
            1,
            PedalAxis {
                axis: 0,
                rest: -32_768,
                peak: 31_900,
            },
        )),
        brake: Some((
            1,
            PedalAxis {
                axis: 1,
                rest: 32_767,
                peak: -30_500,
            },
        )),
        buttons: vec![
            (Button::Light, 0, 4),
            (Button::Wiper, 0, 5),
            (Button::Coin, 1, 0),
            (Button::Exit, 0, 11),
        ],
        skipped: vec![],
    };
    p.apply(&captures, &[csl_dd(), pedals()]);
    p
}

#[test]
fn a_wheel_reads_as_cabinet_controls() {
    let mut out = String::new();
    let guessed = guess(csl_dd(), &wheel_at(&[-130, 32_767, -32_768], &[]));
    let aux = aux_profile();
    for (name, p) in [("guessed", &guessed), ("aux pedals", &aux)] {
        writeln!(out, "== {name}").unwrap();
        for a in actions() {
            writeln!(out, "  describe {:?}: {}", a, p.describe(a)).unwrap();
        }
    }
    let locks = [
        SteeringLock::Degrees(180),
        SteeringLock::Degrees(270),
        SteeringLock::Degrees(900),
        SteeringLock::Full,
    ];
    let wheel_states = [
        wheel_at(&[-32_768, 32_767, -32_768], &[]),
        wheel_at(&[-8_192, 0, 0], &[4]),
        wheel_at(&[-130, -32_768, 32_767], &[5, 11]),
        wheel_at(&[3_000, 16_000, -16_000], &[]),
        wheel_at(&[8_192, -31_000, 31_000], &[9]),
        wheel_at(&[32_767, 32_000, -32_000], &[4, 5]),
        wheel_at(&[12_000], &[]),
    ];
    let aux_states = [
        None,
        Some(wheel_at(&[-32_768, 32_767], &[])),
        Some(wheel_at(&[0, 0], &[0])),
        Some(wheel_at(&[31_900, -30_500], &[0])),
        Some(wheel_at(&[31_000, 31_000], &[])),
    ];
    for (name, p) in [("guessed", &guessed), ("aux pedals", &aux)] {
        writeln!(out, "== {name}").unwrap();
        for lock in locks {
            for (invert_accel, invert_brake) in [(false, false), (true, false), (false, true)] {
                let s = WheelSettings {
                    lock,
                    invert_accel,
                    invert_brake,
                    device: String::new(),
                };
                for w in &wheel_states {
                    for a in &aux_states {
                        let c = p.read(&s, w, a.as_ref());
                        writeln!(
                            out,
                            "{} inv {}{} wheel {:?} {:?} aux {:?}: steer {} accel {} brake {} {}",
                            lock.label(),
                            u8::from(invert_accel),
                            u8::from(invert_brake),
                            w.axes,
                            w.buttons.iter().positions(),
                            a.as_ref().map(|a| (&a.axes, a.buttons.iter().positions())),
                            f(c.steer),
                            f(c.accel),
                            f(c.brake),
                            pressed(c.buttons),
                        )
                        .unwrap();
                    }
                }
            }
        }
    }
    snapshot!("behaviour-wheel-read.txt", out);
}

trait Positions {
    fn positions(self) -> Vec<usize>;
}

impl<'a, I: Iterator<Item = &'a bool>> Positions for I {
    fn positions(self) -> Vec<usize> {
        self.enumerate()
            .filter(|(_, b)| **b)
            .map(|(i, _)| i)
            .collect()
    }
}

#[test]
fn a_pad_reads_as_cabinet_controls() {
    let mut out = String::new();
    let map = default_pad_map();
    for a in actions() {
        writeln!(out, "describe {:?}: {}", a, map.describe(a)).unwrap();
    }
    let base = Settings::default().pad;
    let mut curvy = Settings::default().pad;
    curvy.stick_deadzone = 6_000;
    curvy.stick_linearity = 60;
    curvy.trigger_deadzone = 40;
    curvy.trigger_linearity = 100;
    let mut remapped = Settings::default().pad;
    remapped.map.assign(
        Action::Press(Button::Exit),
        radmobile_core::pad_map::PadInput::Y,
    );
    remapped.map.set(Action::Accelerate, &[]);
    let pads: Vec<(&str, PadState)> = vec![
        ("neutral", PadState::default()),
        (
            "stick half right",
            PadState {
                left_x: 16_000,
                ..Default::default()
            },
        ),
        (
            "stick in deadzone",
            PadState {
                left_x: -2_000,
                ..Default::default()
            },
        ),
        (
            "stick full left + dpad right",
            PadState {
                left_x: -32_768,
                dpad_right: true,
                ..Default::default()
            },
        ),
        (
            "both dpad",
            PadState {
                dpad_left: true,
                dpad_right: true,
                left_x: 9_000,
                ..Default::default()
            },
        ),
        (
            "triggers part way",
            PadState {
                right_trigger: 10_000,
                left_trigger: 20_000,
                ..Default::default()
            },
        ),
        (
            "a and b",
            PadState {
                a: true,
                b: true,
                right_trigger: 5_000,
                ..Default::default()
            },
        ),
        (
            "buttons",
            PadState {
                x: true,
                right_shoulder: true,
                start: true,
                ..Default::default()
            },
        ),
        (
            "coin",
            PadState {
                back: true,
                ..Default::default()
            },
        ),
        (
            "back + start",
            PadState {
                back: true,
                start: true,
                ..Default::default()
            },
        ),
        (
            "guide + start",
            PadState {
                guide: true,
                start: true,
                ..Default::default()
            },
        ),
        (
            "y",
            PadState {
                y: true,
                ..Default::default()
            },
        ),
    ];
    for (name, settings) in [
        ("default", &base),
        ("curvy", &curvy),
        ("remapped", &remapped),
    ] {
        writeln!(out, "== {name}").unwrap();
        for (pad_name, pad) in &pads {
            let c = wheel::read_pad(settings, pad);
            writeln!(
                out,
                "{pad_name}: steer {} accel {} brake {} {}",
                f(c.steer),
                f(c.accel),
                f(c.brake),
                pressed(c.buttons)
            )
            .unwrap();
        }
    }
    snapshot!("behaviour-pad-read.txt", out);
}

/// A full setup: the pedals on a second device, the buttons on the wheel,
/// one button never pressed (skipped after its timeout).
#[test]
fn the_setup_wizard_run() {
    let mut out = String::new();
    let rest = [wheel_at(&[0, 0, 0], &[]), wheel_at(&[-32_768, 32_767], &[])];
    let mut w = full_wizard(0);
    writeln!(out, "steps {}", w.step_count()).unwrap();
    let mut now = 0u64;
    let mut last = String::new();
    let mut tick = |w: &mut wizard::Wizard, devices: &[DeviceState], now: &mut u64| {
        *now += 4;
        let status = w.tick(*now, devices);
        // remaining_ms and travel change every tick; log the kind of status
        let kind = match &status {
            wizard::Status::WarmingUp => "warming up".to_string(),
            wizard::Status::Waiting { step, index, .. } => format!("waiting {step:?} #{index}"),
            wizard::Status::Pressing {
                step,
                index,
                travel,
            } => format!("pressing {step:?} #{index} travel>0.5 {}", *travel > 0.5),
            wizard::Status::Done(c) => format!("done {c:?}"),
        };
        if kind != last {
            writeln!(out, "{now:>6} ms {kind}").unwrap();
            last = kind;
        }
        status
    };
    let hold = |w: &mut wizard::Wizard,
                devices: &[DeviceState],
                ms: u64,
                now: &mut u64,
                tick: &mut dyn FnMut(
        &mut wizard::Wizard,
        &[DeviceState],
        &mut u64,
    ) -> wizard::Status| {
        for _ in 0..ms / 4 {
            tick(w, devices, now);
        }
    };
    hold(&mut w, &rest, 200, &mut now, &mut tick);
    // accelerator: device 1 axis 0, pressed all the way and let go
    for v in [-20_000, 0, 20_000, 31_900] {
        let d = [rest[0].clone(), wheel_at(&[v, 32_767], &[])];
        hold(&mut w, &d, 40, &mut now, &mut tick);
    }
    hold(&mut w, &rest, 200, &mut now, &mut tick);
    // brake: device 1 axis 1
    for v in [10_000, -10_000, -30_500] {
        let d = [rest[0].clone(), wheel_at(&[-32_768, v], &[])];
        hold(&mut w, &d, 40, &mut now, &mut tick);
    }
    hold(&mut w, &rest, 200, &mut now, &mut tick);
    // lights: wheel button 4
    hold(
        &mut w,
        &[wheel_at(&[0, 0, 0], &[4]), rest[1].clone()],
        40,
        &mut now,
        &mut tick,
    );
    hold(&mut w, &rest, 200, &mut now, &mut tick);
    // wiper: never pressed, times out
    hold(&mut w, &rest, 5_400, &mut now, &mut tick);
    // start: wheel button 9
    hold(
        &mut w,
        &[wheel_at(&[0, 0, 0], &[9]), rest[1].clone()],
        40,
        &mut now,
        &mut tick,
    );
    hold(&mut w, &rest, 200, &mut now, &mut tick);
    // exit: aux button 0
    hold(
        &mut w,
        &[rest[0].clone(), wheel_at(&[-32_768, 32_767], &[0])],
        40,
        &mut now,
        &mut tick,
    );
    hold(&mut w, &rest, 200, &mut now, &mut tick);
    snapshot!("behaviour-wizard.txt", out);
}

/// The synthetic force feedback through a scripted race, every 40 ms.
#[test]
fn force_feedback_through_a_race() {
    let mut out = String::new();
    let dt = 1.0 / 250.0;
    let racing = |speed: f32, lateral: f32| Telemetry {
        speed,
        lateral,
        playing: true,
        ..Default::default()
    };
    for s in [
        FfbSettings::default(),
        FfbSettings {
            strength: 30,
            invert: true,
            rumble: 50,
        },
    ] {
        writeln!(out, "== {s:?}").unwrap();
        let mut e = FfbEngine::new();
        let mut frame = 0u32;
        let mut run = |name: &str,
                       secs: f32,
                       steer: f32,
                       t: &dyn Fn(f32) -> Option<Telemetry>,
                       out: &mut String| {
            writeln!(out, "-- {name}").unwrap();
            let n = (secs / dt).round() as u32;
            for i in 0..n {
                let o = e.update(dt, steer, t(i as f32 / n as f32), &s);
                frame += 1;
                if frame.is_multiple_of(10) {
                    writeln!(
                        out,
                        "{frame:>5} const {} damper {} friction {} rumble {}",
                        f(o.constant),
                        f(o.damper),
                        f(o.friction),
                        f(o.rumble)
                    )
                    .unwrap();
                }
            }
        };
        run("attract mode", 0.4, 0.5, &|_| None, &mut out);
        run(
            "not playing",
            0.2,
            -0.4,
            &|_| {
                Some(Telemetry {
                    speed: 300.0,
                    ..Default::default()
                })
            },
            &mut out,
        );
        run(
            "race starts, speeding up",
            2.0,
            0.3,
            &|x| Some(racing(430.0 * x, 0.0)),
            &mut out,
        );
        run(
            "scraping the right barrier",
            0.6,
            0.3,
            &|_| Some(racing(300.0, 580.0)),
            &mut out,
        );
        run(
            "scraping the left barrier, slowly",
            0.4,
            -0.6,
            &|_| Some(racing(10.0, -590.0)),
            &mut out,
        );
        run(
            "crash",
            0.2,
            0.6,
            &|x| Some(racing(400.0 * (1.0 - x), 0.0)),
            &mut out,
        );
        run(
            "hard braking",
            2.0,
            0.0,
            &|x| Some(racing(400.0 * (1.0 - x), 0.0)),
            &mut out,
        );
        run("menu", 0.2, 0.2, &|_| None, &mut out);
        run(
            "a new race from the attract speed",
            0.6,
            0.0,
            &|x| Some(racing(if x < 0.1 { 400.0 } else { 0.0 }, 0.0)),
            &mut out,
        );
    }
    snapshot!("behaviour-ffb.txt", out);
}
