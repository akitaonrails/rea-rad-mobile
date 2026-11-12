//! Rad Mobile launcher: a Wanszai-style front end for the bundled MAME,
//! with wheel, pedal and force feedback support.

#![cfg_attr(windows, windows_subsystem = "windows")]

mod art_io;
mod backdrop;
mod child;
mod input;
mod log;
mod paths;
mod session;
mod ui;

use std::path::PathBuf;
use std::time::Duration;

use eframe::egui;
use radmobile_core::settings::Display;

use crate::input::{Command, InputService};
use crate::paths::AppPaths;

const USAGE: &str = "\
Rad Mobile launcher

USAGE: RadMobile [OPTIONS]

  (no option)            open the menu
  --play                 start the game straight away (for frontends), exit when it ends
  --windowed             open the menu in a window
  --data <DIR>           keep settings and MAME data in DIR
  --check-roms           report what the ROM search finds, then exit
  --list-devices         list wheels, pedals and gamepads for 3 seconds, then exit
  --print-mame-command   print the MAME command line the current settings give, then exit
  --mame-args \"<ARGS>\" extra MAME options, appended as they are (with --play)
  --help                 this text
";

#[derive(Debug)]
struct Cli {
    play: bool,
    windowed: bool,
    data: Option<PathBuf>,
    check_roms: bool,
    list_devices: bool,
    print_mame: bool,
    mame_args: Vec<String>,
    /// Development: save the menu's first frames to a PNG and exit.
    screenshot: Option<ui::Screenshot>,
}

fn parse(args: impl Iterator<Item = String>) -> Result<Cli, String> {
    let mut cli = Cli {
        play: false,
        windowed: false,
        data: None,
        check_roms: false,
        list_devices: false,
        print_mame: false,
        mame_args: Vec::new(),
        screenshot: None,
    };
    let mut args = args.peekable();
    while let Some(a) = args.next() {
        match a.as_str() {
            "--play" => cli.play = true,
            "--windowed" => cli.windowed = true,
            "--data" => cli.data = Some(args.next().ok_or("--data needs a folder")?.into()),
            "--check-roms" => cli.check_roms = true,
            "--list-devices" => cli.list_devices = true,
            "--print-mame-command" => cli.print_mame = true,
            "--mame-args" => {
                let extra = args.next().ok_or("--mame-args needs a value")?;
                cli.mame_args
                    .extend(extra.split_whitespace().map(String::from));
            }
            "--screenshot" => {
                let path = args.next().ok_or("--screenshot needs a file")?;
                cli.screenshot.get_or_insert_with(Default::default).path = path.into();
            }
            "--screen" => {
                let name = args.next().ok_or("--screen needs a name")?;
                cli.screenshot.get_or_insert_with(Default::default).screen = name;
            }
            "--help" | "-h" => return Err(String::new()),
            other => return Err(format!("unknown option {other}")),
        }
    }
    Ok(cli)
}

fn main() {
    let cli = match parse(std::env::args().skip(1)) {
        Ok(c) => c,
        Err(e) => {
            if !e.is_empty() {
                eprintln!("{e}\n");
            }
            print!("{USAGE}");
            std::process::exit(if e.is_empty() { 0 } else { 2 });
        }
    };
    let paths = AppPaths::detect(cli.data.clone());
    let _ = std::fs::create_dir_all(&paths.data);
    log::init(&paths.log());
    log::line(&format!(
        "Rad Mobile launcher {} — {} — MAME {} — data {}",
        env!("CARGO_PKG_VERSION"),
        paths.exe_dir.display(),
        paths.mame_dir.display(),
        paths.data.display()
    ));
    let (settings, profile) = ui::load(&paths);

    let code = if cli.check_roms {
        let (inv, status) = session::scan(&paths, &settings);
        println!("{}", status.summary());
        for a in &inv.scanned {
            println!("  scanned {}", a.display());
        }
        i32::from(!matches!(status, session::RomStatus::Ready(_)))
    } else if cli.print_mame {
        match session::prepare(&paths, &settings, None) {
            Ok(p) => {
                for (k, v) in &p.env {
                    println!("{k}={v}");
                }
                println!("{} {}", p.exe.display(), p.args.join(" "));
                0
            }
            Err(e) => {
                eprintln!("{e:#}");
                1
            }
        }
    } else if cli.list_devices {
        list_devices(settings, profile)
    } else if cli.play {
        play(&paths, settings, profile, &cli.mame_args)
    } else {
        menu(paths, settings, profile, cli.windowed, cli.screenshot)
    };
    std::process::exit(code);
}

