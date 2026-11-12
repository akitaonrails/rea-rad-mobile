//! Golden tests of the launcher binary's own surface, taken before the
//! move onto rea-launcher-sdk (the SDK study's "Step 0"): the help text and
//! how `settings.ini` and `wheel.ini` files are read from disk. The pure
//! outputs (settings.ini, MAME command, wheel.ini, EEPROM…) are pinned in
//! `radmobile-core/tests/golden.rs`.

use std::path::{Path, PathBuf};

use radmobile_core::settings::{Aspect, Display, Filter, Resolution, Settings};
use radmobile_core::wheel::{Binding, PedalAxis, Source, WheelProfile};
use rea_launcher::snapshot;

use crate::paths::AppPaths;

#[test]
fn help_text() {
    snapshot!("help.txt", crate::USAGE);
}

fn app(dir: &Path) -> AppPaths {
    AppPaths {
        exe_dir: dir.to_path_buf(),
        mame_dir: dir.join("mame"),
        data: dir.to_path_buf(),
        user_roms: dir.join("roms"),
    }
}

/// An old `settings.ini` with everything a hand edit or an older launcher
/// leaves: a byte-order mark, a `[section]`, CRLF, keys in any case,
/// `1`/`0` and words for switches, and an accented `rom_dir`.
fn old_settings(rom_dir: &[u8]) -> Vec<u8> {
    let mut text = b"\xef\xbb\xbf# settings.ini - Rad Mobile launcher\r\n\
[launcher]\r\n\
Display=window\r\n\
RESOLUTION=1080p\r\n\
aspect=widescreen\r\n\
filter=arcade_crt\r\n\
bezel=1\r\n\
VSYNC=off\r\n\
Difficulty=3\r\n\
free_play=0\r\n\
ffb_strength=80\r\n\
rom_dir="
        .to_vec();
    text.extend_from_slice(rom_dir);
    text.extend_from_slice(b"\r\n");
    text
}

fn expected_old_settings(rom_dir: &str) -> Settings {
    let mut s = Settings {
        display: Display::Window,
        resolution: Resolution::P1080,
        aspect: Aspect::TrueWidescreen,
        filter: Filter::ArcadeCrt,
        bezel: true,
        vsync: false,
        rom_dir: Some(PathBuf::from(rom_dir)),
        ..Settings::default()
    };
    s.game.difficulty = 3;
    s.game.free_play = false;
    s.ffb.strength = 80;
    s
}

#[test]
fn an_old_utf8_settings_file_loads_every_value() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.ini"),
        old_settings("D:\\Jogos\\Ação\\roms".as_bytes()),
    )
    .unwrap();
    let (settings, profile) = crate::ui::load(&app(dir.path()));
    assert_eq!(settings, expected_old_settings("D:\\Jogos\\Ação\\roms"));
    assert_eq!(profile, None);
}

/// Pinned as it is today, and it is a bug (SDK study §5.8): a file saved
/// as Latin-1 (Notepad "ANSI") is not UTF-8, the read fails, and every
/// setting falls back to its default. Step 4 (`cfg::read_file`) fixes this
/// on purpose; update the expectation then.
#[test]
fn an_old_latin1_settings_file_loads_as_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("settings.ini"),
        old_settings(b"D:\\Jogos\\A\xe7\xe3o\\roms"),
    )
    .unwrap();
    let (settings, _) = crate::ui::load(&app(dir.path()));
    assert_eq!(settings, Settings::default());
}

#[test]
fn no_files_load_as_the_defaults() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(
        crate::ui::load(&app(dir.path())),
        (Settings::default(), None)
    );
}

#[test]
fn an_old_wheel_ini_loads() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("wheel.ini"),
        "# wheel.ini\r\n[wheel]\r\nWHEEL_NAME=FANATEC CSL DD\r\nwheel_vid=0EB7\r\n\
         wheel_pid=0020\r\nRotation=5000\r\ngas_axis=1\r\ngas_rest=32767\r\n\
         gas_peak=-32768\r\nbrake_device=aux\r\nbrake_axis=2\r\nbrake_rest=-32768\r\n\
         brake_peak=32767\r\nBTN_LIGHT=wheel:4\r\nbtn_coin=aux:1\r\nbtn_start=pedal:3\r\n",
    )
    .unwrap();
    let (_, profile) = crate::ui::load(&app(dir.path()));
    let p: WheelProfile = profile.expect("a profile");
    assert_eq!(p.wheel.name, "FANATEC CSL DD");
    assert_eq!((p.wheel.vid, p.wheel.pid), (0x0eb7, 0x0020));
    assert_eq!(p.wheel.guid, "");
    assert_eq!(p.aux, None, "no aux_vid: no aux device");
    assert_eq!(p.steer_axis, 0);
    assert_eq!(p.rotation, 3600, "clamped");
    assert_eq!(
        p.gas,
        Some((
            Source::Wheel,
            PedalAxis {
                axis: 1,
                rest: 32767,
                peak: -32768
            }
        ))
    );
    assert_eq!(
        p.brake,
        Some((
            Source::Aux,
            PedalAxis {
                axis: 2,
                rest: -32768,
                peak: 32767
            }
        ))
    );
    use radmobile_core::controls::Button;
    assert_eq!(
        p.buttons,
        vec![
            (
                Button::Light,
                Binding {
                    source: Source::Wheel,
                    button: 4
                }
            ),
            (
                Button::Coin,
                Binding {
                    source: Source::Aux,
                    button: 1
                }
            ),
        ],
        "an unknown source is dropped"
    );
}
