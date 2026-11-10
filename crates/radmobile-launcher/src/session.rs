//! Getting a game ready and running it: find the ROMs, rebuild the set,
//! write the EEPROM, the controller file and the widescreen layout, create
//! the bridge, and start MAME.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use radmobile_core::bridge::Bridge;
use radmobile_core::rom::{self, Inventory, RomSet};
use radmobile_core::settings::{Aspect, Settings};
use radmobile_core::{controls, eeprom, mame};

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

/// Write `data` to `path` through a temporary file and a rename, so a
/// crash never leaves half a file.
fn write_atomic(path: &Path, data: &[u8]) -> io::Result<()> {
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, data)?;
    fs::rename(&tmp, path)
}

pub fn prepare(
    app: &AppPaths,
    settings: &Settings,
    screen: Option<mame::Screen>,
) -> anyhow::Result<Prepared> {
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

    // operator settings in the EEPROM (Upright cabinet, free play, …)
    let eeprom_path = paths.eeprom(set.name);
    let existing = fs::read(&eeprom_path).ok();
    write_atomic(
        &eeprom_path,
        &eeprom::write_settings(existing.as_deref(), settings.game),
    )?;

    write_atomic(
        &paths.ctrlr().join("rea.cfg"),
        controls::ctrlr_cfg().as_bytes(),
    )?;

    let layout = paths.artwork().join(set.name).join("default.lay");
    if settings.aspect == Aspect::TrueWidescreen {
        let (w, h) = settings.wide_target.ratio();
        write_atomic(
            &layout,
            mame::widescreen_layout(mame::wide_margin(w, h)).as_bytes(),
        )?;
    } else if layout.exists() {
        fs::remove_file(&layout)?;
    }
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
    cmd.spawn()
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
    fn atomic_writes_create_folders() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("a/b/c.bin");
        write_atomic(&p, b"one").unwrap();
        write_atomic(&p, b"two").unwrap();
        assert_eq!(fs::read(&p).unwrap(), b"two");
        assert!(!p.with_extension("tmp").exists());
    }
}
