//! Turning raw wheel, pedal and gamepad readings into cabinet controls,
//! the way Wanszai's wrappers do (docs/WANSZAI_WHEELS.md §2–3).

pub mod known;
pub mod wizard;

use crate::controls::{Action, Button, Cabinet};
use crate::ini::Ini;
use crate::pad_map::PadMap;

/// Raw axis full scale, as SDL reports it.
pub const RAW_MAX: f32 = 32767.0;

/// A device's raw state: axes in −32768..=32767, buttons pressed or not.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DeviceState {
    pub axes: Vec<i16>,
    pub buttons: Vec<bool>,
}

impl DeviceState {
    pub fn axis(&self, i: usize) -> Option<i16> {
        self.axes.get(i).copied()
    }

    pub fn button(&self, i: usize) -> bool {
        self.buttons.get(i).copied().unwrap_or(false)
    }
}

/// Steering lock, in degrees of wheel rotation lock to lock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SteeringLock {
    Degrees(u16),
    /// The wheel's whole rotation.
    Full,
}

impl SteeringLock {
    /// Wanszai's steps, in the order the menu cycles them.
    pub const STEPS: [SteeringLock; 6] = [
        SteeringLock::Degrees(180),
        SteeringLock::Degrees(270),
        SteeringLock::Degrees(360),
        SteeringLock::Degrees(540),
        SteeringLock::Degrees(900),
        SteeringLock::Full,
    ];

    /// 270°, the arcade cabinet's feel.
    pub const DEFAULT: SteeringLock = SteeringLock::Degrees(270);

    pub fn label(self) -> String {
        match self {
            SteeringLock::Degrees(d) => format!("{d}°"),
            SteeringLock::Full => "Full".into(),
        }
    }

    /// Stored as degrees, 0 for Full; unknown values snap to the nearest step.
    pub fn from_setting(v: i64) -> Self {
        if v <= 0 {
            return SteeringLock::Full;
        }
        *Self::STEPS
            .iter()
            .filter_map(|s| match s {
                SteeringLock::Degrees(d) => Some((s, (i64::from(*d) - v).abs())),
                SteeringLock::Full => None,
            })
            .min_by_key(|(_, dist)| *dist)
            .map(|(s, _)| s)
            .unwrap()
    }

    pub fn to_setting(self) -> i64 {
        match self {
            SteeringLock::Degrees(d) => i64::from(d),
            SteeringLock::Full => 0,
        }
    }

    pub fn step(self, delta: i32) -> Self {
        let i = Self::STEPS.iter().position(|s| *s == self).unwrap_or(1) as i32;
        Self::STEPS[(i + delta).rem_euclid(Self::STEPS.len() as i32) as usize]
    }
}

/// Wheel position (−1..1 over the wheel's full rotation) to cabinet
/// steering (−1..1 over the steering lock).
pub fn apply_lock(raw: f32, rotation_deg: u16, lock: SteeringLock) -> f32 {
    let raw = raw.clamp(-1.0, 1.0);
    match lock {
        SteeringLock::Full => raw,
        SteeringLock::Degrees(lock) => {
            let half_rotation = if rotation_deg <= 1 {
                450.0
            } else {
                f32::from(rotation_deg) / 2.0
            };
            (raw * half_rotation / (f32::from(lock) / 2.0)).clamp(-1.0, 1.0)
        }
    }
}

/// Linearity 0..=100: 0 is linear, 100 is a full cubic curve.
pub fn curve(m: f32, linearity: u8) -> f32 {
    let l = f32::from(linearity.min(100)) / 100.0;
    l * m * m * m + (1.0 - l) * m
}

/// Deadzone (a fraction of full scale) then curve, keeping the sign.
pub fn shape(v: f32, deadzone: f32, linearity: u8) -> f32 {
    let v = v.clamp(-1.0, 1.0);
    let dz = deadzone.clamp(0.0, 0.99);
    let m = if v.abs() <= dz {
        0.0
    } else {
        ((v.abs() - dz) / (1.0 - dz)).min(1.0)
    };
    curve(m, linearity).copysign(v)
}

/// The menu name of a linearity value.
pub fn linearity_label(l: u8) -> &'static str {
    match l {
        0 => "Linear",
        1..=20 => "Slight",
        21..=40 => "Mild",
        41..=60 => "Medium",
        61..=80 => "Heavy",
        _ => "Cubic",
    }
}

/// How one pedal is read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PedalAxis {
    pub axis: usize,
    /// Raw value with the pedal up.
    pub rest: i16,
    /// Raw value with the pedal floored.
    pub peak: i16,
}

