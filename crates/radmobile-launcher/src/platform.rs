//! What differs between Windows and Linux at start-up: the console for
//! the command-line answers, and one launcher at a time.

/// The single-instance name (the Windows mutex, the Linux lock file).
const INSTANCE: &str = "rea-rad-mobile";
/// The menu window's title, which a second start on Windows brings forward.
pub const TITLE: &str = "Rad Mobile";

/// What kind of run this is, for where messages go.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Run {
    /// The menu (and the games started from it).
    Menu,
    /// `--play`: straight into the game, for frontends.
    Play,
    /// `--help`, `--check-roms`, `--list-devices`, `--print-mame-command`,
    /// or a bad command line: the answer is the output.
    Command,
}

/// On Windows the launcher is a GUI program (no console window on a
/// double-click), so its output goes nowhere unless it attaches to the
/// console of the terminal that started it (SDK study §5.2: `--list-devices`
/// printed nothing on real Windows; Wine hid it). Output redirected to a
/// file or pipe is kept. The menu and `--play` don't attach. Nothing to
/// do elsewhere.
pub fn attach_console(run: Run) {
    // Only for command runs: `--play` from a frontend's `start ""` batch
    // file would keep that console window open behind the game.
    #[cfg(windows)]
    if run == Run::Command {
        rea_launcher_win32::console::attach_parent();
    }
    #[cfg(not(windows))]
    let _ = run;
}

/// Takes the launcher's single instance for as long as the returned guard
/// lives, or says another launcher (or its game) is running and gives the
/// exit code (5; 0 on Windows when the running menu was brought forward
/// instead). A second launcher would start a second MAME and truncate the
/// running game's `bridge.shm` (SDK study §5.9).
#[cfg(not(windows))]
pub fn single_instance(_run: Run) -> Result<impl Sized, i32> {
    rea_launcher::instance::single(INSTANCE).map_err(|running| {
        eprintln!("Rad Mobile: {running} (quit it, and its game, first)");
        running.exit_code()
    })
}

#[cfg(windows)]
pub fn single_instance(run: Run) -> Result<impl Sized, i32> {
    use rea_launcher_win32::instance::{self, Window};
    use rea_launcher_win32::report;
    instance::single(INSTANCE).map_err(|running| {
        let second = running.second_start(run != Run::Menu, Window::Own(TITLE));
        if let Some(text) = second.message(TITLE) {
            let audience = report::audience(report::Run {
                command: run == Run::Command,
                ..report::Run::default()
            });
            report::fatal(TITLE, &text, audience.mode());
        }
        second.exit_code()
    })
}

/// The single instance when no other launcher holds it, without telling
/// anyone: for `--check-roms` and `--list-devices`, which only look, so
/// they still answer while the game runs (they did in 0.5.0). Without the
/// instance they must leave the running launcher's `launcher.log` alone.
pub fn instance_if_free() -> Option<Box<dyn std::any::Any>> {
    #[cfg(not(windows))]
    let held = rea_launcher::instance::single(INSTANCE);
    #[cfg(windows)]
    let held = rea_launcher_win32::instance::single(INSTANCE);
    held.ok()
        .map(|guard| Box::new(guard) as Box<dyn std::any::Any>)
}
