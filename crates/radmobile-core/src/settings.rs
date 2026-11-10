//! Every option the menu offers, stored in `settings.ini` next to the
//! launcher (Wanszai keeps the same kind of file). Missing or bad values
//! fall back to the defaults; values out of range are clamped.

use std::path::PathBuf;

use crate::eeprom::GameSettings;
use crate::ffb::FfbSettings;
use crate::ini::Ini;
use crate::wheel::{PadSettings, SteeringLock, WheelSettings};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Fullscreen,
    Window,
}

/// The size the picture is rendered at.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    /// The desktop's (fullscreen) or a 1280×960-class window.
    Desktop,
    P1080,
    P1440,
    P2160,
}

impl Resolution {
    pub const ALL: [Resolution; 4] = [
        Resolution::Desktop,
        Resolution::P1080,
        Resolution::P1440,
        Resolution::P2160,
    ];

    pub fn height(self) -> Option<u32> {
        match self {
            Resolution::Desktop => None,
            Resolution::P1080 => Some(1080),
            Resolution::P1440 => Some(1440),
            Resolution::P2160 => Some(2160),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Resolution::Desktop => "Desktop",
            Resolution::P1080 => "1080p",
            Resolution::P1440 => "1440p",
            Resolution::P2160 => "2160p (4K)",
        }
    }
}

/// How the 4:3 arcade picture meets a wide screen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Aspect {
    /// 4:3 with side bars.
    Original,
    /// The 4:3 picture stretched to fill the screen.
    Stretch,
    /// More of the world on the sides (our MAME patch).
    TrueWidescreen,
}

impl Aspect {
    pub const ALL: [Aspect; 3] = [Aspect::Original, Aspect::Stretch, Aspect::TrueWidescreen];

    pub fn label(self) -> &'static str {
        match self {
            Aspect::Original => "4:3 Original",
            Aspect::Stretch => "Stretch to fill",
            Aspect::TrueWidescreen => "True widescreen",
        }
    }
}

/// The screen shape true widescreen aims for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WideTarget {
    W16x9,
    W16x10,
    W21x9,
}

impl WideTarget {
    pub const ALL: [WideTarget; 3] = [WideTarget::W16x9, WideTarget::W16x10, WideTarget::W21x9];

    pub fn ratio(self) -> (u32, u32) {
        match self {
            WideTarget::W16x9 => (16, 9),
            WideTarget::W16x10 => (16, 10),
            WideTarget::W21x9 => (21, 9),
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            WideTarget::W16x9 => "16:9",
            WideTarget::W16x10 => "16:10",
            WideTarget::W21x9 => "21:9",
        }
    }
}

/// Picture filter (MAME bgfx screen chains).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Filter {
    /// Crisp pixels.
    Sharp,
    /// Bilinear smoothing.
    Smooth,
    /// CRT shader (curvature, scanlines).
    Crt,
    /// CRT with a glass bezel and glow.
    CrtDeluxe,
}

impl Filter {
    pub const ALL: [Filter; 4] = [
        Filter::Sharp,
        Filter::Smooth,
        Filter::Crt,
        Filter::CrtDeluxe,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Filter::Sharp => "Sharp",
            Filter::Smooth => "Smooth",
            Filter::Crt => "CRT",
            Filter::CrtDeluxe => "CRT Deluxe",
        }
    }

    pub fn bgfx_chain(self) -> &'static str {
        match self {
            Filter::Sharp => "unfiltered",
            Filter::Smooth => "default",
            Filter::Crt => "crt-geom",
            Filter::CrtDeluxe => "crt-geom-deluxe",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputMode {
    /// The wheel when one is connected, otherwise a gamepad.
    Auto,
    Wheel,
    Controller,
}

impl InputMode {
    pub const ALL: [InputMode; 3] = [InputMode::Auto, InputMode::Wheel, InputMode::Controller];

    pub fn label(self) -> &'static str {
        match self {
            InputMode::Auto => "Auto",
            InputMode::Wheel => "Wheel",
            InputMode::Controller => "Controller",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub display: Display,
    pub resolution: Resolution,
    pub aspect: Aspect,
    pub wide_target: WideTarget,
    pub filter: Filter,
    pub vsync: bool,
    /// 50..=150 %.
    pub brightness: u8,
    /// 0..=100 %.
    pub volume: u8,
    /// "radm" or "radmu"; which set to prefer when both are present.
    pub region: String,
    pub game: GameSettings,
    pub input: InputMode,
    pub wheel: WheelSettings,
    pub pad: PadSettings,
    pub ffb: FfbSettings,
    /// Where to look for the ROMs besides the launcher's `roms` folder.
    pub rom_dir: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            display: Display::Fullscreen,
            resolution: Resolution::Desktop,
            aspect: Aspect::Original,
            wide_target: WideTarget::W16x9,
            filter: Filter::Smooth,
            vsync: true,
            brightness: 100,
            volume: 100,
            region: "radm".into(),
            game: GameSettings::default(),
            input: InputMode::Auto,
            wheel: WheelSettings::default(),
            pad: PadSettings::default(),
            ffb: FfbSettings::default(),
            rom_dir: None,
        }
    }
}