/// Wheel pedal deadzone at the top of the travel (Wanszai's 2 %).
pub const PEDAL_DEADZONE: f32 = 0.02;

impl PedalAxis {
    /// 0 up … 1 floored.
    pub fn read(&self, raw: i16) -> f32 {
        let span = f32::from(self.peak) - f32::from(self.rest);
        if span.abs() < 1000.0 {
            return 0.0;
        }
        let p = ((f32::from(raw) - f32::from(self.rest)) / span).clamp(0.0, 1.0);
        ((p - PEDAL_DEADZONE) / (1.0 - PEDAL_DEADZONE)).clamp(0.0, 1.0)
    }

    /// The same axis with its direction flipped (the menu's invert toggle).
    pub fn inverted(self) -> Self {
        Self {
            rest: self.peak,
            peak: self.rest,
            ..self
        }
    }
}

/// Where a pedal or button lives.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    Wheel,
    /// A second device (separate pedals or a button box).
    Aux,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binding {
    pub source: Source,
    pub button: usize,
}

impl Binding {
    fn parse(s: &str) -> Option<Self> {
        let (src, n) = s.split_once(':')?;
        let source = match src {
            "wheel" => Source::Wheel,
            "aux" => Source::Aux,
            _ => return None,
        };
        Some(Binding {
            source,
            button: n.parse().ok()?,
        })
    }

    fn format(self) -> String {
        let src = match self.source {
            Source::Wheel => "wheel",
            Source::Aux => "aux",
        };
        format!("{src}:{}", self.button)
    }
}

/// A device's identity, for pinning it across runs.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeviceId {
    pub name: String,
    pub vid: u16,
    pub pid: u16,
    /// SDL's GUID string (stable for a model on a platform).
    pub guid: String,
}

/// A connected wheel, as [`pick_wheel`] weighs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WheelCandidate<'a> {
    pub vid: u16,
    pub pid: u16,
    pub path: &'a str,
    pub haptic: bool,
}

/// Which wheel to drive the car with, as an index into `wheels`:
///
/// 1. the one picked by hand (`chosen`, a device path), when connected;
/// 2. otherwise among the saved profile's model (or every wheel), the
///    first that takes force feedback, then the first at all.
///
/// Some wheels show up twice, once per HID collection, and only the one
/// that takes force feedback reports input (Fanatec CSL DD on Windows).
pub fn pick_wheel(
    wheels: &[WheelCandidate],
    saved: Option<&DeviceId>,
    chosen: &str,
) -> Option<usize> {
    if !chosen.is_empty()
        && let Some(i) = wheels.iter().position(|w| w.path == chosen)
    {
        return Some(i);
    }
    let same_model = |w: &WheelCandidate| saved.is_some_and(|s| (s.vid, s.pid) == (w.vid, w.pid));
    let pool: Vec<usize> = if wheels.iter().any(same_model) {
        (0..wheels.len())
            .filter(|&i| same_model(&wheels[i]))
            .collect()
    } else {
        (0..wheels.len()).collect()
    };
    pool.iter()
        .copied()
        .find(|&i| wheels[i].haptic)
        .or(pool.first().copied())
}

/// Everything the setup wizard learns about a wheel; saved as `wheel.ini`.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelProfile {
    pub wheel: DeviceId,
    /// Pedals or buttons on a second USB device.
    pub aux: Option<DeviceId>,
    pub steer_axis: usize,
    /// Lock to lock, degrees.
    pub rotation: u16,
    pub gas: Option<(Source, PedalAxis)>,
    pub brake: Option<(Source, PedalAxis)>,
    pub buttons: Vec<(Button, Binding)>,
}

impl WheelProfile {
    /// A first guess for a wheel nobody has set up: steering on the first
    /// axis, pedals from their resting position, paddles from the table
    /// (buttons 0 and 1 for a wheel it does not list, as Wanszai's generic
    /// row) as lights and wiper.
    pub fn guess(wheel: DeviceId, initial: &DeviceState) -> Self {
        let known = known::lookup(wheel.vid, wheel.pid);
        let rotation = known.map_or(known::GENERIC_ROTATION, |k| k.rotation);
        let (gas, brake) = guess_pedals(initial);
        let mut buttons = Vec::new();
        let paddles = known.map_or(Some(known::GENERIC_PADDLES), |k| k.paddles);
        if let Some((up, down)) = paddles {
            buttons.push((
                Button::Light,
                Binding {
                    source: Source::Wheel,
                    button: usize::from(up),
                },
            ));
            buttons.push((
                Button::Wiper,
                Binding {
                    source: Source::Wheel,
                    button: usize::from(down),
                },
            ));
        }
        Self {
            wheel,
            aux: None,
            steer_axis: 0,
            rotation,
            gas: gas.map(|p| (Source::Wheel, p)),
            brake: brake.map(|p| (Source::Wheel, p)),
            buttons,
        }
    }

