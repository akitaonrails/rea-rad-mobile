//! How the launcher runs the bundled MAME: the command line, the
//! environment, and the files MAME reads from the private data folder.

use std::path::{Path, PathBuf};

use crate::settings::{Aspect, Display, Settings};

/// Pixel aspect of the original 320×224 picture shown at 4:3.
pub const PIXEL_ASPECT: f64 = (4.0 / 3.0) / (320.0 / 224.0);
/// Our MAME patch's limit (512 pixels wide).
pub const MAX_WIDE_MARGIN: u32 = 96;

/// Pixels to add on each side of the 320-pixel picture for a screen of
/// `w:h` (even, so the picture stays on whole pixels).
pub fn wide_margin(w: u32, h: u32) -> u32 {
    let width = (f64::from(w) / f64::from(h)) * 224.0 / PIXEL_ASPECT;
    let margin = ((width - 320.0) / 2.0).round().max(0.0) as u32;
    (margin & !1).min(MAX_WIDE_MARGIN)
}

/// The shape of the wide picture (width over height) for a margin.
pub fn wide_aspect(margin: u32) -> f64 {
    f64::from(320 + 2 * margin) * PIXEL_ASPECT / 224.0
}

/// MAME artwork that shows the screen at the wide picture's real shape
/// (MAME would otherwise squeeze it into 4:3).
pub fn widescreen_layout(margin: u32) -> String {
    let aspect = wide_aspect(margin);
    format!(
        "<?xml version=\"1.0\"?>\n<!-- written by the Rad Mobile launcher -->\n<mamelayout version=\"2\">\n  <view name=\"Widescreen\">\n    <screen index=\"0\"><bounds x=\"0\" y=\"0\" width=\"{:.4}\" height=\"1\" /></screen>\n  </view>\n</mamelayout>\n",
        aspect
    )
}

/// Where everything MAME needs lives.
#[derive(Debug, Clone)]
pub struct Paths {
    /// The `radmobile` executable.
    pub exe: PathBuf,
    /// Folder holding MAME's `bgfx` shaders.
    pub mame_dir: PathBuf,
    /// The rebuilt romset.
    pub roms: PathBuf,
    /// Writable data folder: cfg, nvram, ctrlr, artwork, snaps.
    pub data: PathBuf,
    pub bridge: PathBuf,
}

impl Paths {
    pub fn cfg(&self) -> PathBuf {
        self.data.join("cfg")
    }
    pub fn nvram(&self) -> PathBuf {
        self.data.join("nvram")
    }
    pub fn ctrlr(&self) -> PathBuf {
        self.data.join("ctrlr")
    }
    pub fn artwork(&self) -> PathBuf {
        self.data.join("artwork")
    }
    pub fn snaps(&self) -> PathBuf {
        self.data.join("snap")
    }
    /// MAME's EEPROM file for `set`.
    pub fn eeprom(&self, set: &str) -> PathBuf {
        self.nvram().join(set).join("mainpcb_eeprom")
    }
}

/// A screen the launcher knows about (for fullscreen sizing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Screen {
    pub width: u32,
    pub height: u32,
}

fn path(p: &Path) -> String {
    p.display().to_string()
}

/// The full MAME command line (without the executable).
pub fn args(s: &Settings, set: &str, paths: &Paths, screen: Option<Screen>) -> Vec<String> {
    let mut a: Vec<String> = vec![set.into()];
    let mut opt = |k: &str, v: String| {
        a.push(format!("-{k}"));
        a.push(v);
    };
    // never read a stray mame.ini; everything comes from here
    opt("rompath", path(&paths.roms));
    opt("cfg_directory", path(&paths.cfg()));
    opt("nvram_directory", path(&paths.nvram()));
    opt("ctrlrpath", path(&paths.ctrlr()));
    opt("ctrlr", "rea".into());
    // ours (the widescreen layout), then MAME's (the CRT shaders' textures)
    opt(
        "artpath",
        format!(
            "{};{}",
            path(&paths.artwork()),
            path(&paths.mame_dir.join("artwork"))
        ),
    );
    opt("snapshot_directory", path(&paths.snaps()));
    opt("bgfx_path", path(&paths.mame_dir.join("bgfx")));
    opt("joystickprovider", "reabridge".into());
    opt("output", "reabridge".into());
    opt("joystick_deadzone", "0".into());
    opt("joystick_saturation", "1".into());
    opt("video", "bgfx".into());
    opt("bgfx_screen_chains", s.filter.bgfx_chain().into());
    opt(
        "brightness",
        format!("{:.2}", f32::from(s.brightness) / 100.0),
    );
    opt("volume", volume_db(s.volume).to_string());
    opt("samplerate", "48000".into());

    let height = s.resolution.height();
    match s.display {
        Display::Fullscreen => {
            if let Some(h) = height {
                let w = screen.map_or(h * 16 / 9, |sc| sc.width * h / sc.height.max(1));
                opt("resolution", format!("{w}x{h}"));
            }
        }
        Display::Window => {
            let h = height.unwrap_or(960);
            let w = match s.aspect {
                Aspect::Original => h * 4 / 3,
                Aspect::Stretch | Aspect::TrueWidescreen => {
                    let (rw, rh) = s.wide_target.ratio();
                    h * rw / rh
                }
            };
            opt("resolution", format!("{w}x{h}"));
        }
    }
    if s.aspect == Aspect::TrueWidescreen {
        opt("view", "Widescreen".into());
    }

    let flags: &[&str] = &[
        "noreadconfig",
        "skip_gameinfo",
        "joystick",
        "nomouse",
        "noconfirm_quit",
        if s.display == Display::Window {
            "window"
        } else {
            "nowindow"
        },
        if s.vsync { "waitvsync" } else { "nowaitvsync" },
        if s.aspect == Aspect::Stretch {
            "nokeepaspect"
        } else {
            "keepaspect"
        },
        if s.filter == crate::settings::Filter::Sharp {
            "nofilter"
        } else {
            "filter"
        },
    ];
    a.extend(flags.iter().map(|f| format!("-{f}")));
    a
}