fn pick<T: Copy + PartialEq>(all: &[T], key: Option<&str>, names: &[&str], default: T) -> T {
    key.and_then(|k| names.iter().position(|n| n.eq_ignore_ascii_case(k)))
        .and_then(|i| all.get(i).copied())
        .unwrap_or(default)
}

impl Settings {
    pub fn from_ini(ini: &Ini) -> Self {
        let d = Settings::default();
        let num = |k: &str, def: i64, lo: i64, hi: i64| {
            ini.get_parsed::<i64>(k).unwrap_or(def).clamp(lo, hi)
        };
        let flag = |k: &str, def: bool| ini.get_bool(k).unwrap_or(def);
        let game = GameSettings {
            difficulty: num("difficulty", d.game.difficulty.into(), 1, 16) as u8,
            timer: num("timer", d.game.timer.into(), 40, 60) as u8,
            rival_speed: num("rival_speed", d.game.rival_speed.into(), 1, 16) as u8,
            police_speed: num("police_speed", d.game.police_speed.into(), 1, 16) as u8,
            attract_sound: flag("attract_sound", d.game.attract_sound),
            free_play: flag("free_play", d.game.free_play),
            credits_to_start: num("credits_start", d.game.credits_to_start.into(), 1, 5) as u8,
            credits_to_continue: num("credits_continue", d.game.credits_to_continue.into(), 1, 5)
                as u8,
        };
        Settings {
            display: pick(
                &[Display::Fullscreen, Display::Window],
                ini.get("display"),
                &["fullscreen", "window"],
                d.display,
            ),
            resolution: pick(
                &Resolution::ALL,
                ini.get("resolution"),
                &["desktop", "1080p", "1440p", "2160p"],
                d.resolution,
            ),
            aspect: pick(
                &Aspect::ALL,
                ini.get("aspect"),
                &["original", "stretch", "widescreen"],
                d.aspect,
            ),
            wide_target: pick(
                &WideTarget::ALL,
                ini.get("wide_target"),
                &["16:9", "16:10", "21:9"],
                d.wide_target,
            ),
            filter: pick(
                &Filter::ALL,
                ini.get("filter"),
                &["sharp", "smooth", "crt", "crt_deluxe"],
                d.filter,
            ),
            vsync: flag("vsync", d.vsync),
            brightness: num("brightness", d.brightness.into(), 50, 150) as u8,
            volume: num("volume", d.volume.into(), 0, 100) as u8,
            region: match ini.get("region") {
                Some("radmu") => "radmu".into(),
                _ => "radm".into(),
            },
            game,
            input: pick(
                &InputMode::ALL,
                ini.get("input_device"),
                &["auto", "wheel", "controller"],
                d.input,
            ),
            wheel: WheelSettings {
                lock: SteeringLock::from_setting(num(
                    "steer_lock",
                    d.wheel.lock.to_setting(),
                    0,
                    3600,
                )),
                invert_accel: flag("invert_accel", false),
                invert_brake: flag("invert_brake", false),
            },
            pad: PadSettings {
                stick_deadzone: num("stick_deadzone", d.pad.stick_deadzone.into(), 0, 16_000)
                    as u16,
                stick_linearity: num("stick_linearity", d.pad.stick_linearity.into(), 0, 100) as u8,
                trigger_deadzone: num("trigger_deadzone", d.pad.trigger_deadzone.into(), 0, 64)
                    as u8,
                trigger_linearity: num("trigger_linearity", d.pad.trigger_linearity.into(), 0, 100)
                    as u8,
                steer_assist: flag("steer_assist", d.pad.steer_assist),
            },
            ffb: FfbSettings {
                strength: num("ffb_strength", d.ffb.strength.into(), 0, 100) as u8,
                invert: flag("ffb_invert", d.ffb.invert),
                rumble: num("rumble_strength", d.ffb.rumble.into(), 0, 100) as u8,
            },
            rom_dir: ini
                .get("rom_dir")
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
        }
    }

