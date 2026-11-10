//! Where the launcher finds MAME and keeps its data.
//!
//! Portable first, like Wanszai's builds: `mame/`, `roms/` and `data/`
//! sit next to the executable. When that folder is read-only the data goes
//! to the user's data folder instead.

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub exe_dir: PathBuf,
    /// Holds the `radmobile` executable and `bgfx/`.
    pub mame_dir: PathBuf,
    /// Settings, logs, MAME's cfg/nvram, the rebuilt romset.
    pub data: PathBuf,
    /// Where players drop their ROM zips.
    pub user_roms: PathBuf,
}

pub const MAME_EXE: &str = if cfg!(windows) {
    "radmobile.exe"
} else {
    "radmobile"
};

impl AppPaths {
    pub fn detect(data_override: Option<PathBuf>) -> Self {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("."));
        let mame_dir = std::env::var_os("RADMOBILE_MAME_DIR")
            .map(PathBuf::from)
            .or_else(|| Some(exe_dir.join("mame")).filter(|d| d.join(MAME_EXE).is_file()))
            .or_else(|| dev_mame_dir(&exe_dir))
            .unwrap_or_else(|| exe_dir.join("mame"));
        let data = data_override.unwrap_or_else(|| {
            let portable = exe_dir.join("data");
            if is_writable(&portable) {
                portable
            } else {
                user_data_dir()
            }
        });
        // MAME runs from its own folder, so every path it gets is absolute
        let data = std::path::absolute(&data).unwrap_or(data);
        AppPaths {
            user_roms: exe_dir.join("roms"),
            exe_dir,
            mame_dir,
            data,
        }
    }

    pub fn mame_exe(&self) -> PathBuf {
        self.mame_dir.join(MAME_EXE)
    }
    pub fn settings(&self) -> PathBuf {
        self.data.join("settings.ini")
    }
    pub fn wheel_profile(&self) -> PathBuf {
        self.data.join("wheel.ini")
    }
    pub fn log(&self) -> PathBuf {
        self.data.join("launcher.log")
    }
}

/// A `cargo run` from the repository uses `dist/mame-<os>/`.
fn dev_mame_dir(exe_dir: &Path) -> Option<PathBuf> {
    let os = if cfg!(windows) {
        "mame-windows"
    } else {
        "mame-linux"
    };
    exe_dir
        .ancestors()
        .map(|a| a.join("dist").join(os))
        .find(|d| d.join(MAME_EXE).is_file())
}

fn is_writable(dir: &Path) -> bool {
    if std::fs::create_dir_all(dir).is_err() {
        return false;
    }
    let probe = dir.join(".write-test");
    let ok = std::fs::write(&probe, b"").is_ok();
    let _ = std::fs::remove_file(&probe);
    ok
}

fn user_data_dir() -> PathBuf {
    if cfg!(windows) {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir)
            .join("RadMobile")
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/share")))
            .unwrap_or_else(std::env::temp_dir)
            .join("rad-mobile")
    }
}
