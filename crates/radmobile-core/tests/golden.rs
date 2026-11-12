//! Golden snapshots of everything the launcher writes or hands to MAME,
//! taken before the move onto rea-launcher-sdk (docs: the SDK study's
//! "Step 0"). Every later refactor step must leave these unchanged, or
//! change them on purpose and say so in the commit.
//!
//! The snapshots live in `tests/snapshots/`; `REA_UPDATE_SNAPSHOTS=1 cargo
//! test` rewrites them (review the diff like code). All data is synthetic:
//! no ROM bytes, no real player files.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use radmobile_core::controls::{self, Action, Button};
use radmobile_core::eeprom::{self, GameSettings};
use radmobile_core::ffb::FfbSettings;
use radmobile_core::mame::{self, Paths, Screen};
use radmobile_core::pad_map::{PadInput, PadMap};
use radmobile_core::settings::{
    Aspect, Display, Filter, InputMode, Preset, RaceTime, Resolution, Settings, WideTarget,
};
use radmobile_core::wheel::{
    self, DeviceId, DeviceState, PadSettings, PedalAxis, SteeringLock, WheelProfile, WheelSettings,
    wizard,
};
use rea_launcher::snapshot;

/// MAME paths are built with `Path::join`; on Windows that puts `\`
/// between our synthetic `/`-rooted parts. The snapshots use `/`.
fn slashes(s: String) -> String {
    if cfg!(windows) {
        s.replace('\\', "/")
    } else {
        s
    }
}

// ---------------------------------------------------------------- settings.ini

fn every_field_changed() -> Settings {
    Settings {
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
            rival_speed: 3,
            police_speed: 14,
            attract_sound: true,
            free_play: false,
            credits_to_start: 4,
            credits_to_continue: 2,
        },
        race_time: RaceTime::X1_5,
        input: InputMode::Wheel,
        wheel: WheelSettings {
            lock: SteeringLock::Full,
            invert_accel: true,
            invert_brake: true,
            device: r"\\?\hid#vid_0eb7&pid_0020&col01#8&1d2f&0&0000#{4d1e55b2-f16f-11cf-88cb-001111000030}".into(),
        },
        pad: PadSettings {
            stick_deadzone: 4500,
            stick_linearity: 40,
            trigger_deadzone: 12,
            trigger_linearity: 25,
            steer_assist: false,
            map: {
                let mut map = PadMap::default();
                map.assign(Action::Press(Button::Pause), PadInput::RightStick);
                map.assign(Action::Accelerate, PadInput::RightShoulder);
                map
            },
        },
        ffb: FfbSettings {
            strength: 75,
            invert: true,
            rumble: 0,
        },
        rom_dir: Some(PathBuf::from("/games/arcade roms")),
    }
}

#[test]
fn settings_ini_from_the_defaults() {
    snapshot!("settings-default.ini", Settings::default().to_cfg(""));
}

#[test]
fn settings_ini_from_each_preset() {
    for preset in Preset::ALL {
        let mut s = Settings::default();
        preset.apply(&mut s);
        let name = format!("settings-preset-{}.ini", preset.label().to_lowercase());
        snapshot!(&name, s.to_cfg(""));
        // what was written reads back as the same preset
        let back = Settings::from_cfg(&s.to_cfg(""));
        assert_eq!(Preset::of(&back), Some(preset));
    }
}

#[test]
fn settings_ini_with_every_field_changed() {
    let s = every_field_changed();
    let text = s.to_cfg("");
    snapshot!("settings-every-field.ini", text);
    assert_eq!(Settings::from_cfg(&text), s);
}