    pub fn to_ini(&self) -> Ini {
        let mut ini = Ini::default();
        ini.comment("settings.ini - Rad Mobile launcher");
        let b = |v: bool| if v { "1" } else { "0" };
        ini.set(
            "display",
            if self.display == Display::Window {
                "window"
            } else {
                "fullscreen"
            },
        );
        ini.set(
            "resolution",
            ["desktop", "1080p", "1440p", "2160p"][Resolution::ALL
                .iter()
                .position(|r| *r == self.resolution)
                .unwrap()],
        );
        ini.set(
            "aspect",
            ["original", "stretch", "widescreen"]
                [Aspect::ALL.iter().position(|a| *a == self.aspect).unwrap()],
        );
        ini.set("wide_target", self.wide_target.label());
        ini.set(
            "filter",
            ["sharp", "smooth", "crt", "crt_deluxe"]
                [Filter::ALL.iter().position(|f| *f == self.filter).unwrap()],
        );
        ini.set("vsync", b(self.vsync));
        ini.set("brightness", &self.brightness.to_string());
        ini.set("volume", &self.volume.to_string());
        ini.set("region", &self.region);
        ini.set("difficulty", &self.game.difficulty.to_string());
        ini.set("timer", &self.game.timer.to_string());
        ini.set("rival_speed", &self.game.rival_speed.to_string());
        ini.set("police_speed", &self.game.police_speed.to_string());
        ini.set("attract_sound", b(self.game.attract_sound));
        ini.set("free_play", b(self.game.free_play));
        ini.set("credits_start", &self.game.credits_to_start.to_string());
        ini.set(
            "credits_continue",
            &self.game.credits_to_continue.to_string(),
        );
        ini.set("input_device", &self.input.label().to_ascii_lowercase());
        ini.set("steer_lock", &self.wheel.lock.to_setting().to_string());
        ini.set("invert_accel", b(self.wheel.invert_accel));
        ini.set("invert_brake", b(self.wheel.invert_brake));
        ini.set("stick_deadzone", &self.pad.stick_deadzone.to_string());
        ini.set("stick_linearity", &self.pad.stick_linearity.to_string());
        ini.set("trigger_deadzone", &self.pad.trigger_deadzone.to_string());
        ini.set("trigger_linearity", &self.pad.trigger_linearity.to_string());
        ini.set("steer_assist", b(self.pad.steer_assist));
        ini.set("ffb_strength", &self.ffb.strength.to_string());
        ini.set("ffb_invert", b(self.ffb.invert));
        ini.set("rumble_strength", &self.ffb.rumble.to_string());
        ini.set(
            "rom_dir",
            &self
                .rom_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
        ini
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_follow_wanszai() {
        let s = Settings::default();
        assert_eq!(s.wheel.lock, SteeringLock::Degrees(270));
        assert_eq!(s.ffb.strength, 50);
        assert_eq!(s.pad.stick_deadzone, 3000);
        assert!(s.game.free_play);
        assert_eq!(Settings::from_ini(&Ini::default()), s);
    }

    #[test]
    fn round_trips_every_field() {
        let s = Settings {
            display: Display::Window,
            resolution: Resolution::P2160,
            aspect: Aspect::TrueWidescreen,
            wide_target: WideTarget::W21x9,
            filter: Filter::CrtDeluxe,
            vsync: false,
            brightness: 120,
            volume: 35,
            region: "radmu".into(),
            game: GameSettings {
                difficulty: 12,
                timer: 60,
                free_play: false,
                attract_sound: true,
                ..Default::default()
            },
            input: InputMode::Wheel,
            wheel: WheelSettings {
                lock: SteeringLock::Full,
                invert_brake: true,
                ..Default::default()
            },
            pad: PadSettings {
                stick_linearity: 40,
                steer_assist: false,
                ..Default::default()
            },
            ffb: FfbSettings {
                strength: 75,
                invert: true,
                rumble: 0,
            },
            rom_dir: Some(PathBuf::from("/games/roms")),
        };
        let text = s.to_ini().to_string();
        assert_eq!(Settings::from_ini(&Ini::parse(&text)), s);
    }

    #[test]
    fn bad_values_fall_back_or_clamp() {
        let s = Settings::from_ini(&Ini::parse(
            "difficulty=99\nffb_strength=-5\naspect=round\nsteer_lock=275\nregion=moon\n",
        ));
        assert_eq!(s.game.difficulty, 16);
        assert_eq!(s.ffb.strength, 0);
        assert_eq!(s.aspect, Aspect::Original);
        assert_eq!(s.wheel.lock, SteeringLock::Degrees(270));
        assert_eq!(s.region, "radm");
    }
}
