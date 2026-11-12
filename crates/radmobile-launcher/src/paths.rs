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

#[cfg(test)]
mod tests {
    //! The data folder's fallback order, pinned before the move onto
    //! rea-launcher-sdk. The order reads the process environment and the
    //! executable's folder, so each case runs this test binary again as a
    //! child with the environment (and, for the read-only case, a copy of
    //! the binary) set up for it; nothing here changes this process.

    use std::path::{Path, PathBuf};
    use std::process::Command;

    use super::*;

    const PROBE: &str = "RADM_TEST_PATHS_PROBE";
    const PROBE_TEST: &str = "paths::tests::probe";

    /// Not a check of its own: the child side of [`run_probe`].
    #[test]
    fn probe() {
        let Some(mode) = std::env::var_os(PROBE) else {
            return;
        };
        // a line of its own: libtest prints the test's name before it
        println!("\nuser={}", user_data_dir().display());
        println!("temp={}", std::env::temp_dir().display());
        if mode == "detect" {
            let paths = AppPaths::detect(None);
            println!("data={}", paths.data.display());
            println!("exe_dir={}", paths.exe_dir.display());
            println!("user_roms={}", paths.user_roms.display());
        }
    }

    struct Probe {
        user: PathBuf,
        temp: PathBuf,
        data: Option<PathBuf>,
        exe_dir: Option<PathBuf>,
        user_roms: Option<PathBuf>,
    }

    /// Run [`probe`] in `exe` with only the given data-folder variables.
    fn run_probe(exe: &Path, mode: &str, env: &[(&str, &str)]) -> Probe {
        let mut cmd = Command::new(exe);
        cmd.args(["--exact", PROBE_TEST, "--nocapture", "--test-threads=1"])
            .env(PROBE, mode);
        for var in ["XDG_DATA_HOME", "HOME", "LOCALAPPDATA"] {
            cmd.env_remove(var);
        }
        cmd.envs(env.iter().copied());
        let out = cmd.output().unwrap();
        let stdout = String::from_utf8_lossy(&out.stdout);
        assert!(
            out.status.success(),
            "{stdout}\n{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let field = |key: &str| {
            stdout
                .lines()
                .find_map(|l| l.strip_prefix(&format!("{key}=")))
                .map(PathBuf::from)
        };
        Probe {
            user: field("user").expect("probe output"),
            temp: field("temp").expect("probe output"),
            data: field("data"),
            exe_dir: field("exe_dir"),
            user_roms: field("user_roms"),
        }
    }

    fn user_dir(env: &[(&str, &str)]) -> Probe {
        run_probe(&std::env::current_exe().unwrap(), "user", env)
    }

    #[cfg(unix)]
    #[test]
    fn the_user_data_folder_falls_back_xdg_then_home_then_temp() {
        assert_eq!(
            user_dir(&[("XDG_DATA_HOME", "/xdg"), ("HOME", "/home/p")]).user,
            Path::new("/xdg/rad-mobile")
        );
        assert_eq!(
            user_dir(&[("HOME", "/home/p")]).user,
            Path::new("/home/p/.local/share/rad-mobile")
        );
        let p = user_dir(&[("TMPDIR", "/tmp-radm")]);
        assert_eq!(p.temp, Path::new("/tmp-radm"));
        assert_eq!(p.user, Path::new("/tmp-radm/rad-mobile"));
        // pinned as it is: an empty XDG_DATA_HOME counts as set (the XDG
        // spec says to ignore it), giving a relative folder that
        // `AppPaths::detect` then makes absolute against the working folder
        assert_eq!(
            user_dir(&[("XDG_DATA_HOME", ""), ("HOME", "/home/p")]).user,
            Path::new("rad-mobile")
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_user_data_folder_falls_back_localappdata_then_temp() {
        assert_eq!(
            user_dir(&[("LOCALAPPDATA", r"C:\Users\p\AppData\Local")]).user,
            Path::new(r"C:\Users\p\AppData\Local\RadMobile")
        );
        let p = user_dir(&[]);
        assert_eq!(p.user, p.temp.join("RadMobile"));
    }

    /// A copy (hard link) of this test binary in a fresh folder next to
    /// it, so the portable `data` folder lands somewhere we own.
    fn binary_in_fresh_folder() -> (tempfile::TempDir, PathBuf) {
        let me = std::env::current_exe().unwrap();
        let dir = tempfile::Builder::new()
            .prefix("radm-paths-")
            .tempdir_in(me.parent().unwrap().parent().unwrap())
            .unwrap();
        let exe = dir.path().join(me.file_name().unwrap());
        if std::fs::hard_link(&me, &exe).is_err() {
            std::fs::copy(&me, &exe).unwrap();
        }
        (dir, exe)
    }

    #[test]
    fn the_portable_data_folder_comes_first_when_writable() {
        let (dir, exe) = binary_in_fresh_folder();
        let env: &[(&str, &str)] = &[("XDG_DATA_HOME", "/xdg"), ("LOCALAPPDATA", r"C:\Local")];
        let p = run_probe(&exe, "detect", env);
        let root = std::path::absolute(dir.path()).unwrap();
        assert_eq!(p.exe_dir.as_deref(), Some(root.as_path()));
        assert_eq!(p.data, Some(root.join("data")));
        assert_eq!(p.user_roms, Some(root.join("roms")));
        // the probe write leaves nothing behind
        let left: Vec<_> = std::fs::read_dir(root.join("data")).unwrap().collect();
        assert!(left.is_empty(), "{left:?}");
    }

    #[cfg(unix)]
    #[test]
    fn a_read_only_portable_folder_falls_back_to_the_user_folder() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, exe) = binary_in_fresh_folder();
        let set_mode = |mode| {
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(mode)).unwrap()
        };
        set_mode(0o555);
        // root writes anyway: nothing to check then
        let writable = std::fs::create_dir(dir.path().join("data")).is_ok();
        let p = (!writable).then(|| run_probe(&exe, "detect", &[("XDG_DATA_HOME", "/xdg")]));
        set_mode(0o755);
        if let Some(p) = p {
            assert_eq!(p.data, Some(PathBuf::from("/xdg/rad-mobile")));
        }
    }

    #[test]
    fn a_data_override_is_taken_as_given_and_made_absolute() {
        let given = AppPaths::detect(Some(PathBuf::from("rel/data")));
        assert_eq!(given.data, std::path::absolute("rel/data").unwrap());
        assert!(!Path::new("rel").exists(), "detect creates nothing");
        let abs = std::path::absolute("/somewhere/data").unwrap();
        assert_eq!(AppPaths::detect(Some(abs.clone())).data, abs);
    }
}