    pub fn binding(&self, button: Button) -> Option<Binding> {
        self.buttons
            .iter()
            .find(|(b, _)| *b == button)
            .map(|(_, bind)| *bind)
    }

    /// Bind `button` to `binding` alone, taking that binding away from any
    /// other button so one press never does two things.
    pub fn assign(&mut self, button: Button, binding: Binding) {
        self.buttons
            .retain(|(b, bind)| *b != button && *bind != binding);
        self.buttons.push((button, binding));
        self.buttons.sort_by_key(|(b, _)| *b);
    }

    /// Drop the second device and everything bound to it.
    pub fn forget_aux(&mut self) {
        self.aux = None;
        self.buttons
            .retain(|(_, bind)| bind.source == Source::Wheel);
        for pedal in [&mut self.gas, &mut self.brake] {
            if pedal.is_some_and(|(src, _)| src == Source::Aux) {
                *pedal = None;
            }
        }
    }

    /// Take what the setup wizard captured. `devices[0]` is the wheel, the
    /// others are the devices it also watched (wizard device order). The
    /// first other device used becomes the aux device; whatever was bound to
    /// a different aux device before is dropped.
    pub fn apply(&mut self, captures: &wizard::Captures, devices: &[DeviceId]) {
        let other = captures
            .gas
            .map(|(d, _)| d)
            .into_iter()
            .chain(captures.brake.map(|(d, _)| d))
            .chain(captures.buttons.iter().map(|(_, d, _)| *d))
            .find(|d| *d != 0 && *d < devices.len());
        if let Some(d) = other
            && self.aux.as_ref() != Some(&devices[d])
        {
            self.forget_aux();
            self.aux = Some(devices[d].clone());
        }
        // the wheel, the new aux device, or (any third device) nothing
        let source = |d: usize| match d {
            0 => Some(Source::Wheel),
            _ if Some(d) == other => Some(Source::Aux),
            _ => None,
        };
        if let Some((src, p)) = captures.gas.and_then(|(d, p)| Some((source(d)?, p))) {
            self.gas = Some((src, p));
            if same_pedal(self.brake, (src, p)) {
                self.brake = None;
            }
        }
        if let Some((src, p)) = captures.brake.and_then(|(d, p)| Some((source(d)?, p))) {
            self.brake = Some((src, p));
            if same_pedal(self.gas, (src, p)) {
                self.gas = None;
            }
        }
        for &(button, d, n) in &captures.buttons {
            if let Some(src) = source(d) {
                self.assign(
                    button,
                    Binding {
                        source: src,
                        button: n,
                    },
                );
            }
        }
    }

    /// What the menu shows for a control: "AXIS 2", "AUX BUTTON 5" (counted
    /// from 1, as wheel software shows them), or "-".
    pub fn describe(&self, action: Action) -> String {
        let prefix = |src: Source| if src == Source::Aux { "AUX " } else { "" };
        let pedal = |p: &Option<(Source, PedalAxis)>| {
            p.map_or("-".to_string(), |(src, axis)| {
                format!("{}AXIS {}", prefix(src), axis.axis + 1)
            })
        };
        match action {
            Action::Accelerate => pedal(&self.gas),
            Action::Brake => pedal(&self.brake),
            Action::Press(button) => self.binding(button).map_or("-".to_string(), |b| {
                format!("{}BUTTON {}", prefix(b.source), b.button + 1)
            }),
        }
    }

    /// The cabinet controls for the current readings.
    pub fn read(
        &self,
        settings: &WheelSettings,
        wheel: &DeviceState,
        aux: Option<&DeviceState>,
    ) -> Cabinet {
        let device = |src: Source| match src {
            Source::Wheel => Some(wheel),
            Source::Aux => aux,
        };
        let pedal = |p: &Option<(Source, PedalAxis)>, invert: bool| {
            p.and_then(|(src, axis)| {
                let axis = if invert { axis.inverted() } else { axis };
                device(src)
                    .and_then(|d| d.axis(axis.axis))
                    .map(|raw| axis.read(raw))
            })
            .unwrap_or(0.0)
        };
        let raw_steer = wheel
            .axis(self.steer_axis)
            .map_or(0.0, |v| f32::from(v) / RAW_MAX);
        let mut cab = Cabinet {
            steer: apply_lock(raw_steer, self.rotation, settings.lock),
            accel: pedal(&self.gas, settings.invert_accel),
            brake: pedal(&self.brake, settings.invert_brake),
            buttons: 0,
        };
        for (button, bind) in &self.buttons {
            if device(bind.source).is_some_and(|d| d.button(bind.button)) {
                cab.press(*button);
            }
        }
        cab
    }

