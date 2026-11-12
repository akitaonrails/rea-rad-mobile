//! Getting a game ready and running it: find the ROMs, rebuild the set,
//! write the EEPROM, the controller file and the widescreen layout, create
//! the bridge, and start MAME.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use radmobile_core::bridge::Bridge;
use radmobile_core::rom::{self, Inventory, RomSet};
use radmobile_core::settings::{Aspect, Display, Settings};
use radmobile_core::{art, controls, eeprom, mame};
use rea_launcher::fsx;

use crate::paths::AppPaths;

/// What the ROM check found, for the menu's status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RomStatus {
    Ready(RomSet),
    /// Some files are there, but not a whole set.
    Incomplete {
        set: RomSet,
        missing: Vec<&'static str>,
    },
    NotFound,
}

impl RomStatus {
    pub fn summary(&self) -> String {
        match self {
            RomStatus::Ready(set) => format!("ROM OK: {}", set.title),
            RomStatus::Incomplete { set, missing } => {
                format!(
                    "ROM INCOMPLETE: {} is missing {}",
                    set.name,
                    missing.join(", ")
                )
            }
            RomStatus::NotFound => "ROM NOT FOUND: put radm.zip in the roms folder".into(),
        }
    }
}

/// The folders searched for ROM zips.
pub fn rom_dirs(app: &AppPaths, settings: &Settings) -> Vec<PathBuf> {
    let mut dirs = vec![app.user_roms.clone()];
    dirs.extend(settings.rom_dir.clone());
    dirs
}

pub fn scan(app: &AppPaths, settings: &Settings) -> (Inventory, RomStatus) {
    let archives: Vec<PathBuf> = rom_dirs(app, settings)
        .iter()
        .flat_map(|d| rom::candidates(d))
        .collect();
    let inv = Inventory::scan(archives);
    let status = match inv.best(Some(&settings.region)) {
        Ok(set) => RomStatus::Ready(set),
        Err(m) if m.files.len() == m.set.files.iter().filter(|f| !f.optional).count() => {
            RomStatus::NotFound
        }
        Err(m) => RomStatus::Incomplete {
            set: m.set,
            missing: m.files.iter().map(|f| f.name).collect(),
        },
    };
    (inv, status)
}

/// Everything needed to start MAME.
pub struct Prepared {
    pub set: RomSet,
    pub exe: PathBuf,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub bridge: Bridge,
    pub log: PathBuf,
}

/// Write `data` to `path` through a temporary file and a rename
/// ([`fsx::write_atomic`]), so a crash never leaves half a file. Also
/// removes the `<name>.tmp` that 0.5.0 and older wrote through, should a
/// crash have left one.
pub fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    fsx::write_atomic(path, data)?;
    let _ = fsx::remove_files(&[&path.with_extension("tmp")]);
    Ok(())
}

/// The romset ready for MAME: found, and rebuilt under MAME 0.289's names
/// in the data folder unless it already is.
pub fn ready_set(app: &AppPaths, settings: &Settings) -> anyhow::Result<(RomSet, mame::Paths)> {
    let (inv, status) = scan(app, settings);
    let RomStatus::Ready(set) = status else {
        anyhow::bail!(status.summary())
    };
    let exe = app.mame_exe();
    anyhow::ensure!(exe.is_file(), "MAME is missing: {}", exe.display());

    let paths = mame::Paths {
        exe: exe.clone(),
        mame_dir: app.mame_dir.clone(),
        roms: app.data.join("roms"),
        data: app.data.clone(),
        bridge: app.data.join("bridge.shm"),
    };

    // the romset, rebuilt under MAME 0.289's names unless already there
    let built = paths.roms.join(format!("{}.zip", set.name));
    let current = built.is_file()
        && Inventory::scan_for([built.clone()], &[set])
            .missing(set)
            .is_empty();
    if !current {
        crate::log::line(&format!(
            "rom: building {} from {:?}",
            built.display(),
            inv.scanned
        ));
        rom::build_set(&inv, set, &paths.roms)?;
    }
    Ok((set, paths))
}

/// Operator settings (Upright cabinet, free play, …) in the EEPROM file.
pub fn write_eeprom(path: &Path, settings: &Settings) -> io::Result<()> {
    let existing = fs::read(path).ok();
    write_atomic(
        path,
        &eeprom::write_settings(existing.as_deref(), settings.game),
    )
}

