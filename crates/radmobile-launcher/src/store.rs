//! `settings.ini` and `wheel.ini` on disk, through `rea_launcher::cfg`.
//!
//! Read with [`cfg::read_file`]: UTF-8, else Latin-1 (a file saved by
//! Notepad as "ANSI" with an accented `rom_dir`), and only a missing file
//! is a first run. A file that exists but cannot be read is never
//! written over: the run goes on with the defaults and leaves the
//! player's file alone. Saving edits the file in place (the player's
//! comments and unknown lines stay) and writes it atomically.

use std::io;
use std::path::{Path, PathBuf};

use radmobile_core::settings::Settings;
use radmobile_core::wheel::{self, WheelProfile};
use rea_launcher::cfg;

use crate::paths::AppPaths;

/// What [`load`] found.
#[derive(Debug, Default, PartialEq)]
pub struct Loaded {
    pub settings: Settings,
    pub profile: Option<WheelProfile>,
    /// Files that are there but could not be read, with why: the run
    /// uses the defaults for them and must not save over them.
    pub unreadable: Vec<(PathBuf, String)>,
}

/// The saved settings and wheel profile (defaults when missing).
pub fn load(paths: &AppPaths) -> Loaded {
    let mut loaded = Loaded::default();
    let mut read = |path: PathBuf| match cfg::read_file(&path) {
        Ok(text) => text,
        Err(e) => {
            loaded.unreadable.push((path, e.to_string()));
            None
        }
    };
    let settings = read(paths.settings());
    let profile = read(paths.wheel_profile());
    loaded.settings = settings
        .as_deref()
        .map(Settings::from_cfg)
        .unwrap_or_default();
    loaded.profile = profile.as_deref().and_then(WheelProfile::from_cfg);
    loaded
}

/// The text of `path` to save into: "" when it does not exist yet; an
/// error (and nothing written) when it cannot be read.
fn existing(path: &Path) -> io::Result<String> {
    Ok(cfg::read_file(path)?.unwrap_or_default())
}

/// `settings` written into `settings.ini`.
pub fn save_settings(path: &Path, settings: &Settings) -> io::Result<()> {
    let text = settings.to_cfg(&existing(path)?);
    crate::session::write_atomic(path, text.as_bytes())
}

/// `profile` written into `wheel.ini`, or the file removed when there is
/// no profile.
pub fn save_profile(path: &Path, profile: Option<&WheelProfile>) -> io::Result<()> {
    match profile {
        Some(p) => {
            let text = p.to_cfg(&existing(path)?, wheel::WHEEL_INI_HEADER);
            crate::session::write_atomic(path, text.as_bytes())
        }
        None => match std::fs::remove_file(path) {
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(()),
            r => r,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(dir: &Path) -> AppPaths {
        AppPaths {
            exe_dir: dir.to_path_buf(),
            mame_dir: dir.join("mame"),
            data: dir.to_path_buf(),
            user_roms: dir.join("roms"),
        }
    }

    #[test]
    fn saving_keeps_the_players_comments_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let paths = app(dir.path());
        std::fs::write(paths.settings(), "# my note\r\nDifficulty=3\r\nbezel=1\r\n").unwrap();
        let mut loaded = load(&paths);
        assert!(loaded.unreadable.is_empty());
        assert_eq!(loaded.settings.game.difficulty, 3);
        loaded.settings.game.difficulty = 4;
        save_settings(&paths.settings(), &loaded.settings).unwrap();
        let text = std::fs::read_to_string(paths.settings()).unwrap();
        assert!(
            text.starts_with("# my note\r\ndifficulty = 4\r\nbezel = on\r\n"),
            "{text}"
        );
        assert_eq!(load(&paths).settings, loaded.settings);
    }

    #[test]
    fn a_profile_is_saved_into_the_old_file_and_removed() {
        let dir = tempfile::tempdir().unwrap();
        let path = app(dir.path()).wheel_profile();
        std::fs::write(
            &path,
            "# by hand\nwheel_vid=0eb7\nwheel_pid=0020\nrotation=900\n",
        )
        .unwrap();
        let mut p = WheelProfile::from_cfg(&std::fs::read_to_string(&path).unwrap()).unwrap();
        p.rotation = 1080;
        save_profile(&path, Some(&p)).unwrap();
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.starts_with("# by hand\n"), "{text}");
        assert!(text.contains("\nrotation = 1080\n"));
        save_profile(&path, None).unwrap();
        assert!(!path.exists());
        save_profile(&path, None).unwrap();
    }

    /// SDK study §5.8: a settings file that can't be read (here: a folder
    /// in its place) gives the defaults for this run, is reported, and is
    /// never written over.
    #[test]
    fn an_unreadable_file_is_reported_and_never_overwritten() {
        let dir = tempfile::tempdir().unwrap();
        let paths = app(dir.path());
        std::fs::create_dir(paths.settings()).unwrap();
        std::fs::create_dir(paths.wheel_profile()).unwrap();
        let loaded = load(&paths);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.profile, None);
        let names: Vec<_> = loaded.unreadable.iter().map(|(p, _)| p.clone()).collect();
        assert_eq!(names, [paths.settings(), paths.wheel_profile()]);
        assert!(save_settings(&paths.settings(), &loaded.settings).is_err());
        assert!(paths.settings().is_dir());
    }
}
