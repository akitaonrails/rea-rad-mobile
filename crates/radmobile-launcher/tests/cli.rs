//! The launcher's command line end to end, pinned before the move onto
//! rea-launcher-sdk (the SDK study's "Step 0"). Only options that answer
//! and exit before any window, device or data folder is touched.

use std::process::{Command, Output};

use rea_launcher::snapshot;

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_radmobile-launcher"))
        .args(args)
        .output()
        .unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).unwrap()
}

#[test]
fn help_prints_the_usage_and_exits_0() {
    for flag in ["--help", "-h"] {
        let out = run(&[flag]);
        assert_eq!(out.status.code(), Some(0), "{flag}");
        snapshot!("help.txt", text(&out.stdout));
        assert_eq!(text(&out.stderr), "", "{flag}");
    }
    // --help wins wherever it is, and nothing before it runs
    let out = run(&["--check-roms", "--help"]);
    assert_eq!(out.status.code(), Some(0));
    snapshot!("help.txt", text(&out.stdout));
}

#[test]
fn a_bad_command_line_prints_why_and_the_usage_and_exits_2() {
    for (args, why) in [
        (&["--bogus"][..], "unknown option --bogus"),
        (&["--data"][..], "--data needs a folder"),
        (&["--mame-args"][..], "--mame-args needs a value"),
        (&["--screenshot"][..], "--screenshot needs a file"),
        (&["--screen"][..], "--screen needs a name"),
        (&["play"][..], "unknown option play"),
    ] {
        let out = run(args);
        assert_eq!(out.status.code(), Some(2), "{args:?}");
        assert_eq!(text(&out.stderr), format!("{why}\n\n"), "{args:?}");
        snapshot!("help.txt", text(&out.stdout));
    }
}

/// One launcher at a time (SDK study §5.9: a second one started a second
/// MAME and truncated the running game's bridge.shm). The lock is taken
/// here, in a private runtime folder, as a running launcher would hold it.
#[cfg(unix)]
#[test]
fn a_second_launcher_is_refused_with_exit_5_and_touches_nothing() {
    let runtime = tempfile::tempdir().unwrap();
    let data = tempfile::tempdir().unwrap();
    let lock = runtime.path().join("rea-launcher-rea-rad-mobile.lock");
    let run_check = || {
        Command::new(env!("CARGO_BIN_EXE_radmobile-launcher"))
            .args(["--check-roms", "--data"])
            .arg(data.path())
            .env("XDG_RUNTIME_DIR", runtime.path())
            .output()
            .unwrap()
    };
    let held = rea_launcher::instance::single_at(&lock).unwrap();
    assert!(held.is_held());
    let out = run_check();
    assert_eq!(out.status.code(), Some(5));
    assert_eq!(
        text(&out.stderr),
        "Rad Mobile: the launcher is already running (quit it, and its game, first)\n"
    );
    assert_eq!(text(&out.stdout), "");
    assert!(
        std::fs::read_dir(data.path()).unwrap().next().is_none(),
        "no log, no settings: the running launcher's files are left alone"
    );
    // once the first one is gone, the next one runs
    drop(held);
    let out = run_check();
    assert_eq!(out.status.code(), Some(1), "no ROMs here");
    assert!(text(&out.stdout).starts_with("ROM NOT FOUND"));
}
