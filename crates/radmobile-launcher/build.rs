//! The Windows executable's version information, icon and manifest
//! (rea-launcher-build): a named, versioned program with the launcher's
//! icon instead of an anonymous one that antivirus heuristics distrust.
fn main() {
    rea_launcher_build::Resources::new(
        "Rad Mobile Launcher",
        "Launcher for Sega's Rad Mobile with a bundled MAME",
    )
    .company("AkitaOnRails")
    .file_name("RadMobile.exe")
    .icon("../../packaging/appimage/radmobile-launcher.png")
    .embed()
    .unwrap();
}
