//! The wheel setup wizard: press each pedal and button when asked.
//!
//! A pure state machine fed with device snapshots, so it is tested without
//! hardware. Thresholds follow Wanszai's wizard (docs/WANSZAI_WHEELS.md §4):
//! an axis is captured when it moves more than 16000 from where it rested
//! and is let go again (back within 8000); a button when it goes down.
//! Steps time out (10 s for pedals, 5 s for buttons) and are skipped.
//! Device 0 is the wheel; the others are candidates for separate pedals
//! or a button box.

use super::{DeviceState, PedalAxis};
use crate::controls::Button;

pub const ARM_DELTA: i32 = 16_000;
pub const RELEASE_DELTA: i32 = 8_000;
pub const AXIS_TIMEOUT_MS: u64 = 10_000;
pub const BUTTON_TIMEOUT_MS: u64 = 5_000;
/// Readings to wait before taking a step's baseline.
pub const WARM_UP_TICKS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    Accelerator,
    Brake,
    Button(Button),
}

impl Step {
    pub fn prompt(self) -> String {
        match self {
            Step::Accelerator => "Press the ACCELERATOR all the way, then let go".into(),
            Step::Brake => "Press the BRAKE all the way, then let go".into(),
            Step::Button(b) => format!("Press the button for {}", b.label().to_uppercase()),
        }
    }

    fn timeout_ms(self) -> u64 {
        match self {
            Step::Accelerator | Step::Brake => AXIS_TIMEOUT_MS,
            Step::Button(_) => BUTTON_TIMEOUT_MS,
        }
    }
}

pub const STEPS: [Step; 6] = [
    Step::Accelerator,
    Step::Brake,
    Step::Button(Button::Light),
    Step::Button(Button::Wiper),
    Step::Button(Button::Start),
    Step::Button(Button::Exit),
];

/// What the wizard learned. Device indices refer to the snapshot order.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Captures {
    pub gas: Option<(usize, PedalAxis)>,
    pub brake: Option<(usize, PedalAxis)>,
    pub buttons: Vec<(Button, usize, usize)>,
    pub skipped: Vec<Step>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Armed {
    device: usize,
    axis: usize,
    rest: i16,
    peak: i16,
}

#[derive(Debug, Clone)]
pub struct Wizard {
    step: usize,
    step_started_ms: Option<u64>,
    ticks: u32,
    baseline: Vec<DeviceState>,
    armed: Option<Armed>,
    /// The wheel's steering axis is never taken for a pedal.
    steer_axis: usize,
    result: Captures,
}

/// What to show after a tick.
#[derive(Debug, Clone, PartialEq)]
pub enum Status {
    WarmingUp,
    Waiting {
        step: Step,
        index: usize,
        remaining_ms: u64,
    },
    /// A pedal is being pressed (0..1 of its travel so far).
    Pressing {
        step: Step,
        index: usize,
        travel: f32,
    },
    Done(Captures),
}

impl Wizard {
    pub fn new(steer_axis: usize) -> Self {
        Self {
            step: 0,
            step_started_ms: None,
            ticks: 0,
            baseline: Vec::new(),
            armed: None,
            steer_axis,
            result: Captures::default(),
        }
    }

    pub fn current(&self) -> Option<Step> {
        STEPS.get(self.step).copied()
    }

    /// Feed the readings of every device; `now_ms` is a monotonic clock.
    pub fn tick(&mut self, now_ms: u64, devices: &[DeviceState]) -> Status {
        let Some(step) = self.current() else {
            return Status::Done(self.result.clone());
        };
        if self.step_started_ms.is_none() {
            self.ticks += 1;
            if self.ticks < WARM_UP_TICKS {
                return Status::WarmingUp;
            }
            self.baseline = devices.to_vec();
            self.step_started_ms = Some(now_ms);
            self.armed = None;
        }
        let started = self.step_started_ms.expect("set above");
        let elapsed = now_ms.saturating_sub(started);

        let captured = match step {
            Step::Accelerator | Step::Brake => self.track_axis(step, devices),
            Step::Button(b) => self.find_button(devices).map(|(dev, btn)| {
                self.result.buttons.push((b, dev, btn));
            }),
        };
        if captured.is_some() {
            return self.advance(now_ms, devices);
        }
        if let Some(a) = self.armed {
            let span = (i32::from(a.peak) - i32::from(a.rest)).abs() as f32;
            return Status::Pressing {
                step,
                index: self.step,
                travel: (span / 65_535.0).min(1.0),
            };
        }
        if elapsed >= step.timeout_ms() {
            self.result.skipped.push(step);
            return self.advance(now_ms, devices);
        }
        Status::Waiting {
            step,
            index: self.step,
            remaining_ms: step.timeout_ms() - elapsed,
        }
    }