/// A file as an older launcher, or a player in Notepad, may leave it:
/// `key=value` with `1`/`0`, keys in any case, a `[section]` header, CRLF
/// line ends, blank values, junk lines and spaces around `=`.
const OLD_FORMAT: &str = "# settings.ini - Rad Mobile launcher\r\n\
[launcher]\r\n\
display=window\r\n\
Resolution=1440p\r\n\
ASPECT = Widescreen\r\n\
wide_target=16:10\r\n\
filter=TRINITRON_CRT\r\n\
crt_curvature=0\r\n\
bezel=1\r\n\
VSync=0\r\n\
brightness=80\r\n\
volume=200\r\n\
region=radmu\r\n\
Difficulty=3\r\n\
timer=50\r\n\
rival_speed=0\r\n\
police_speed=5\r\n\
race_time=2X\r\n\
attract_sound=yes\r\n\
free_play=0\r\n\
credits_start=3\r\n\
credits_continue=9\r\n\
input_device=Controller\r\n\
steer_lock=500\r\n\
invert_accel=1\r\n\
invert_brake=off\r\n\
wheel_device=\r\n\
stick_deadzone=2500\r\n\
stick_linearity=10\r\n\
trigger_deadzone=4\r\n\
trigger_linearity=0\r\n\
steer_assist=0\r\n\
ffb_strength=60\r\n\
ffb_invert=true\r\n\
rumble_strength=30\r\n\
; a player's note\r\n\
this line is junk\r\n\
rom_dir=D:\\Arcade\\ROMs\r\n";

fn old_format_expected() -> Settings {
    let d = Settings::default();
    Settings {
        display: Display::Window,
        resolution: Resolution::P1440,
        aspect: Aspect::TrueWidescreen,
        wide_target: WideTarget::W16x10,
        filter: Filter::TrinitronCrt,
        crt_curvature: false,
        bezel: true,
        vsync: false,
        brightness: 80,
        volume: 100, // 200 clamps
        region: "radmu".into(),
        game: GameSettings {
            difficulty: 3,
            timer: 50,
            rival_speed: 1, // 0 clamps
            police_speed: 5,
            attract_sound: true,
            free_play: false,
            credits_to_start: 3,
            credits_to_continue: 5, // 9 clamps
        },
        race_time: RaceTime::X2,
        input: InputMode::Controller,
        wheel: WheelSettings {
            lock: SteeringLock::Degrees(540), // 500 snaps to the nearest step
            invert_accel: true,
            invert_brake: false,
            device: String::new(),
        },
        pad: PadSettings {
            stick_deadzone: 2500,
            stick_linearity: 10,
            trigger_deadzone: 4,
            trigger_linearity: 0,
            steer_assist: false,
            map: d.pad.map,
        },
        ffb: FfbSettings {
            strength: 60,
            invert: true,
            rumble: 30,
        },
        rom_dir: Some(PathBuf::from(r"D:\Arcade\ROMs")),
    }
}

#[test]
fn an_old_format_settings_file_reads_every_value() {
    assert_eq!(Settings::from_cfg(OLD_FORMAT), old_format_expected());
}

/// Saving over a 0.5.0 file: the launcher's lines become `key = value`
/// with `on`/`off` (step 4 of the SDK move, on purpose), in place; the
/// player's comments, the `[section]`, the junk line and CRLF stay, and a
/// key in another case (`Resolution=`) is written in lower case where it
/// was. The keys the old file lacked (the pad map) are added at the end.
#[test]
fn saving_over_an_old_format_file() {
    let text = old_format_expected().to_cfg(OLD_FORMAT);
    snapshot!("settings-saved-over-old-format.ini", text);
    // (snapshots compare with LF line ends)
    assert_eq!(text.matches("\r\n").count(), text.lines().count());
    assert_eq!(Settings::from_cfg(&text), old_format_expected());
    // saving again changes nothing
    assert_eq!(old_format_expected().to_cfg(&text), text);
}

/// 0.5.0 read a byte-order mark as part of the first key, so a Notepad
/// "UTF-8 with BOM" file lost its first setting. `cfg` looks past the
/// mark (on purpose since step 4), and keeps it when saving.
#[test]
fn a_byte_order_mark_is_looked_past_and_kept() {
    let s = Settings::from_cfg("\u{feff}display=window\r\nbezel=1\r\n");
    assert_eq!(s.display, Display::Window);
    assert!(s.bezel);
    assert!(
        s.to_cfg("\u{feff}display=window\r\n")
            .starts_with("\u{feff}display = window\r\n")
    );

    let with_bom = format!("\u{feff}{OLD_FORMAT}");
    assert_eq!(Settings::from_cfg(&with_bom), old_format_expected());
}