    pub fn to_ini(&self) -> Ini {
        let mut ini = Ini::default();
        ini.comment("wheel.ini - written by the Rad Mobile launcher's wheel setup");
        ini.comment("hand edits stay until the setup runs again");
        let put_id = |ini: &mut Ini, prefix: &str, id: &DeviceId| {
            ini.set(&format!("{prefix}name"), &id.name);
            ini.set(&format!("{prefix}vid"), &format!("{:04x}", id.vid));
            ini.set(&format!("{prefix}pid"), &format!("{:04x}", id.pid));
            ini.set(&format!("{prefix}guid"), &id.guid);
        };
        put_id(&mut ini, "wheel_", &self.wheel);
        if let Some(aux) = &self.aux {
            put_id(&mut ini, "aux_", aux);
        }
        ini.set("steer_axis", &self.steer_axis.to_string());
        ini.set("rotation", &self.rotation.to_string());
        for (key, pedal) in [("gas", &self.gas), ("brake", &self.brake)] {
            if let Some((src, p)) = pedal {
                let src = if *src == Source::Aux { "aux" } else { "wheel" };
                ini.set(&format!("{key}_device"), src);
                ini.set(&format!("{key}_axis"), &p.axis.to_string());
                ini.set(&format!("{key}_rest"), &p.rest.to_string());
                ini.set(&format!("{key}_peak"), &p.peak.to_string());
            }
        }
        for (button, bind) in &self.buttons {
            ini.set(&format!("btn_{}", button.key()), &bind.format());
        }
        ini
    }

    pub fn from_ini(ini: &Ini) -> Option<Self> {
        let get_id = |prefix: &str| -> Option<DeviceId> {
            Some(DeviceId {
                name: ini
                    .get(&format!("{prefix}name"))
                    .unwrap_or_default()
                    .to_string(),
                vid: u16::from_str_radix(ini.get(&format!("{prefix}vid"))?, 16).ok()?,
                pid: u16::from_str_radix(ini.get(&format!("{prefix}pid"))?, 16).ok()?,
                guid: ini
                    .get(&format!("{prefix}guid"))
                    .unwrap_or_default()
                    .to_string(),
            })
        };
        let pedal = |key: &str| -> Option<(Source, PedalAxis)> {
            let src = match ini.get(&format!("{key}_device")).unwrap_or("wheel") {
                "aux" => Source::Aux,
                _ => Source::Wheel,
            };
            Some((
                src,
                PedalAxis {
                    axis: ini.get_parsed(&format!("{key}_axis"))?,
                    rest: ini.get_parsed(&format!("{key}_rest"))?,
                    peak: ini.get_parsed(&format!("{key}_peak"))?,
                },
            ))
        };
        let mut buttons = Vec::new();
        for button in Button::ALL {
            if let Some(b) = ini
                .get(&format!("btn_{}", button.key()))
                .and_then(Binding::parse)
            {
                buttons.push((button, b));
            }
        }
        Some(Self {
            wheel: get_id("wheel_")?,
            aux: get_id("aux_"),
            steer_axis: ini.get_parsed("steer_axis").unwrap_or(0),
            rotation: ini
                .get_parsed("rotation")
                .unwrap_or(known::GENERIC_ROTATION)
                .clamp(90, 3600),
            gas: pedal("gas"),
            brake: pedal("brake"),
            buttons,
        })
    }
}

/// Whether `pedal` reads the same axis the same way as `other`. Two pedals
/// on one axis in opposite directions are a combined pedal axis and fine.
fn same_pedal(pedal: Option<(Source, PedalAxis)>, other: (Source, PedalAxis)) -> bool {
    let direction = |p: PedalAxis| (i32::from(p.peak) - i32::from(p.rest)).signum();
    pedal.is_some_and(|(src, p)| {
        src == other.0 && p.axis == other.1.axis && direction(p) == direction(other.1)
    })
}

/// Pedals rest at one end of their axis while the steering rests near the
/// middle, so the first two axes (after the steering) resting beyond
/// ±24000 are taken as accelerator and brake.
pub fn guess_pedals(initial: &DeviceState) -> (Option<PedalAxis>, Option<PedalAxis>) {
    let mut resting = initial
        .axes
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(_, v)| v.unsigned_abs() > 24_000)
        .map(|(axis, &v)| PedalAxis {
            axis,
            rest: if v > 0 { 32_767 } else { -32_768 },
            peak: if v > 0 { -32_768 } else { 32_767 },
        });
    (resting.next(), resting.next())
}

