//! The menu backdrop: once the ROM is found, MAME runs unseen for a few
//! seconds to take a snapshot of the intro's sunset skyline, which is then
//! treated ([`radmobile_core::art::backdrop`]) and kept in the data folder.
//! Made from the player's own ROM; nothing from the game ships.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use radmobile_core::settings::Settings;
use radmobile_core::{art, mame};

use crate::paths::AppPaths;
use crate::session;

/// Where the treated backdrop is kept.
pub fn path(app: &AppPaths) -> PathBuf {
    app.data.join("menu").join("backdrop.png")
}

/// MAME needs a few seconds at full speed; anything longer is stuck.
const TIMEOUT: Duration = Duration::from_secs(90);

/// Take and treat the snapshot. Blocks; run it on its own thread.
pub fn capture(app: &AppPaths, settings: &Settings) -> anyhow::Result<PathBuf> {
    let (set, paths) = session::ready_set(app, settings)?;
    let scratch = app.data.join("menu").join("capture");
    let _ = fs::remove_dir_all(&scratch);
    fs::create_dir_all(&scratch)?;
    let result = run(&paths, set.name, settings, &scratch).and_then(|snaps| pick(&snaps));
    let _ = fs::remove_dir_all(&scratch);
    let image = result?;
    let out = path(app);
    crate::art_io::save_png(&out, &image)?;
    Ok(out)
}

/// Run MAME on private copies of its files; the snapshot folder.
fn run(
    paths: &mame::Paths,
    set: &str,
    settings: &Settings,
    scratch: &Path,
) -> anyhow::Result<PathBuf> {
    // the player's operator settings, so the attract mode runs as theirs
    let eeprom = scratch.join("nvram").join(set).join("mainpcb_eeprom");
    if let Some(dir) = eeprom.parent() {
        fs::create_dir_all(dir)?;
    }
    if let Ok(own) = fs::read(paths.eeprom(set)) {
        fs::write(&eeprom, own)?;
    }
    session::write_eeprom(&eeprom, settings)?;
    let script = scratch.join("backdrop.lua");
    fs::write(&script, mame::backdrop_script())?;

    let mut cmd = Command::new(&paths.exe);
    cmd.args(mame::backdrop_args(set, &paths.roms, scratch, &script))
        .env("REA_SKIP_STARTUP_SCREENS", "1")
        .env("REA_S32_WIDE_MARGIN", "0")
        .current_dir(paths.exe.parent().unwrap_or(Path::new(".")))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let mut child = rea_launcher_native::child::spawn_tied(&mut cmd)?;
    let started = Instant::now();
    loop {
        if child.try_wait()?.is_some() {
            break;
        }
        if started.elapsed() > TIMEOUT {
            let _ = child.kill();
            let _ = child.wait();
            anyhow::bail!("MAME took too long");
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    Ok(scratch.join("snap"))
}

/// The first snapshot, in [`mame::BACKDROP_FRAMES`] order, that is a scene.
/// MAME numbers them in the order taken, which is frame order.
fn pick(snaps: &Path) -> anyhow::Result<art::Image> {
    let mut taken = mame::BACKDROP_FRAMES;
    taken.sort_unstable();
    for frame in mame::BACKDROP_FRAMES {
        let index = taken.iter().position(|f| *f == frame).unwrap_or_default();
        let file = snaps.join(format!("{index:04}.png"));
        if let Ok(snapshot) = crate::art_io::load_png(&file)
            && let Some(image) = art::backdrop(&snapshot)
        {
            return Ok(image);
        }
    }
    anyhow::bail!("no usable snapshot in {}", snaps.display())
}

#[cfg(test)]
mod tests {
    use super::*;
    use radmobile_core::art::Image;

    fn scene(width: u32, height: u32, busy: bool) -> Image {
        let mut img = Image::new(width, height);
        for (i, px) in img.rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let v = if busy && i % 5 < 2 { 220 } else { 70 };
            px.copy_from_slice(&[v, 60, 150, 255]);
        }
        img
    }

    #[test]
    fn the_preferred_frame_wins_and_blank_frames_are_skipped() {
        let dir = tempfile::tempdir().unwrap();
        // 380 (index 0) is a scene, 400 (index 1, preferred) is blank
        crate::art_io::save_png(&dir.path().join("0000.png"), &scene(320, 224, true)).unwrap();
        crate::art_io::save_png(&dir.path().join("0001.png"), &scene(320, 224, false)).unwrap();
        let image = pick(dir.path()).unwrap();
        assert_eq!(image.width, art::GAME_WIDTH);
        assert!(pick(&dir.path().join("missing")).is_err());
    }
}