/// 0.5.0 matched the region case-sensitively (`RADMU` read as `radm`);
/// since step 4 it takes any case, like every other value.
#[test]
fn the_region_is_read_in_any_case() {
    assert_eq!(Settings::from_cfg("region=RADMU").region, "radmu");
    assert_eq!(Settings::from_cfg("region=Radm").region, "radm");
}

// ------------------------------------------------------------- MAME command

fn paths() -> Paths {
    Paths {
        exe: "/opt/rm/mame/radmobile".into(),
        mame_dir: "/opt/rm/mame".into(),
        roms: "/data/roms".into(),
        data: "/data".into(),
        bridge: "/data/bridge.shm".into(),
    }
}

/// The command line one argument per line, then the environment.
fn command(s: &Settings, set: &str, screen: Option<Screen>) -> String {
    let mut out = String::new();
    for a in mame::args(s, set, &paths(), screen) {
        writeln!(out, "{a}").unwrap();
    }
    out.push_str("--- env\n");
    for (k, v) in mame::env(s, &paths()) {
        writeln!(out, "{k}={v}").unwrap();
    }
    slashes(out)
}

#[test]
fn mame_command_default() {
    snapshot!(
        "mame-default.txt",
        command(&Settings::default(), "radm", None)
    );
}

#[test]
fn mame_command_widescreen_4k_arcade_crt() {
    let s = Settings {
        aspect: Aspect::TrueWidescreen,
        wide_target: WideTarget::W16x9,
        resolution: Resolution::P2160,
        filter: Filter::ArcadeCrt,
        ..Settings::default()
    };
    let screen = Screen {
        width: 3840,
        height: 2160,
    };
    snapshot!(
        "mame-widescreen-4k-arcade-crt.txt",
        command(&s, "radm", Some(screen))
    );
}

#[test]
fn mame_command_windowed_stretch() {
    let s = Settings {
        display: Display::Window,
        aspect: Aspect::Stretch,
        resolution: Resolution::P1080,
        vsync: false,
        ..Settings::default()
    };
    snapshot!("mame-windowed-stretch.txt", command(&s, "radmu", None));
}

#[test]
fn mame_command_bezel() {
    let s = Settings {
        display: Display::Window,
        resolution: Resolution::P1080,
        bezel: true,
        filter: Filter::TrinitronCrt,
        crt_curvature: false,
        brightness: 120,
        volume: 50,
        ..Settings::default()
    };
    snapshot!("mame-bezel.txt", command(&s, "radm", None));
}

#[test]
fn mame_command_3x_clock() {
    let s = Settings {
        race_time: RaceTime::X3,
        resolution: Resolution::P1440,
        ..Settings::default()
    };
    let screen = Screen {
        width: 3440,
        height: 1440,
    };
    snapshot!("mame-clock-3x.txt", command(&s, "radm", Some(screen)));
}

#[test]
fn mame_command_sharp_21_9_without_a_known_screen() {
    let s = Settings {
        aspect: Aspect::TrueWidescreen,
        wide_target: WideTarget::W21x9,
        resolution: Resolution::P1080,
        filter: Filter::Sharp,
        volume: 0,
        ..Settings::default()
    };
    snapshot!("mame-sharp-21x9.txt", command(&s, "radmu", None));
}

#[test]
fn mame_controller_file() {
    snapshot!("ctrlr-rea.cfg", controls::ctrlr_cfg());
}

/// `Button` order is the bridge's bit order and MAME's
/// `JOYCODE_1_BUTTONn`; `Action` order is the menu's and the pad map's.
#[test]
fn control_tables() {
    let mut out = String::from("button bit key mame_type mame_key radm_field label\n");
    for b in Button::ALL {
        writeln!(
            out,
            "{b:?} {:#04x} {} {} {} {:?} {:?}",
            b.bit(),
            b.key(),
            b.mame_type(),
            b.mame_key(),
            b.radm_field(),
            b.label()
        )
        .unwrap();
    }
    out.push_str("\naction key label\n");
    for a in Action::ALL {
        writeln!(out, "{a:?} {} {:?}", a.key(), a.label()).unwrap();
    }
    snapshot!("controls.txt", out);
}