    fn advance(&mut self, now_ms: u64, devices: &[DeviceState]) -> Status {
        self.step += 1;
        self.step_started_ms = None;
        self.ticks = WARM_UP_TICKS; // devices are already warm
        match self.current() {
            Some(_) => self.tick(now_ms, devices),
            None => Status::Done(self.result.clone()),
        }
    }

    fn base_axis(&self, device: usize, axis: usize) -> Option<i16> {
        self.baseline.get(device).and_then(|d| d.axis(axis))
    }

    /// Arm on a big move from the baseline, follow it to its peak, and
    /// capture when it comes back. The brake may share the accelerator's
    /// axis only in the opposite direction (a combined pedal axis).
    fn track_axis(&mut self, step: Step, devices: &[DeviceState]) -> Option<()> {
        if let Some(mut a) = self.armed {
            let cur = devices.get(a.device)?.axis(a.axis)?;
            if (i32::from(cur) - i32::from(a.rest)).abs()
                > (i32::from(a.peak) - i32::from(a.rest)).abs()
            {
                a.peak = cur;
            }
            self.armed = Some(a);
            if (i32::from(cur) - i32::from(a.rest)).abs() < RELEASE_DELTA {
                let pedal = (
                    a.device,
                    PedalAxis {
                        axis: a.axis,
                        rest: a.rest,
                        peak: a.peak,
                    },
                );
                match step {
                    Step::Accelerator => self.result.gas = Some(pedal),
                    _ => self.result.brake = Some(pedal),
                }
                return Some(());
            }
            return None;
        }
        for (d, dev) in devices.iter().enumerate() {
            for (axis, &cur) in dev.axes.iter().enumerate() {
                if d == 0 && axis == self.steer_axis {
                    continue;
                }
                let Some(rest) = self.base_axis(d, axis) else {
                    continue;
                };
                let delta = i32::from(cur) - i32::from(rest);
                if delta.abs() <= ARM_DELTA {
                    continue;
                }
                if step == Step::Brake
                    && let Some((gd, g)) = self.result.gas
                    && gd == d
                    && g.axis == axis
                    && (i32::from(g.peak) - i32::from(g.rest)).signum() == delta.signum()
                {
                    continue; // that is the accelerator again
                }
                self.armed = Some(Armed {
                    device: d,
                    axis,
                    rest,
                    peak: cur,
                });
                return None;
            }
        }
        None
    }

