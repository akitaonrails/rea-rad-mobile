//! The Windows executable's version information and manifest
//! (rea-launcher-build): a named, versioned program instead of an
//! anonymous one that antivirus heuristics distrust. No icon yet: the
//! launcher has none of its own (the menu art comes from the player's ROM
//! and is never shipped).
fn main() {
    rea_launcher_build::Resources::new(
        "Rad Mobile Launcher",
        "Launcher for Sega's Rad Mobile with a bundled MAME",
    )
    .company("AkitaOnRails")
    .file_name("RadMobile.exe")
    .embed()
    .unwrap();
}
