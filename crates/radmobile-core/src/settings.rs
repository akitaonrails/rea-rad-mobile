//! Every option the menu offers, stored in `settings.ini` next to the
//! launcher (Wanszai keeps the same kind of file). Missing or bad values
//! fall back to the defaults; values out of range are clamped.

use std::path::PathBuf;

use rea_launcher::cfg;

use crate::controls::Action;
use crate::eeprom::GameSettings;
use crate::ffb::FfbSettings;
use crate::keys::{self, Keys, put};
use crate::pad_map;
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
    /// Wanszai's "ARCADE" CRT (Scud Race PC): shadow mask, strong scanlines
    /// and glow (`bgfx/` in this repository).
    ArcadeCrt,
    /// Wanszai's "TRINITRON" CRT: aperture grille, softer lines.
    TrinitronCrt,
}

impl Filter {
    pub const ALL: [Filter; 6] = [
        Filter::Sharp,
        Filter::Smooth,
        Filter::ArcadeCrt,
        Filter::TrinitronCrt,
        Filter::Crt,
        Filter::CrtDeluxe,
    ];

    /// The name in settings files.
    fn key(self) -> &'static str {
        match self {
            Filter::Sharp => "sharp",
            Filter::Smooth => "smooth",
            Filter::Crt => "crt",
            Filter::CrtDeluxe => "crt_deluxe",
            Filter::ArcadeCrt => "arcade_crt",
            Filter::TrinitronCrt => "trinitron_crt",
        }
    }

    /// Whether the CRT CURVATURE switch applies (the Wanszai-style CRTs).
    pub fn has_curvature_switch(self) -> bool {
        matches!(self, Filter::ArcadeCrt | Filter::TrinitronCrt)
    }

    pub fn label(self) -> &'static str {
        match self {
            Filter::Sharp => "Sharp",
            Filter::Smooth => "Smooth",
            Filter::Crt => "CRT",
            Filter::CrtDeluxe => "CRT Deluxe",
            Filter::ArcadeCrt => "Arcade CRT",
            Filter::TrinitronCrt => "Trinitron CRT",
        }
    }

    /// MAME's `-bgfx_screen_chains`; `curved` picks the curved screen of the
    /// filters that have [`Filter::has_curvature_switch`].
    pub fn bgfx_chain(self, curved: bool) -> &'static str {
        match (self, curved) {
            (Filter::Sharp, _) => "unfiltered",
            (Filter::Smooth, _) => "default",
            (Filter::Crt, _) => "crt-geom",
            (Filter::CrtDeluxe, _) => "crt-geom-deluxe",
            (Filter::ArcadeCrt, true) => "rea-crt-arcade",
            (Filter::ArcadeCrt, false) => "rea-crt-arcade-flat",
            (Filter::TrinitronCrt, true) => "rea-crt-trinitron",
            (Filter::TrinitronCrt, false) => "rea-crt-trinitron-flat",
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

/// How long each second of the race clock lasts. Our MAME patch slows
/// the game's own clock (docs/RAM_MAP.md), so the starting time and every
/// checkpoint extension last longer alike.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RaceTime {
    Arcade,
    X1_5,
    X2,
    X3,
}

impl RaceTime {
    pub const ALL: [RaceTime; 4] = [RaceTime::Arcade, RaceTime::X1_5, RaceTime::X2, RaceTime::X3];
    const NAMES: [&'static str; 4] = ["arcade", "1.5x", "2x", "3x"];

    /// The clock's length against the arcade's, for `REA_RADM_TIME_SCALE`.
    pub fn percent(self) -> u16 {
        match self {
            RaceTime::Arcade => 100,
            RaceTime::X1_5 => 150,
            RaceTime::X2 => 200,
            RaceTime::X3 => 300,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            RaceTime::Arcade => "Arcade",
            RaceTime::X1_5 => "1.5x",
            RaceTime::X2 => "2x",
            RaceTime::X3 => "3x",
        }
    }
}

/// Ready-made sets of the arcade settings that decide how hard a race is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Preset {
    /// The factory settings.
    Arcade,
    /// Easiest difficulty, slowest rivals and police, a 60 second start
    /// and a clock at half speed.
    Easy,
}