    fn find_button(&self, devices: &[DeviceState]) -> Option<(usize, usize)> {
        for (d, dev) in devices.iter().enumerate() {
            for (b, &down) in dev.buttons.iter().enumerate() {
                let was = self.baseline.get(d).is_some_and(|base| base.button(b));
                if down && !was {
                    return Some((d, b));
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rig {
        wiz: Wizard,
        now: u64,
        devs: Vec<DeviceState>,
    }

    impl Rig {
        fn new(devs: Vec<DeviceState>) -> Self {
            let mut rig = Rig {
                wiz: Wizard::new(0),
                now: 0,
                devs,
            };
            for _ in 0..WARM_UP_TICKS {
                rig.tick();
            }
            rig
        }
        fn tick(&mut self) -> Status {
            self.now += 4;
            self.wiz.tick(self.now, &self.devs)
        }
        fn sweep(&mut self, d: usize, axis: usize, to: i16) -> Status {
            let from = self.devs[d].axes[axis];
            for v in [to / 2, to, to / 2, from] {
                self.devs[d].axes[axis] = v;
                self.tick();
            }
            self.devs[d].axes[axis] = from;
            self.tick()
        }
        fn press(&mut self, d: usize, b: usize) -> Status {
            self.devs[d].buttons[b] = true;
            let s = self.tick();
            self.devs[d].buttons[b] = false;
            self.tick();
            s
        }
    }

    fn wheel() -> DeviceState {
        DeviceState {
            axes: vec![0, 32_767, 32_767],
            buttons: vec![false; 12],
        }
    }

    #[test]
    fn captures_pedals_and_buttons_on_the_wheel() {
        let mut rig = Rig::new(vec![wheel()]);
        assert!(matches!(
            rig.tick(),
            Status::Waiting {
                step: Step::Accelerator,
                ..
            }
        ));

        // turning the wheel is not a pedal
        rig.devs[0].axes[0] = 30_000;
        rig.tick();
        rig.devs[0].axes[0] = 0;
        assert!(matches!(
            rig.tick(),
            Status::Waiting {
                step: Step::Accelerator,
                ..
            }
        ));

        rig.devs[0].axes[1] = -30_000;
        assert!(matches!(
            rig.tick(),
            Status::Pressing {
                step: Step::Accelerator,
                ..
            }
        ));
        rig.devs[0].axes[1] = -32_768;
        rig.tick();
        rig.devs[0].axes[1] = 32_767;
        assert!(matches!(
            rig.tick(),
            Status::Waiting {
                step: Step::Brake,
                ..
            }
        ));

        rig.sweep(0, 2, -32_768);
        rig.press(0, 4);
        rig.press(0, 5);
        rig.press(0, 9);
        let Status::Done(result) = rig.press(0, 8) else {
            panic!("not done")
        };
        assert_eq!(
            result.gas,
            Some((
                0,
                PedalAxis {
                    axis: 1,
                    rest: 32_767,
                    peak: -32_768
                }
            ))
        );
        assert_eq!(
            result.brake,
            Some((
                0,
                PedalAxis {
                    axis: 2,
                    rest: 32_767,
                    peak: -32_768
                }
            ))
        );
        assert_eq!(
            result.buttons,
            vec![
                (Button::Light, 0, 4),
                (Button::Wiper, 0, 5),
                (Button::Start, 0, 9),
                (Button::Exit, 0, 8)
            ]
        );
        assert!(result.skipped.is_empty());
    }

    #[test]
    fn combined_axis_and_separate_pedal_device() {
        let pedals = DeviceState {
            axes: vec![0],
            buttons: vec![],
        };
        let mut rig = Rig::new(vec![
            DeviceState {
                axes: vec![0],
                buttons: vec![false; 4],
            },
            pedals,
        ]);
        rig.tick();
        rig.sweep(1, 0, 32_767);
        // pressing the accelerator again during the brake step is ignored
        rig.sweep(1, 0, 32_767);
        assert!(matches!(
            rig.tick(),
            Status::Waiting {
                step: Step::Brake,
                ..
            }
        ));
        rig.sweep(1, 0, -32_768);
        assert_eq!(
            rig.wiz.result.gas,
            Some((
                1,
                PedalAxis {
                    axis: 0,
                    rest: 0,
                    peak: 32_767
                }
            ))
        );
        assert_eq!(
            rig.wiz.result.brake,
            Some((
                1,
                PedalAxis {
                    axis: 0,
                    rest: 0,
                    peak: -32_768
                }
            ))
        );
    }

    #[test]
    fn steps_time_out_and_held_buttons_are_ignored() {
        let mut held = wheel();
        held.buttons[3] = true;
        let mut rig = Rig::new(vec![held]);
        rig.now += AXIS_TIMEOUT_MS;
        rig.tick();
        rig.now += AXIS_TIMEOUT_MS;
        assert!(matches!(
            rig.tick(),
            Status::Waiting {
                step: Step::Button(Button::Light),
                ..
            }
        ));
        // button 3 was already down when the step began
        assert!(matches!(
            rig.tick(),
            Status::Waiting {
                step: Step::Button(Button::Light),
                ..
            }
        ));
        for _ in 0..4 {
            rig.now += BUTTON_TIMEOUT_MS;
            rig.tick();
        }
        let Status::Done(r) = rig.tick() else {
            panic!()
        };
        assert_eq!(r.skipped.len(), STEPS.len());
        assert_eq!(r.gas, None);
    }
}
