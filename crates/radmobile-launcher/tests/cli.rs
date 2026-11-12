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
