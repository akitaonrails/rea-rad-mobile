//! Turning raw wheel, pedal and gamepad readings into cabinet controls,
//! the way Wanszai's wrappers do (docs/WANSZAI_WHEELS.md §2–3).

pub mod known;
pub mod wizard;

use crate::controls::{Button, Cabinet};
use crate::ini::Ini;

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
    /// axis, pedals from their resting position, paddles from the table.
    pub fn guess(wheel: DeviceId, initial: &DeviceState) -> Self {
        let known = known::lookup(wheel.vid, wheel.pid);
        let rotation = known.map_or(known::GENERIC_ROTATION, |k| k.rotation);
        let (gas, brake) = guess_pedals(initial);
        let mut buttons = Vec::new();
        if let Some((up, down)) = known.and_then(|k| k.paddles) {
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

    pub fn bind(&mut self, button: Button, binding: Option<Binding>) {
        self.buttons.retain(|(b, _)| *b != button);
        if let Some(binding) = binding {
            self.buttons.push((button, binding));
            self.buttons.sort_by_key(|(b, _)| *b);
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
            ini.set(&format!("btn_{}", button_key(*button)), &bind.format());
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
                .get(&format!("btn_{}", button_key(button)))
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

fn button_key(b: Button) -> &'static str {
    match b {
        Button::Light => "light",
        Button::Wiper => "wiper",
        Button::Start => "start",
        Button::Coin => "coin",
        Button::Service => "service",
        Button::Test => "test",
        Button::Pause => "pause",
        Button::Exit => "exit",
    }
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
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelSettings {
    pub lock: SteeringLock,
    pub invert_accel: bool,
    pub invert_brake: bool,
}

impl Default for WheelSettings {
    fn default() -> Self {
        Self {
            lock: SteeringLock::DEFAULT,
            invert_accel: false,
            invert_brake: false,
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
    pub dpad_left: bool,
    pub dpad_right: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadSettings {
    /// Of 32767.
    pub stick_deadzone: u16,
    pub stick_linearity: u8,
    /// Of 255.
    pub trigger_deadzone: u8,
    pub trigger_linearity: u8,
}

impl Default for PadSettings {
    fn default() -> Self {
        Self {
            stick_deadzone: 3000,
            stick_linearity: 0,
            trigger_deadzone: 12,
            trigger_linearity: 0,
        }
    }
}

/// Gamepad layout: left stick steers (d-pad too), right trigger or A
/// accelerates, left trigger or B brakes, X lights, Y wiper, Start starts,
/// Back inserts a coin, Back + Start (or Guide) returns to the menu.
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
    let trigger = |v: i16, digital: bool| {
        let analog = shape(
            f32::from(v.max(0)) / RAW_MAX,
            f32::from(settings.trigger_deadzone) / 255.0,
            settings.trigger_linearity,
        );
        if digital { 1.0 } else { analog }
    };
    let mut cab = Cabinet {
        steer: if dpad != 0.0 { dpad } else { stick },
        accel: trigger(pad.right_trigger, pad.a),
        brake: trigger(pad.left_trigger, pad.b),
        buttons: 0,
    };
    let exit = pad.guide || (pad.back && pad.start);
    if exit {
        cab.press(Button::Exit);
    } else {
        if pad.start {
            cab.press(Button::Start);
        }
        if pad.back {
            cab.press(Button::Coin);
        }
    }
    if pad.x || pad.left_shoulder {
        cab.press(Button::Light);
    }
    if pad.y || pad.right_shoulder {
        cab.press(Button::Wiper);
    }
    cab
}

#[cfg(test)]
mod tests {
    use super::*;

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
        profile.bind(
            Button::Exit,
            Some(Binding {
                source: Source::Aux,
                button: 3,
            }),
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
}