/// The wheel options from the menu.
#[derive(Debug, Clone, PartialEq)]
pub struct WheelSettings {
    pub lock: SteeringLock,
    pub invert_accel: bool,
    pub invert_brake: bool,
    /// The wheel picked by hand, by device path; empty picks automatically.
    pub device: String,
}

impl Default for WheelSettings {
    fn default() -> Self {
        Self {
            lock: SteeringLock::DEFAULT,
            invert_accel: false,
            invert_brake: false,
            device: String::new(),
        }
    }
}

/// A gamepad as SDL's controller layer sees it.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct PadState {
    pub left_x: i16,
    /// 0..=32767
    pub left_trigger: i16,
    pub right_trigger: i16,
    pub a: bool,
    pub b: bool,
    pub x: bool,
    pub y: bool,
    pub start: bool,
    pub back: bool,
    pub guide: bool,
    pub left_shoulder: bool,
    pub right_shoulder: bool,
    pub left_stick: bool,
    pub right_stick: bool,
    pub dpad_left: bool,
    pub dpad_right: bool,
    /// For menu navigation only.
    pub dpad_up: bool,
    pub dpad_down: bool,
    pub left_y: i16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadSettings {
    /// Of 32767.
    pub stick_deadzone: u16,
    pub stick_linearity: u8,
    /// Of 255.
    pub trigger_deadzone: u8,
    pub trigger_linearity: u8,
    /// The stick asks for an angle to the road instead of moving the
    /// cabinet wheel directly ([`crate::assist`]).
    pub steer_assist: bool,
    /// Which pad buttons and triggers drive the other controls.
    pub map: PadMap,
}

impl Default for PadSettings {
    fn default() -> Self {
        Self {
            stick_deadzone: 3000,
            stick_linearity: 0,
            trigger_deadzone: 12,
            trigger_linearity: 0,
            steer_assist: true,
            map: PadMap::default(),
        }
    }
}

/// A gamepad's cabinet controls. The left stick (or the d-pad) steers;
/// everything else follows [`PadSettings::map`], by default right trigger
/// or A accelerates, left trigger or B brakes, X or LB lights, Y or RB
/// wiper, Start starts, Back inserts a coin and Guide returns to the menu.
/// Back + Start together always returns to the menu, and while the menu
/// is asked for, Start and Coin are not pressed.
pub fn read_pad(settings: &PadSettings, pad: &PadState) -> Cabinet {
    let stick = shape(
        f32::from(pad.left_x) / RAW_MAX,
        f32::from(settings.stick_deadzone) / RAW_MAX,
        settings.stick_linearity,
    );
    let dpad = match (pad.dpad_left, pad.dpad_right) {
        (true, false) => -1.0,
        (false, true) => 1.0,
        _ => 0.0,
    };
    // a trigger is a pedal through its deadzone and curve; a button floors it
    let pedal = |action: Action| {
        settings
            .map
            .inputs(action)
            .map(|input| match input.trigger(pad) {
                Some(pull) => shape(
                    pull,
                    f32::from(settings.trigger_deadzone) / 255.0,
                    settings.trigger_linearity,
                ),
                None if input.is_down(pad) => 1.0,
                None => 0.0,
            })
            .fold(0.0, f32::max)
    };
    let mut cab = Cabinet {
        steer: if dpad != 0.0 { dpad } else { stick },
        accel: pedal(Action::Accelerate),
        brake: pedal(Action::Brake),
        buttons: 0,
    };
    let exit = (pad.back && pad.start) || settings.map.is_down(Action::Press(Button::Exit), pad);
    for button in Button::ALL {
        let held = settings.map.is_down(Action::Press(button), pad);
        let pressed = match button {
            Button::Exit => exit,
            Button::Start | Button::Coin => held && !exit,
            _ => held,
        };
        if pressed {
            cab.press(button);
        }
    }
    cab
}