impl Preset {
    pub const ALL: [Preset; 2] = [Preset::Arcade, Preset::Easy];

    pub fn label(self) -> &'static str {
        match self {
            Preset::Arcade => "Arcade",
            Preset::Easy => "Easy",
        }
    }

    fn values(self) -> (GameSettings, RaceTime) {
        let arcade = GameSettings::default();
        match self {
            Preset::Arcade => (arcade, RaceTime::Arcade),
            Preset::Easy => (
                GameSettings {
                    difficulty: 1,
                    timer: 60,
                    rival_speed: 1,
                    police_speed: 1,
                    ..arcade
                },
                RaceTime::X2,
            ),
        }
    }

    /// Set what the preset covers; free play, credits and sound are kept.
    pub fn apply(self, s: &mut Settings) {
        let (g, race_time) = self.values();
        s.game.difficulty = g.difficulty;
        s.game.timer = g.timer;
        s.game.rival_speed = g.rival_speed;
        s.game.police_speed = g.police_speed;
        s.race_time = race_time;
    }

    /// The preset the settings match, if any.
    pub fn of(s: &Settings) -> Option<Preset> {
        Preset::ALL.into_iter().find(|p| {
            let mut probe = s.clone();
            p.apply(&mut probe);
            probe == *s
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    pub display: Display,
    pub resolution: Resolution,
    pub aspect: Aspect,
    pub wide_target: WideTarget,
    pub filter: Filter,
    /// The curved screen of the Wanszai-style CRT filters.
    pub crt_curvature: bool,
    /// The cabinet frame around the 4:3 picture.
    pub bezel: bool,
    pub vsync: bool,
    /// 50..=150 %.
    pub brightness: u8,
    /// 0..=100 %.
    pub volume: u8,
    /// "radm" or "radmu"; which set to prefer when both are present.
    pub region: String,
    pub game: GameSettings,
    pub race_time: RaceTime,
    pub input: InputMode,
    pub wheel: WheelSettings,
    pub pad: PadSettings,
    /// The pad's stick asks for an angle to the road instead of moving the
    /// cabinet wheel directly ([`crate::assist`]).
    pub steer_assist: bool,
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
            crt_curvature: true,
            bezel: false,
            vsync: true,
            brightness: 100,
            volume: 100,
            region: "radm".into(),
            game: GameSettings::default(),
            race_time: RaceTime::Arcade,
            input: InputMode::Auto,
            wheel: WheelSettings::default(),
            pad: PadSettings::new(pad_map::layout()),
            steer_assist: true,
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
    /// The settings in a `settings.ini`'s text; anything missing or
    /// unusable keeps its default, numbers are clamped to their range.
    pub fn from_cfg(text: &str) -> Self {
        let ini = Keys::of(text);
        let d = Settings::default();
        let num =
            |k: &str, def: i64, lo: i64, hi: i64| ini.parsed::<i64>(k).unwrap_or(def).clamp(lo, hi);
        let flag = |k: &str, def: bool| ini.flag(k).unwrap_or(def);
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
            filter: ini
                .get("filter")
                .and_then(|k| {
                    Filter::ALL
                        .into_iter()
                        .find(|f| f.key().eq_ignore_ascii_case(k))
                })
                .unwrap_or(d.filter),
            crt_curvature: flag("crt_curvature", d.crt_curvature),
            bezel: flag("bezel", d.bezel),
            vsync: flag("vsync", d.vsync),
            brightness: num("brightness", d.brightness.into(), 50, 150) as u8,
            volume: num("volume", d.volume.into(), 0, 100) as u8,
            region: match ini.get("region") {
                Some(r) if r.eq_ignore_ascii_case("radmu") => "radmu".into(),
                _ => "radm".into(),
            },
            game,
            race_time: pick(
                &RaceTime::ALL,
                ini.get("race_time"),
                &RaceTime::NAMES,
                d.race_time,
            ),
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
                device: ini
                    .get("wheel_device")
                    .unwrap_or_default()
                    .trim()
                    .to_string(),
            },
            pad: PadSettings {
                stick_deadzone: num("stick_deadzone", d.pad.stick_deadzone.into(), 0, 16_000)
                    as u16,
                stick_linearity: num("stick_linearity", d.pad.stick_linearity.into(), 0, 100) as u8,
                trigger_deadzone: num("trigger_deadzone", d.pad.trigger_deadzone.into(), 0, 64)
                    as u8,
                trigger_linearity: num("trigger_linearity", d.pad.trigger_linearity.into(), 0, 100)
                    as u8,
                map: pad_map::from_cfg(text),
            },
            steer_assist: flag("steer_assist", d.steer_assist),
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

    /// The settings written into `existing` (a `settings.ini`'s text, or
    /// "" for a new file): each setting's `key = value` line is replaced or
    /// added, switches as `on`/`off`; the player's comments, order and other
    /// lines stay.
    pub fn to_cfg(&self, existing: &str) -> String {
        let mut text = keys::with_header(existing, "settings.ini - Rad Mobile launcher");
        let b = cfg::on_off;
        put(
            &mut text,
            "display",
            if self.display == Display::Window {
                "window"
            } else {
                "fullscreen"
            },
        );
        put(
            &mut text,
            "resolution",
            ["desktop", "1080p", "1440p", "2160p"][Resolution::ALL
                .iter()
                .position(|r| *r == self.resolution)
                .unwrap()],
        );
        put(
            &mut text,
            "aspect",
            ["original", "stretch", "widescreen"]
                [Aspect::ALL.iter().position(|a| *a == self.aspect).unwrap()],
        );
        put(&mut text, "wide_target", self.wide_target.label());
        put(&mut text, "filter", self.filter.key());
        put(&mut text, "crt_curvature", b(self.crt_curvature));
        put(&mut text, "bezel", b(self.bezel));
        put(&mut text, "vsync", b(self.vsync));
        put(&mut text, "brightness", &self.brightness.to_string());
        put(&mut text, "volume", &self.volume.to_string());
        put(&mut text, "region", &self.region);
        put(&mut text, "difficulty", &self.game.difficulty.to_string());
        put(&mut text, "timer", &self.game.timer.to_string());
        put(&mut text, "rival_speed", &self.game.rival_speed.to_string());
        put(
            &mut text,
            "police_speed",
            &self.game.police_speed.to_string(),
        );
        put(
            &mut text,
            "race_time",
            RaceTime::NAMES[RaceTime::ALL
                .iter()
                .position(|r| *r == self.race_time)
                .unwrap()],
        );
        put(&mut text, "attract_sound", b(self.game.attract_sound));
        put(&mut text, "free_play", b(self.game.free_play));
        put(
            &mut text,
            "credits_start",
            &self.game.credits_to_start.to_string(),
        );
        put(
            &mut text,
            "credits_continue",
            &self.game.credits_to_continue.to_string(),
        );
        put(
            &mut text,
            "input_device",
            &self.input.label().to_ascii_lowercase(),
        );
        put(
            &mut text,
            "steer_lock",
            &self.wheel.lock.to_setting().to_string(),
        );
        put(&mut text, "invert_accel", b(self.wheel.invert_accel));
        put(&mut text, "invert_brake", b(self.wheel.invert_brake));
        put(&mut text, "wheel_device", &self.wheel.device);
        put(
            &mut text,
            "stick_deadzone",
            &self.pad.stick_deadzone.to_string(),
        );
        put(
            &mut text,
            "stick_linearity",
            &self.pad.stick_linearity.to_string(),
        );
        put(
            &mut text,
            "trigger_deadzone",
            &self.pad.trigger_deadzone.to_string(),
        );
        put(
            &mut text,
            "trigger_linearity",
            &self.pad.trigger_linearity.to_string(),
        );
        put(&mut text, "steer_assist", b(self.steer_assist));
        for action in Action::all() {
            text = keys::lower_key_case(&text, &format!("pad_{}", action.key()));
        }
        text = self.pad.map.to_cfg(&text);
        put(&mut text, "ffb_strength", &self.ffb.strength.to_string());
        put(&mut text, "ffb_invert", b(self.ffb.invert));
        put(&mut text, "rumble_strength", &self.ffb.rumble.to_string());
        put(
            &mut text,
            "rom_dir",
            &self
                .rom_dir
                .as_ref()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        );
        text
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Button;
    use crate::pad_map::PadInput;

    #[test]
    fn defaults_follow_wanszai() {
        let s = Settings::default();
        assert_eq!(s.wheel.lock, SteeringLock::Degrees(270));
        assert_eq!(s.ffb.strength, 50);
        assert_eq!(s.pad.stick_deadzone, 3000);
        assert!(s.game.free_play);
        assert_eq!(Settings::from_cfg(""), s);
    }

    #[test]
    fn round_trips_every_field() {
        let s = Settings {
            display: Display::Window,
            resolution: Resolution::P2160,
            aspect: Aspect::TrueWidescreen,
            wide_target: WideTarget::W21x9,
            filter: Filter::ArcadeCrt,
            crt_curvature: false,
            bezel: true,
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
            race_time: RaceTime::X3,
            input: InputMode::Wheel,
            wheel: WheelSettings {
                lock: SteeringLock::Full,
                invert_brake: true,
                device: r"\\?\hid#vid_0eb7&pid_0020&col01#8&1d2f&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}".into(),
                ..Default::default()
            },
            pad: PadSettings {
                stick_linearity: 40,
                map: {
                    let mut map = pad_map::layout();
                    map.assign(Action::Press(Button::Pause), PadInput::RightStick);
                    map
                },
                ..Settings::default().pad
            },
            steer_assist: false,
            ffb: FfbSettings {
                strength: 75,
                invert: true,
                rumble: 0,
            },
            rom_dir: Some(PathBuf::from("/games/roms")),
        };
        let text = s.to_cfg("");
        assert_eq!(Settings::from_cfg(&text), s);
        // saved again over itself: the same text
        assert_eq!(s.to_cfg(&text), text);
    }

    #[test]
    fn defaults_are_the_arcade_preset() {
        assert_eq!(Preset::of(&Settings::default()), Some(Preset::Arcade));
    }

    #[test]
    fn the_easy_preset_at_least_doubles_the_race_time() {
        let mut s = Settings::default();
        Preset::Easy.apply(&mut s);
        assert_eq!(Preset::of(&s), Some(Preset::Easy));
        let arcade = Settings::default();
        let seconds = |s: &Settings| u32::from(s.game.timer) * u32::from(s.race_time.percent());
        assert!(seconds(&s) >= 2 * seconds(&arcade));
        assert!(s.game.difficulty < arcade.game.difficulty);
    }

    #[test]
    fn a_preset_keeps_coins_and_sound() {
        let mut s = Settings::default();
        s.game.free_play = false;
        s.game.attract_sound = true;
        Preset::Easy.apply(&mut s);
        assert!(!s.game.free_play && s.game.attract_sound);
    }

    #[test]
    fn changed_preset_settings_match_no_preset() {
        let s = Settings {
            race_time: RaceTime::X1_5,
            ..Default::default()
        };
        assert_eq!(Preset::of(&s), None);
    }

    #[test]
    fn bad_values_fall_back_or_clamp() {
        let s = Settings::from_cfg(
            "difficulty=99\nffb_strength=-5\naspect=round\nsteer_lock=275\nregion=moon\nrace_time=9x\n",
        );
        assert_eq!(s.game.difficulty, 16);
        assert_eq!(s.ffb.strength, 0);
        assert_eq!(s.aspect, Aspect::Original);
        assert_eq!(s.wheel.lock, SteeringLock::Degrees(270));
        assert_eq!(s.region, "radm");
        assert_eq!(s.race_time, RaceTime::Arcade);
    }

    #[test]
    fn saving_keeps_the_players_lines_and_writes_keys_in_lower_case() {
        let old = "# my notes\r\n[launcher]\r\nDifficulty=3\r\nmy_own_key=1\r\n";
        let mut s = Settings::from_cfg(old);
        assert_eq!(s.game.difficulty, 3);
        s.game.difficulty = 5;
        let text = s.to_cfg(old);
        assert!(
            text.starts_with("# my notes\r\n[launcher]\r\ndifficulty = 5\r\nmy_own_key=1\r\n"),
            "{text}"
        );
        assert_eq!(Settings::from_cfg(&text), s);
        // a region in any case
        assert_eq!(Settings::from_cfg("region=RADMU").region, "radmu");
    }
}