// ---------------------------------------------------------- layouts, backdrop

#[test]
fn widescreen_layouts() {
    let mut out = String::new();
    for target in WideTarget::ALL {
        let (w, h) = target.ratio();
        let margin = mame::wide_margin(w, h);
        writeln!(out, "=== {} margin {margin}", target.label()).unwrap();
        out.push_str(&mame::widescreen_layout(margin));
    }
    snapshot!("layout-widescreen.lay", out);
}

/// The sizes `session::write_layout` uses: 1080 high, as wide as the
/// screen's shape (never narrower than 4:3).
#[test]
fn cabinet_layouts() {
    let mut out = String::new();
    for width in [1920, 1728, 2520, 1440] {
        let (_, rect) = radmobile_core::art::bezel(width, 1080);
        writeln!(out, "=== {width}x1080").unwrap();
        out.push_str(&mame::cabinet_layout(width, 1080, rect));
    }
    snapshot!("layout-cabinet.lay", out);
}

#[test]
fn backdrop_script_and_arguments() {
    let scratch = PathBuf::from("/data/menu/capture");
    let args = mame::backdrop_args(
        "radm",
        Path::new("/data/roms"),
        &scratch,
        &scratch.join("backdrop.lua"),
    );
    let mut out = mame::backdrop_script();
    out.push_str("--- args\n");
    for a in args {
        writeln!(out, "{a}").unwrap();
    }
    snapshot!("backdrop.txt", slashes(out));
}

// ------------------------------------------------------------------ wheel.ini

fn csl_dd() -> DeviceId {
    DeviceId {
        name: "FANATEC CSL DD".into(),
        vid: 0x0eb7,
        pid: 0x0020,
        guid: "03000000b70e00002000000000000000".into(),
    }
}

/// The wheel at rest as SDL reports it: steering near the middle, two
/// pedals resting at opposite ends, a third axis at the middle.
fn at_rest() -> DeviceState {
    DeviceState {
        axes: vec![-130, 32_767, -32_768, 0, -32_768],
        buttons: vec![false; 32],
    }
}

fn wheel_ini(p: &WheelProfile) -> String {
    let text = p.to_cfg("");
    // what was written reads back the same, and saving again changes nothing
    assert_eq!(WheelProfile::from_cfg(&text).as_ref(), Some(p));
    assert_eq!(p.to_cfg(&text), text);
    text
}

#[test]
fn wheel_ini_for_a_guessed_csl_dd() {
    let p = WheelProfile::guess(csl_dd(), &at_rest());
    snapshot!("wheel-csl-dd-guess.ini", wheel_ini(&p));
}

#[test]
fn wheel_ini_for_a_guessed_unknown_wheel() {
    let unknown = DeviceId {
        name: "Some Wheel".into(),
        vid: 0x1234,
        pid: 0xabcd,
        guid: String::new(),
    };
    let p = WheelProfile::guess(unknown, &DeviceState::default());
    snapshot!("wheel-unknown-guess.ini", wheel_ini(&p));
}