#[cfg(test)]
mod tests {
    fn cand(pid: u16, path: &str, haptic: bool) -> WheelCandidate<'_> {
        WheelCandidate {
            vid: 0x0eb7,
            pid,
            path,
            haptic,
        }
    }

    #[test]
    fn the_collection_that_takes_force_feedback_wins() {
        // the inert collection comes first, as on the reporter's Windows
        let wheels = [cand(0x20, "col02", false), cand(0x20, "col01", true)];
        assert_eq!(pick_wheel(&wheels, None, ""), Some(1));
        let only_inert = [cand(0x20, "col02", false)];
        assert_eq!(pick_wheel(&only_inert, None, ""), Some(0));
        assert_eq!(pick_wheel(&[], None, ""), None);
    }

    #[test]
    fn a_wheel_picked_by_hand_wins_while_connected() {
        let wheels = [cand(0x20, "a", true), cand(0x20, "b", false)];
        assert_eq!(pick_wheel(&wheels, None, "b"), Some(1));
        assert_eq!(pick_wheel(&wheels, None, "gone"), Some(0));
    }

    #[test]
    fn the_saved_model_comes_before_force_feedback_elsewhere() {
        let saved = DeviceId {
            vid: 0x0eb7,
            pid: 0x20,
            ..DeviceId::default()
        };
        let wheels = [cand(0x05, "other", true), cand(0x20, "mine", false)];
        assert_eq!(pick_wheel(&wheels, Some(&saved), ""), Some(1));
    }

    use super::*;
    use crate::pad_map::PadInput;

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 1e-3
    }

    #[test]
    fn steering_lock_scales_the_wheel_rotation() {
        // a 1080° wheel at 270° lock: a quarter of the half-rotation is full lock
        assert!(approx(
            apply_lock(0.25, 1080, SteeringLock::Degrees(270)),
            1.0
        ));
        assert!(approx(
            apply_lock(0.125, 1080, SteeringLock::Degrees(270)),
            0.5
        ));
        assert!(approx(
            apply_lock(-0.9, 1080, SteeringLock::Degrees(270)),
            -1.0
        ));
        assert!(approx(apply_lock(0.4, 900, SteeringLock::Full), 0.4));
        // a nonsense rotation falls back to 900°
        assert!(approx(apply_lock(0.1, 0, SteeringLock::Degrees(180)), 0.5));
    }

    #[test]
    fn lock_steps_cycle_and_snap() {
        assert_eq!(SteeringLock::DEFAULT.step(1), SteeringLock::Degrees(360));
        assert_eq!(SteeringLock::Full.step(1), SteeringLock::Degrees(180));
        assert_eq!(SteeringLock::Degrees(180).step(-1), SteeringLock::Full);
        assert_eq!(SteeringLock::from_setting(300), SteeringLock::Degrees(270));
        assert_eq!(SteeringLock::from_setting(0), SteeringLock::Full);
        assert_eq!(
            SteeringLock::from_setting(SteeringLock::Degrees(540).to_setting()),
            SteeringLock::Degrees(540)
        );
    }

    #[test]
    fn curves_follow_the_cubic_blend() {
        assert!(approx(curve(0.5, 0), 0.5));
        assert!(approx(curve(0.5, 100), 0.125));
        assert!(approx(curve(0.5, 50), 0.3125));
        assert!(approx(shape(0.05, 0.1, 0), 0.0));
        assert!(approx(shape(-0.55, 0.1, 0), -0.5));
        assert!(approx(shape(1.0, 0.1, 100), 1.0));
        assert_eq!(linearity_label(0), "Linear");
        assert_eq!(linearity_label(30), "Mild");
        assert_eq!(linearity_label(100), "Cubic");
    }

    #[test]
    fn pedals_read_with_calibration_and_deadzone() {
        let gas = PedalAxis {
            axis: 1,
            rest: 32_767,
            peak: -32_768,
        };
        assert_eq!(gas.read(32_767), 0.0);
        assert!(approx(gas.read(-32_768), 1.0));
        assert!(approx(
            gas.read(0),
            (0.5 - PEDAL_DEADZONE) / (1.0 - PEDAL_DEADZONE)
        ));
        assert_eq!(gas.read(32_000), 0.0, "inside the top deadzone");
        assert!(approx(gas.inverted().read(32_767), 1.0));
        // an uncalibrated axis reads as released
        assert_eq!(
            PedalAxis {
                axis: 1,
                rest: 0,
                peak: 10
            }
            .read(10),
            0.0
        );
    }

    #[test]
    fn pedals_are_guessed_from_where_they_rest() {
        let initial = DeviceState {
            axes: vec![-120, 32_767, 5, -32_768, 32_767],
            buttons: vec![],
        };
        let (gas, brake) = guess_pedals(&initial);
        assert_eq!(
            gas,
            Some(PedalAxis {
                axis: 1,
                rest: 32_767,
                peak: -32_768
            })
        );
        assert_eq!(
            brake,
            Some(PedalAxis {
                axis: 3,
                rest: -32_768,
                peak: 32_767
            })
        );
        let none = guess_pedals(&DeviceState {
            axes: vec![0, 0],
            buttons: vec![],
        });
        assert_eq!(none, (None, None));
    }

    fn csl() -> DeviceId {
        DeviceId {
            name: "FANATEC CSL DD".into(),
            vid: 0x0eb7,
            pid: 0x0020,
            guid: "030000b70e00002000".into(),
        }
    }

    #[test]
    fn a_guessed_profile_drives_the_cabinet() {
        let rest = DeviceState {
            axes: vec![0, 32_767, 32_767],
            buttons: vec![false; 16],
        };
        let profile = WheelProfile::guess(csl(), &rest);
        assert_eq!(profile.rotation, 1080);
        assert_eq!(
            profile.binding(Button::Light),
            Some(Binding {
                source: Source::Wheel,
                button: 0
            })
        );

        let mut now = rest.clone();
        now.axes[0] = 8_192; // a quarter of the way to the right of a 1080° wheel = 135°
        now.axes[1] = -32_768;
        now.buttons[1] = true;
        let cab = profile.read(&WheelSettings::default(), &now, None);
        assert!(approx(cab.steer, 1.0));
        assert!(approx(cab.accel, 1.0));
        assert_eq!(cab.brake, 0.0);
        assert!(cab.is_pressed(Button::Wiper));

        let inverted = WheelSettings {
            invert_brake: true,
            ..Default::default()
        };
        assert!(approx(profile.read(&inverted, &rest, None).brake, 1.0));
    }

    #[test]
    fn pedals_on_a_separate_device() {
        let mut profile = WheelProfile::guess(
            csl(),
            &DeviceState {
                axes: vec![0],
                buttons: vec![],
            },
        );
        profile.aux = Some(DeviceId {
            name: "Pedals".into(),
            vid: 0x0eb7,
            pid: 0x183b,
            guid: String::new(),
        });
        profile.gas = Some((
            Source::Aux,
            PedalAxis {
                axis: 0,
                rest: -32_768,
                peak: 32_767,
            },
        ));
        let wheel = DeviceState {
            axes: vec![0],
            buttons: vec![],
        };
        let pedals = DeviceState {
            axes: vec![32_767],
            buttons: vec![],
        };
        assert!(approx(
            profile
                .read(&WheelSettings::default(), &wheel, Some(&pedals))
                .accel,
            1.0
        ));
        // pedals unplugged: released, not stuck
        assert_eq!(
            profile.read(&WheelSettings::default(), &wheel, None).accel,
            0.0
        );
    }

    #[test]
    fn profiles_round_trip_through_ini() {
        let mut profile = WheelProfile::guess(
            csl(),
            &DeviceState {
                axes: vec![0, 32_767, -32_768],
                buttons: vec![],
            },
        );
        profile.aux = Some(DeviceId {
            name: "CSL Pedals".into(),
            vid: 0x0eb7,
            pid: 0x6204,
            guid: "abc".into(),
        });
        profile.assign(
            Button::Exit,
            Binding {
                source: Source::Aux,
                button: 3,
            },
        );
        let text = profile.to_ini().to_string();
        let back = WheelProfile::from_ini(&Ini::parse(&text)).unwrap();
        assert_eq!(back, profile);
        assert!(WheelProfile::from_ini(&Ini::parse("steer_axis=1")).is_none());
    }

    #[test]
    fn gamepad_layout() {
        let s = PadSettings::default();
        let mut pad = PadState {
            left_x: -32_767,
            right_trigger: 32_767,
            x: true,
            ..Default::default()
        };
        let cab = read_pad(&s, &pad);
        assert!(approx(cab.steer, -1.0));
        assert!(approx(cab.accel, 1.0));
        assert!(cab.is_pressed(Button::Light));

        pad = PadState {
            b: true,
            dpad_right: true,
            start: true,
            ..Default::default()
        };
        let cab = read_pad(&s, &pad);
        assert_eq!(cab.steer, 1.0);
        assert_eq!(cab.brake, 1.0);
        assert!(cab.is_pressed(Button::Start) && !cab.is_pressed(Button::Exit));

        pad.back = true;
        let cab = read_pad(&s, &pad);
        assert!(
            cab.is_pressed(Button::Exit)
                && !cab.is_pressed(Button::Start)
                && !cab.is_pressed(Button::Coin)
        );

        // the stick's deadzone keeps a resting stick centred
        let drift = PadState {
            left_x: 2_000,
            ..Default::default()
        };
        assert_eq!(read_pad(&s, &drift).steer, 0.0);
    }

    #[test]
    fn unknown_wheels_get_the_generic_paddles() {
        let id = DeviceId {
            name: "Some Wheel".into(),
            vid: 0x1234,
            pid: 0x0001,
            guid: String::new(),
        };
        let profile = WheelProfile::guess(id, &DeviceState::default());
        assert_eq!(profile.describe(Action::Press(Button::Light)), "BUTTON 1");
        assert_eq!(profile.describe(Action::Press(Button::Wiper)), "BUTTON 2");
        assert_eq!(profile.describe(Action::Press(Button::Start)), "-");
    }

    #[test]
    fn a_remapped_wheel_button_leaves_its_old_control() {
        let mut profile = WheelProfile::guess(csl(), &DeviceState::default());
        let paddle = profile.binding(Button::Light).unwrap();
        profile.assign(Button::Start, paddle);
        assert_eq!(profile.binding(Button::Start), Some(paddle));
        assert_eq!(profile.binding(Button::Light), None);
    }

    #[test]
    fn captures_from_a_second_device_make_it_the_aux() {
        let pedals = DeviceId {
            name: "Pedals".into(),
            vid: 0x0eb7,
            pid: 0x6204,
            guid: "p".into(),
        };
        let mut profile = WheelProfile::guess(csl(), &DeviceState::default());
        let gas = PedalAxis {
            axis: 1,
            rest: 32_767,
            peak: -32_768,
        };
        let captures = wizard::Captures {
            gas: Some((1, gas)),
            buttons: vec![(Button::Coin, 0, 7)],
            ..Default::default()
        };
        profile.apply(&captures, &[csl(), pedals.clone()]);
        assert_eq!(profile.aux, Some(pedals));
        assert_eq!(profile.describe(Action::Accelerate), "AUX AXIS 2");
        assert_eq!(profile.describe(Action::Press(Button::Coin)), "BUTTON 8");

        // pedals moved back onto the wheel: the old aux and its pedal go
        let mut moved = profile.clone();
        moved.forget_aux();
        assert_eq!(moved.aux, None);
        assert_eq!(moved.describe(Action::Accelerate), "-");
        assert_eq!(moved.describe(Action::Press(Button::Coin)), "BUTTON 8");
    }

    #[test]
    fn a_pedal_moved_onto_the_other_pedals_axis_takes_it_over() {
        let at_top = |axis| PedalAxis {
            axis,
            rest: 32_767,
            peak: -32_768,
        };
        let mut profile = WheelProfile::guess(csl(), &DeviceState::default());
        profile.gas = Some((Source::Wheel, at_top(1)));
        profile.brake = Some((Source::Wheel, at_top(2)));
        let captures = wizard::Captures {
            gas: Some((0, at_top(2))),
            ..Default::default()
        };
        profile.apply(&captures, &[csl()]);
        assert_eq!(profile.describe(Action::Accelerate), "AXIS 3");
        assert_eq!(profile.describe(Action::Brake), "-");

        // one axis, opposite directions: a combined pedal axis keeps both
        let mut combined = WheelProfile::guess(csl(), &DeviceState::default());
        combined.brake = Some((
            Source::Wheel,
            PedalAxis {
                axis: 1,
                rest: 0,
                peak: 32_767,
            },
        ));
        let captures = wizard::Captures {
            gas: Some((
                0,
                PedalAxis {
                    axis: 1,
                    rest: 0,
                    peak: -32_768,
                },
            )),
            ..Default::default()
        };
        combined.apply(&captures, &[csl()]);
        assert_eq!(combined.describe(Action::Brake), "AXIS 2");
    }

    #[test]
    fn a_remapped_pad_follows_its_map() {
        let mut s = PadSettings::default();
        s.map.assign(Action::Accelerate, PadInput::RightShoulder);
        s.map.assign(Action::Press(Button::Light), PadInput::A);
        let pad = PadState {
            right_shoulder: true,
            a: true,
            right_trigger: 32_767,
            ..Default::default()
        };
        let cab = read_pad(&s, &pad);
        // RB is the accelerator now, A the lights; RT does nothing
        assert_eq!(cab.accel, 1.0);
        assert!(cab.is_pressed(Button::Light) && !cab.is_pressed(Button::Wiper));
        let trigger_only = PadState {
            right_trigger: 32_767,
            ..Default::default()
        };
        assert_eq!(read_pad(&s, &trigger_only).accel, 0.0);
    }

    #[test]
    fn back_and_start_leave_the_game_whatever_is_bound() {
        let mut s = PadSettings::default();
        s.map.set(Action::Press(Button::Exit), &[]);
        let pad = PadState {
            back: true,
            start: true,
            ..Default::default()
        };
        let cab = read_pad(&s, &pad);
        assert!(cab.is_pressed(Button::Exit) && !cab.is_pressed(Button::Start));
    }
}