/// The environment MAME needs on top of the launcher's.
pub fn env(s: &Settings, paths: &Paths) -> Vec<(String, String)> {
    let margin = match s.aspect {
        Aspect::TrueWidescreen => {
            let (w, h) = s.wide_target.ratio();
            wide_margin(w, h)
        }
        _ => 0,
    };
    vec![
        ("REA_BRIDGE".into(), path(&paths.bridge)),
        ("REA_S32_WIDE_MARGIN".into(), margin.to_string()),
        (
            "REA_RADM_TIME_SCALE".into(),
            s.race_time.percent().to_string(),
        ),
        // no MAME warning box waiting for a key before the game
        ("REA_SKIP_STARTUP_SCREENS".into(), "1".into()),
    ]
}

/// MAME's `-volume` is attenuation in dB (−32 … 0).
pub fn volume_db(percent: u8) -> i32 {
    if percent == 0 {
        return -32;
    }
    let db = 20.0 * (f64::from(percent.min(100)) / 100.0).log10();
    (db.round() as i32).clamp(-32, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::{Filter, Resolution, WideTarget};

    fn paths() -> Paths {
        Paths {
            exe: "/opt/rm/mame/radmobile".into(),
            mame_dir: "/opt/rm/mame".into(),
            roms: "/data/roms".into(),
            data: "/data".into(),
            bridge: "/data/bridge.shm".into(),
        }
    }

    fn value<'a>(args: &'a [String], key: &str) -> Option<&'a str> {
        args.iter()
            .position(|a| a == key)
            .map(|i| args[i + 1].as_str())
    }

    #[test]
    fn margins_for_common_screens() {
        assert_eq!(wide_margin(4, 3), 0);
        assert_eq!(wide_margin(16, 9), 52);
        assert_eq!(wide_margin(16, 10), 32);
        assert_eq!(wide_margin(21, 9), MAX_WIDE_MARGIN);
        assert!((wide_aspect(52) - 16.0 / 9.0).abs() < 0.02);
        assert!((wide_aspect(0) - 4.0 / 3.0).abs() < 1e-9);
        assert!(widescreen_layout(52).contains("<view name=\"Widescreen\">"));
    }

    #[test]
    fn default_command_line() {
        let a = args(&Settings::default(), "radm", &paths(), None);
        assert_eq!(a[0], "radm");
        assert_eq!(value(&a, "-joystickprovider"), Some("reabridge"));
        assert_eq!(value(&a, "-output"), Some("reabridge"));
        assert_eq!(value(&a, "-ctrlr"), Some("rea"));
        assert_eq!(value(&a, "-rompath"), Some("/data/roms"));
        assert_eq!(
            value(&a, "-artpath"),
            Some("/data/artwork;/opt/rm/mame/artwork")
        );
        assert_eq!(value(&a, "-bgfx_screen_chains"), Some("default"));
        assert!(a.contains(&"-nowindow".to_string()));
        assert!(a.contains(&"-noreadconfig".to_string()));
        assert_eq!(
            value(&a, "-resolution"),
            None,
            "fullscreen at the desktop size"
        );
        let env = env(&Settings::default(), &paths());
        assert!(env.contains(&("REA_S32_WIDE_MARGIN".into(), "0".into())));
        assert!(env.contains(&("REA_BRIDGE".into(), "/data/bridge.shm".into())));
        assert!(env.contains(&("REA_RADM_TIME_SCALE".into(), "100".into())));
    }

    #[test]
    fn a_longer_race_clock_reaches_mame() {
        let s = Settings {
            race_time: crate::settings::RaceTime::X2,
            ..Settings::default()
        };
        assert!(env(&s, &paths()).contains(&("REA_RADM_TIME_SCALE".into(), "200".into())));
    }

    #[test]
    fn widescreen_4k_crt() {
        let s = Settings {
            aspect: Aspect::TrueWidescreen,
            wide_target: WideTarget::W16x9,
            resolution: Resolution::P2160,
            filter: Filter::Crt,
            ..Settings::default()
        };
        let a = args(
            &s,
            "radm",
            &paths(),
            Some(Screen {
                width: 3840,
                height: 2160,
            }),
        );
        assert_eq!(value(&a, "-resolution"), Some("3840x2160"));
        assert_eq!(value(&a, "-view"), Some("Widescreen"));
        assert_eq!(value(&a, "-bgfx_screen_chains"), Some("crt-geom"));
        assert!(env(&s, &paths()).contains(&("REA_S32_WIDE_MARGIN".into(), "52".into())));
    }

    #[test]
    fn windowed_stretch() {
        let s = Settings {
            display: Display::Window,
            aspect: Aspect::Stretch,
            resolution: Resolution::P1080,
            vsync: false,
            ..Settings::default()
        };
        let a = args(&s, "radmu", &paths(), None);
        assert_eq!(value(&a, "-resolution"), Some("1920x1080"));
        for f in ["-window", "-nokeepaspect", "-nowaitvsync"] {
            assert!(a.contains(&f.to_string()), "{f}");
        }
    }

    #[test]
    fn volume_maps_to_attenuation() {
        assert_eq!(volume_db(100), 0);
        assert_eq!(volume_db(50), -6);
        assert_eq!(volume_db(0), -32);
        assert_eq!(volume_db(1), -32);
    }
}