fn list_devices(
    settings: radmobile_core::settings::Settings,
    profile: Option<radmobile_core::wheel::WheelProfile>,
) -> i32 {
    let input = InputService::start(settings, profile);
    std::thread::sleep(Duration::from_secs(3));
    let snap = input.snapshot();
    if let Some(e) = &snap.error {
        eprintln!("input unavailable: {e}");
        return 1;
    }
    for (i, d) in snap.devices.iter().enumerate() {
        let st = &snap.states[i];
        let ffb = match (&d.ffb, d.haptic) {
            (Some(effects), _) => effects.as_str(),
            (None, true) => "yes",
            (None, false) => "none",
        };
        println!(
            "{i}: {:?} \"{}\" {:04x}:{:04x} guid {} axes {:?} buttons {} ffb {}",
            d.kind,
            d.id.name,
            d.id.vid,
            d.id.pid,
            d.id.guid,
            st.axes,
            st.buttons.len(),
            ffb
        );
        println!("   path {}", d.path);
    }
    println!("active: {:?}", snap.active);
    0
}

/// Run the game with no menu, feeding the wheel until MAME exits.
fn play(
    paths: &AppPaths,
    settings: radmobile_core::settings::Settings,
    profile: Option<radmobile_core::wheel::WheelProfile>,
    mame_args: &[String],
) -> i32 {
    let prepared = match session::prepare(paths, &settings, None) {
        Ok(mut p) => {
            p.args.extend_from_slice(mame_args);
            p
        }
        Err(e) => {
            log::line(&format!("{e:#}"));
            return 1;
        }
    };
    let mut child = match session::launch(&prepared) {
        Ok(c) => c,
        Err(e) => {
            log::line(&format!("mame: {e}"));
            return 1;
        }
    };
    let input = InputService::start(settings, profile);
    input.send(Command::Attach(prepared.bridge));
    let status = child.wait();
    input.send(Command::Detach);
    drop(input);
    let _ = std::fs::remove_file(paths.data.join("bridge.shm"));
    match status {
        Ok(s) => {
            log::line(&format!("mame exited ({s})"));
            s.code().unwrap_or(1)
        }
        Err(e) => {
            log::line(&format!("mame: {e}"));
            1
        }
    }
}

fn menu(
    paths: AppPaths,
    settings: radmobile_core::settings::Settings,
    profile: Option<radmobile_core::wheel::WheelProfile>,
    windowed: bool,
    screenshot: Option<ui::Screenshot>,
) -> i32 {
    let fullscreen = !windowed && screenshot.is_none() && settings.display == Display::Fullscreen;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Rad Mobile")
            .with_app_id("rad-mobile")
            .with_inner_size([1280.0, 720.0])
            .with_min_inner_size([640.0, 360.0])
            .with_fullscreen(fullscreen),
        ..Default::default()
    };
    let result = eframe::run_native(
        "Rad Mobile",
        options,
        Box::new(move |_cc| Ok(Box::new(ui::App::new(paths, settings, profile, screenshot)))),
    );
    match result {
        Ok(()) => 0,
        Err(e) => {
            log::line(&format!("window: {e}"));
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Result<Cli, String> {
        parse(v.iter().map(|s| s.to_string()))
    }

    #[test]
    fn parses_every_option() {
        let c = args(&[
            "--play",
            "--windowed",
            "--data",
            "/tmp/x",
            "--check-roms",
            "--list-devices",
            "--print-mame-command",
            "--mame-args",
            "-video none  -sound none",
        ])
        .unwrap();
        assert_eq!(c.mame_args, ["-video", "none", "-sound", "none"]);
        assert!(c.play && c.windowed && c.check_roms && c.list_devices && c.print_mame);
        assert_eq!(c.data, Some(PathBuf::from("/tmp/x")));
        assert!(args(&["--data"]).is_err());
        assert_eq!(args(&["--help"]).err(), Some(String::new()));
        assert!(args(&["--bogus"]).unwrap_err().contains("--bogus"));
        let none = args(&[]).unwrap();
        assert!(!none.play && none.data.is_none());
    }
}