#[test]
fn wheel_ini_after_the_wizard_with_aux_pedals() {
    let pedals = DeviceId {
        name: "FANATEC CSL Pedals".into(),
        vid: 0x0eb7,
        pid: 0x6204,
        guid: "03000000b70e00000462000000000000".into(),
    };
    let button_box = DeviceId {
        name: "Button Box".into(),
        vid: 0x16c0,
        pid: 0x05e1,
        guid: "bb".into(),
    };
    let mut p = WheelProfile::guess(csl_dd(), &at_rest());
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
                axis: 2,
                rest: 32_767,
                peak: -30_500,
            },
        )),
        buttons: vec![
            (Button::Light, 0, 4),
            (Button::Wiper, 0, 5),
            (Button::Start, 0, 9),
            (Button::Coin, 1, 0),
            // a third device is not kept
            (Button::Pause, 2, 3),
            (Button::Exit, 0, 11),
        ],
        skipped: vec![wizard::Step::Button(Button::Service)],
    };
    p.apply(&captures, &[csl_dd(), pedals, button_box]);
    snapshot!("wheel-csl-dd-wizard-aux.ini", wheel_ini(&p));
    assert_eq!(p.describe(Action::Accelerate), "AUX AXIS 1");
    assert_eq!(p.describe(Action::Press(Button::Pause)), "-");
}

#[test]
fn known_wheels_table() {
    let mut out = String::new();
    for k in wheel::known::KNOWN {
        writeln!(
            out,
            "{:04x}:{:04x} {:?} rotation {} paddles {:?}",
            k.vid, k.pid, k.name, k.rotation, k.paddles
        )
        .unwrap();
    }
    writeln!(
        out,
        "generic rotation {} paddles {:?}",
        wheel::known::GENERIC_ROTATION,
        wheel::known::GENERIC_PADDLES
    )
    .unwrap();
    snapshot!("known-wheels.txt", out);
}

// --------------------------------------------------------------------- EEPROM

fn hex(bytes: &[u8]) -> String {
    let mut out = String::new();
    for (i, row) in bytes.chunks(16).enumerate() {
        let cells: Vec<String> = row.iter().map(|b| format!("{b:02x}")).collect();
        writeln!(out, "{:04x}: {}", i * 16, cells.join(" ")).unwrap();
    }
    out
}

#[test]
fn eeprom_default() {
    let image = eeprom::write_settings(None, GameSettings::default());
    assert!(eeprom::is_valid(&image));
    snapshot!("eeprom-default.hex", hex(&image));
}

#[test]
fn eeprom_easy_preset() {
    let mut s = Settings::default();
    Preset::Easy.apply(&mut s);
    let image = eeprom::write_settings(None, s.game);
    snapshot!("eeprom-easy.hex", hex(&image));
}

#[test]
fn eeprom_out_of_range() {
    let wild = GameSettings {
        difficulty: 0,
        timer: 99,
        rival_speed: 200,
        police_speed: 0,
        attract_sound: true,
        free_play: false,
        credits_to_start: 0,
        credits_to_continue: 9,
    };
    let image = eeprom::write_settings(None, wild);
    snapshot!("eeprom-out-of-range.hex", hex(&image));
    assert_eq!(image, eeprom::write_settings(None, wild.clamped()));
}

/// A valid image keeps its bookkeeping (here a made-up byte at 0x30 of
/// the first copy); a corrupt one is replaced by the factory image.
#[test]
fn eeprom_over_an_existing_image() {
    let mut existing = eeprom::write_settings(None, GameSettings::default());
    existing[0x30] = 0x5a;
    let crc = eeprom::crc16(&existing[10..62]).to_le_bytes();
    existing[8..10].copy_from_slice(&crc);
    let mut easy = Settings::default();
    Preset::Easy.apply(&mut easy);
    let kept = eeprom::write_settings(Some(&existing), easy.game);
    snapshot!("eeprom-easy-over-existing.hex", hex(&kept));

    let mut corrupt = existing;
    corrupt[0] = b'X';
    corrupt[64] = b'X';
    assert_eq!(
        eeprom::write_settings(Some(&corrupt), easy.game),
        eeprom::write_settings(None, easy.game)
    );
}

#[test]
fn steering_lock_steps() {
    let mut out = String::new();
    for lock in SteeringLock::STEPS {
        writeln!(out, "{} = {}", lock.label(), lock.to_setting()).unwrap();
    }
    for v in [-1, 0, 1, 200, 225, 226, 500, 720, 1000, 3600] {
        writeln!(out, "from {v} -> {}", SteeringLock::from_setting(v).label()).unwrap();
    }
    snapshot!("steering-lock.txt", out);
}