pub fn prepare(
    app: &AppPaths,
    settings: &Settings,
    screen: Option<mame::Screen>,
) -> anyhow::Result<Prepared> {
    let (set, paths) = ready_set(app, settings)?;
    let exe = paths.exe.clone();

    write_eeprom(&paths.eeprom(set.name), settings)?;

    write_atomic(
        &paths.ctrlr().join("rea.cfg"),
        controls::ctrlr_cfg().as_bytes(),
    )?;

    write_layout(&paths.artwork().join(set.name), settings, screen)?;
    fs::create_dir_all(paths.snaps())?;

    let bridge = Bridge::create(&paths.bridge)?;
    Ok(Prepared {
        set,
        exe,
        args: mame::args(settings, set.name, &paths, screen),
        env: mame::env(settings, &paths),
        bridge,
        log: app.data.join("mame.log"),
    })
}

/// The bezel's height in pixels; MAME scales it to the screen.
const BEZEL_HEIGHT: u32 = 1080;

/// The artwork for the picture shape: the widescreen view, the cabinet
/// bezel around the 4:3 picture, or none (MAME's own 4:3 view).
fn write_layout(
    dir: &Path,
    settings: &Settings,
    screen: Option<mame::Screen>,
) -> anyhow::Result<()> {
    let layout = dir.join("default.lay");
    let bezel = dir.join(mame::BEZEL_FILE);
    if settings.aspect == Aspect::TrueWidescreen {
        let (w, h) = settings.wide_target.ratio();
        write_atomic(
            &layout,
            mame::widescreen_layout(mame::wide_margin(w, h)).as_bytes(),
        )?;
    } else if mame::shows_bezel(settings) {
        // the bezel takes the screen's shape (the window's, when windowed)
        let (rw, rh) = match screen {
            Some(s) if settings.display == Display::Fullscreen => (s.width, s.height.max(1)),
            _ => settings.wide_target.ratio(),
        };
        let width = (BEZEL_HEIGHT * rw / rh).max(BEZEL_HEIGHT * 4 / 3);
        let (image, rect) = art::bezel(width, BEZEL_HEIGHT);
        crate::art_io::save_png(&bezel, &image)?;
        write_atomic(
            &layout,
            mame::cabinet_layout(width, BEZEL_HEIGHT, rect).as_bytes(),
        )?;
        return Ok(());
    } else if layout.exists() {
        fs::remove_file(&layout)?;
    }
    if bezel.exists() {
        fs::remove_file(&bezel)?;
    }
    Ok(())
}

/// Start MAME with its output in `mame.log`.
pub fn launch(p: &Prepared) -> io::Result<Child> {
    crate::log::line(&format!("mame: {} {}", p.exe.display(), p.args.join(" ")));
    let log = fs::File::create(&p.log)?;
    let mut cmd = Command::new(&p.exe);
    cmd.args(&p.args)
        .envs(p.env.iter().map(|(k, v)| (k, v)))
        .current_dir(p.exe.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(log.try_clone()?)
        .stderr(log);
    rea_launcher_native::child::spawn_tied(&mut cmd)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(dir: &Path) -> AppPaths {
        AppPaths {
            exe_dir: dir.to_path_buf(),
            mame_dir: dir.join("mame"),
            data: dir.join("data"),
            user_roms: dir.join("roms"),
        }
    }

    #[test]
    fn no_roms_is_reported_as_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (_, status) = scan(&app(dir.path()), &Settings::default());
        assert_eq!(status, RomStatus::NotFound);
        assert!(status.summary().starts_with("ROM NOT FOUND"));
        let err = prepare(&app(dir.path()), &Settings::default(), None)
            .err()
            .unwrap();
        assert!(err.to_string().starts_with("ROM NOT FOUND"));
    }

    #[test]
    fn the_layout_follows_the_picture_shape() {
        let dir = tempfile::tempdir().unwrap();
        let art = dir.path().join("artwork/radm");
        let mut s = Settings {
            bezel: true,
            ..Settings::default()
        };
        write_layout(&art, &s, None).unwrap();
        let lay = fs::read_to_string(art.join("default.lay")).unwrap();
        assert!(lay.contains(mame::CABINET_VIEW));
        assert!(art.join(mame::BEZEL_FILE).is_file());

        s.aspect = Aspect::TrueWidescreen;
        write_layout(&art, &s, None).unwrap();
        let lay = fs::read_to_string(art.join("default.lay")).unwrap();
        assert!(lay.contains("Widescreen"));
        assert!(!art.join(mame::BEZEL_FILE).exists());

        s.aspect = Aspect::Original;
        s.bezel = false;
        write_layout(&art, &s, None).unwrap();
        assert!(!art.join("default.lay").exists());
    }

    #[test]
    fn atomic_writes_create_folders() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/b/c.bin");
        write_atomic(&p, b"one").unwrap();
        // 0.5.0's temporary file, left by a crash, goes with the next write
        fs::write(p.with_extension("tmp"), b"half").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert!(!p.with_extension("tmp").exists());
        let left: Vec<_> = fs::read_dir(p.parent().unwrap())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(left, ["c.bin"]);
    }
}
