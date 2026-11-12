//! The menu: a Wanszai-style front end (header bar, content window,
//! tooltip bar with the controls) over a Rad Mobile sunset. Driven by the
//! wheel, a gamepad, the keyboard or the mouse.

use std::process::Child;
use std::time::{Duration, Instant};

use eframe::egui::{
    self, Align2, Color32, CornerRadius, FontId, Pos2, Rect, Stroke, StrokeKind, Vec2,
};
use radmobile_core::art::{self, palette};
use radmobile_core::controls::{Action, Button};
use radmobile_core::ini::Ini;
use radmobile_core::pad_map::{Capture, PadCapture, PadMap};
use radmobile_core::settings::{self as cfg, InputMode, Settings};
use radmobile_core::wheel::wizard::{Status, Step, Wizard};
use radmobile_core::wheel::{self, DeviceId, WheelProfile};

use crate::input::{Active, Command, InputService, Kind, Nav, Snapshot};
use crate::paths::AppPaths;
use crate::session::{self, RomStatus};

// The Rad Mobile title screen: a purple dusk sky, crimson clouds, the gold
// of "MOBILE" and the chrome of "RAD" (radmobile_core::art::palette).
const fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}
const SKY_TOP: Color32 = rgb(palette::NIGHT);
const SKY_MID: Color32 = rgb(palette::PURPLE);
const HORIZON: Color32 = rgb(palette::CRIMSON);
const GROUND: Color32 = Color32::from_rgb(0x0a, 0x06, 0x14);
const GRID: Color32 = Color32::from_rgb(0x9a, 0x4a, 0xc8);
const GOLD: Color32 = rgb(palette::GOLD);
const CHROME: Color32 = rgb(palette::CHROME);
const PANEL: Color32 = Color32::from_rgba_premultiplied(10, 6, 22, 220);
const TEXT: Color32 = Color32::from_rgb(0xf4, 0xf0, 0xff);
const DIM: Color32 = Color32::from_rgb(0xb0, 0xa4, 0xcc);
/// The selected row: gold like "MOBILE", with dark text on it.
const SELECT: Color32 = rgb(palette::GOLD);
const ON_SELECT: Color32 = rgb(palette::NIGHT);
const ACCENT: Color32 = rgb(palette::CRIMSON);
const OK: Color32 = Color32::from_rgb(0x6c, 0xf0, 0x9a);
const BAD: Color32 = Color32::from_rgb(0xff, 0x5a, 0x6a);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Screen {
    Main,
    Settings,
    Video,
    Sound,
    Controller,
    PadMap,
    Wheel,
    WheelMap,
    Game,
    Roms,
    Wizard,
    Running,
}

impl Screen {
    fn title(self) -> &'static str {
        match self {
            Screen::Main => "MAIN MENU",
            Screen::Settings => "SETTINGS",
            Screen::Video => "VIDEO SETTINGS",
            Screen::Sound => "SOUND SETTINGS",
            Screen::Controller => "CONTROLLER SETTINGS",
            Screen::PadMap => "GAMEPAD BUTTONS",
            Screen::Wheel => "WHEEL SETTINGS",
            Screen::WheelMap => "WHEEL BUTTONS",
            Screen::Game => "GAME SETTINGS",
            Screen::Roms => "ROMS",
            Screen::Wizard => "WHEEL SETUP",
            Screen::Running => "RACING",
        }
    }

    fn parent(self) -> Option<Screen> {
        match self {
            Screen::Main | Screen::Running => None,
            Screen::Settings => Some(Screen::Main),
            Screen::Wizard | Screen::WheelMap => Some(Screen::Wheel),
            Screen::PadMap => Some(Screen::Controller),
            _ => Some(Screen::Settings),
        }
    }
}

/// One line of a menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Item {
    Start,
    OpenSettings,
    Quit,
    Open(Screen),
    Display,
    Resolution,
    Aspect,
    WideTarget,
    Bezel,
    Filter,
    CrtCurvature,
    Vsync,
    Brightness,
    Volume,
    AttractSound,
    InputDevice,
    SteerAssist,
    StickDeadzone,
    StickLinearity,
    TriggerDeadzone,
    TriggerLinearity,
    Rumble,
    FfbStrength,
    FfbInvert,
    SteeringLock,
    WheelSetup,
    TestFfb,
    InvertAccel,
    InvertBrake,
    ResetWheel,
    /// Press A, then the new button or pedal for this control.
    Map(Action),
    ResetPadMap,
    Region,
    Preset,
    Difficulty,
    Timer,
    RaceTime,
    RivalSpeed,
    PoliceSpeed,
    FreePlay,
    CreditsStart,
    CreditsContinue,
    RomFolder,
    Rescan,
    Back,
}

fn items(screen: Screen, s: &Settings) -> Vec<Item> {
    use Item::*;
    match screen {
        Screen::Main => vec![Start, OpenSettings, Quit],
        Screen::Settings => vec![
            Open(Screen::Video),
            Open(Screen::Sound),
            Open(Screen::Controller),
            Open(Screen::Wheel),
            Open(Screen::Game),
            Open(Screen::Roms),
            Back,
        ],
        Screen::Video => {
            let mut v = vec![Display, Resolution, Aspect];
            if s.aspect == cfg::Aspect::Original {
                v.push(Bezel);
            } else {
                v.push(WideTarget);
            }
            v.push(Filter);
            if s.filter.has_curvature_switch() {
                v.push(CrtCurvature);
            }
            v.extend([Vsync, Brightness, Back]);
            v
        }
        Screen::Sound => vec![Volume, AttractSound, Back],
        Screen::Controller => vec![
            InputDevice,
            SteerAssist,
            Open(Screen::PadMap),
            StickDeadzone,
            StickLinearity,
            TriggerDeadzone,
            TriggerLinearity,
            Rumble,
            Back,
        ],
        Screen::Wheel => vec![
            InputDevice,
            FfbStrength,
            FfbInvert,
            SteeringLock,
            WheelSetup,
            Open(Screen::WheelMap),
            TestFfb,
            InvertAccel,
            InvertBrake,
            ResetWheel,
            Back,
        ],
        Screen::Game => vec![
            Region,
            Preset,
            Difficulty,
            Timer,
            RaceTime,
            RivalSpeed,
            PoliceSpeed,
            FreePlay,
            CreditsStart,
            CreditsContinue,
            Back,
        ],
        Screen::PadMap => Action::ALL
            .into_iter()
            .map(Map)
            .chain([ResetPadMap, Back])
            .collect(),
        Screen::WheelMap => Action::ALL
            .into_iter()
            .map(Map)
            .chain([ResetWheel, Back])
            .collect(),
        Screen::Roms => vec![RomFolder, Rescan, Back],
        Screen::Wizard | Screen::Running => vec![],
    }
}

fn steering_label(assisted: bool) -> String {
    if assisted { "ASSISTED" } else { "ARCADE" }.into()
}

fn on_off(b: bool) -> String {
    if b { "ON".into() } else { "OFF".into() }
}

fn label(item: Item) -> &'static str {
    use Item::*;
    match item {
        Start => "START GAME",
        OpenSettings => "SETTINGS",
        Quit => "QUIT",
        Open(s) => s.title(),
        Display => "DISPLAY",
        Resolution => "RESOLUTION",
        Aspect => "ASPECT RATIO",
        WideTarget => "SCREEN SHAPE",
        Bezel => "CABINET BEZEL",
        CrtCurvature => "CRT CURVATURE",
        Filter => "FILTER",
        Vsync => "VSYNC",
        Brightness => "BRIGHTNESS",
        Volume => "MASTER VOLUME",
        AttractSound => "ATTRACT MODE SOUND",
        InputDevice => "INPUT DEVICE",
        SteerAssist => "STICK STEERING",
        StickDeadzone => "STICK DEADZONE",
        StickLinearity => "STICK LINEARITY",
        TriggerDeadzone => "TRIGGER DEADZONE",
        TriggerLinearity => "TRIGGER LINEARITY",
        Rumble => "RUMBLE",
        FfbStrength => "FFB STRENGTH",
        FfbInvert => "INVERT FFB",
        SteeringLock => "STEERING LOCK",
        WheelSetup => "WHEEL SETUP",
        TestFfb => "TEST FORCE FEEDBACK",
        InvertAccel => "INVERT ACCEL",
        InvertBrake => "INVERT BRAKE",
        ResetWheel => "RESET WHEEL TO AUTOMATIC",
        Map(action) => action.label(),
        ResetPadMap => "RESET TO DEFAULTS",
        Region => "REGION",
        Difficulty => "DIFFICULTY",
        Preset => "PRESET",
        Timer => "STARTING TIMER",
        RaceTime => "RACE CLOCK",
        RivalSpeed => "RIVAL CAR SPEED",
        PoliceSpeed => "POLICE CAR SPEED",
        FreePlay => "FREE PLAY",
        CreditsStart => "CREDITS TO START",
        CreditsContinue => "CREDITS TO CONTINUE",
        RomFolder => "EXTRA ROM FOLDER",
        Rescan => "SCAN FOR ROMS",
        Back => "BACK",
    }
}

fn help(item: Item) -> &'static str {
    use Item::*;
    match item {
        Start => "Race through America's highways. Your own radm.zip is required.",
        OpenSettings => "Video, sound, controller, wheel and arcade settings.",
        Quit => "Leave the launcher.",
        Open(_) | Back => "",
        Display => "Fullscreen or a window.",
        Resolution => "The size the game is rendered at. Desktop uses your screen's own.",
        Aspect => {
            "4:3 as in the arcade, stretched, or true widescreen: more road on the sides (experimental)."
        }
        WideTarget => "The shape of your screen, for stretch and true widescreen.",
        Filter => {
            "Sharp, smooth, or a CRT. Arcade and Trinitron CRT follow Wanszai's ports: scanlines, phosphor mask and glow."
        }
        Bezel => "Frame the 4:3 picture in an arcade cabinet's monitor surround.",
        CrtCurvature => "The curved glass and darker corners of a CRT tube.",
        Vsync => "Sync to the display to avoid tearing.",
        Brightness => "Picture brightness.",
        Volume => "Game volume.",
        AttractSound => "Play sound while the game shows its attract demo.",
        InputDevice => "Auto uses a wheel when one is connected, otherwise a gamepad.",
        SteerAssist => {
            "Assisted: the stick moves the car across the road; let go and it keeps its line, through bends too. Arcade: the stick is the cabinet wheel, so you counter-steer."
        }
        StickDeadzone => "How far the stick moves before the car steers.",
        StickLinearity | TriggerLinearity => {
            "Curve: Linear is direct, Cubic is gentle near the centre."
        }
        TriggerDeadzone => "How far a trigger moves before it counts.",
        Rumble => "Gamepad vibration for crashes and the barriers.",
        FfbStrength => {
            "Force feedback strength. Rad Mobile had no motor: forces come from the car's speed, crashes and the barriers."
        }
        FfbInvert => "Flip the force direction if the wheel pushes the wrong way.",
        SteeringLock => "Degrees of wheel rotation for full lock. 270 matches the arcade cabinet.",
        WheelSetup => {
            "Press each pedal and button when asked. Steering is detected automatically. To change one control, use WHEEL BUTTONS."
        }
        TestFfb => "Push the wheel left, then right.",
        InvertAccel | InvertBrake => "Flip a pedal that reads backwards.",
        ResetWheel => {
            "Forget the wheel setup and button mapping: steering, pedals and shift paddles as detected."
        }
        Map(_) => {
            "Select, then press the button (or pedal) to use. Esc, or waiting a few seconds, keeps it."
        }
        ResetPadMap => {
            "RT or A accelerates, LT or B brakes, X or LB lights, Y or RB wiper, Start, Back adds a coin, Guide returns to the menu."
        }
        Region => "World or US set, when you have both.",
        Difficulty => "The arcade's difficulty, 1 (easy) to 16 (hard). 8 is the factory setting.",
        Preset => {
            "Easy: lowest difficulty, slow rivals and police, 60 seconds and a clock at half speed. Arcade: the factory settings."
        }
        Timer => "Seconds on the clock at the start of each round.",
        RaceTime => {
            "How long each second on the clock lasts: 2x doubles the start time and every checkpoint bonus."
        }
        RivalSpeed => "How fast the rival car drives.",
        PoliceSpeed => "How fast the police cars drive.",
        FreePlay => "No coins needed.",
        CreditsStart | CreditsContinue => {
            "Credits needed when free play is off (Coin button adds one)."
        }
        RomFolder => {
            "Another folder to search for the ROM zip, besides the roms folder next to the launcher. Type a path and press Enter."
        }
        Rescan => "Search the ROM folders again.",
    }
}

fn cycle<T: Copy + PartialEq>(all: &[T], cur: T, d: i32) -> T {
    let i = all.iter().position(|x| *x == cur).unwrap_or(0) as i32;
    all[(i + d).rem_euclid(all.len() as i32) as usize]
}

fn step_u8(v: u8, d: i32, step: i32, lo: i32, hi: i32) -> u8 {
    (i32::from(v) + d * step).clamp(lo, hi) as u8
}

/// Development aid: open `screen`, save a frame to `path`, and exit.
#[derive(Debug, Clone, Default)]
pub struct Screenshot {
    pub path: std::path::PathBuf,
    pub screen: String,
}

fn screen_by_name(name: &str) -> Screen {
    match name {
        "settings" => Screen::Settings,
        "video" => Screen::Video,
        "sound" => Screen::Sound,
        "controller" => Screen::Controller,
        "padmap" => Screen::PadMap,
        "wheelmap" => Screen::WheelMap,
        "wheel" => Screen::Wheel,
        "game" => Screen::Game,
        "roms" => Screen::Roms,
        "running" => Screen::Running,
        _ => Screen::Main,
    }
}

fn save_png(path: &std::path::Path, image: &egui::ColorImage) -> anyhow::Result<()> {
    crate::art_io::save_png(
        path,
        &art::Image {
            width: image.size[0] as u32,
            height: image.size[1] as u32,
            rgba: image.as_raw().to_vec(),
        },
    )
}

struct WizardRun {
    wizard: Wizard,
    started: Instant,
    /// Device ids, in the order given to the wizard (0 is the wheel).
    devices: Vec<(usize, DeviceId)>,
    status: Status,
    done_at: Option<Instant>,
}

/// Waiting for the new pad input, or wheel button or pedal, of one control.
struct Remap {
    action: Action,
    started: Instant,
    on: RemapOn,
}

enum RemapOn {
    Pad(PadCapture),
    /// A one-step wizard over the wheel and the devices beside it.
    Wheel {
        wizard: Box<Wizard>,
        devices: Vec<(usize, DeviceId)>,
    },
}

/// After a capture, menu input is ignored this long, so the press that was
/// just captured does not also act on the menu.
const REMAP_QUIET: Duration = Duration::from_millis(400);

pub struct App {
    paths: AppPaths,
    settings: Settings,
    profile: Option<WheelProfile>,
    input: InputService,
    screen: Screen,
    selected: usize,
    rom: RomStatus,
    child: Option<Child>,
    wizard: Option<WizardRun>,
    remap: Option<Remap>,
    /// The treated snapshot behind the menu, once there is one.
    backdrop: Option<egui::TextureHandle>,
    /// Taking the snapshot (MAME, unseen) in the background.
    backdrop_job: Option<std::thread::JoinHandle<anyhow::Result<std::path::PathBuf>>>,
    /// The last capture failed; try again only after a ROM rescan.
    backdrop_failed: bool,
    /// Menu input is ignored until then (see [`REMAP_QUIET`]).
    quiet_until: Option<Instant>,
    /// What the remap row shows while waiting.
    remap_status: Option<String>,
    rom_dir_text: String,
    message: Option<(String, Color32, Instant)>,
    snapshot: Snapshot,
    screenshot: Option<(Screenshot, u32)>,
}

impl App {
    pub fn new(
        paths: AppPaths,
        settings: Settings,
        profile: Option<WheelProfile>,
        screenshot: Option<Screenshot>,
    ) -> Self {
        let input = InputService::start(settings.clone(), profile.clone());
        let (_, rom) = session::scan(&paths, &settings);
        crate::log::line(&rom.summary());
        let rom_dir_text = settings
            .rom_dir
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self {
            paths,
            settings,
            profile,
            input,
            selected: 0,
            rom,
            child: None,
            wizard: None,
            remap: None,
            backdrop: None,
            backdrop_job: None,
            backdrop_failed: false,
            quiet_until: None,
            remap_status: None,
            rom_dir_text,
            message: None,
            snapshot: Snapshot::default(),
            screen: screenshot
                .as_ref()
                .map_or(Screen::Main, |s| screen_by_name(&s.screen)),
            screenshot: screenshot.map(|s| (s, 0)),
        }
    }

    /// Ask for a frame once the screen has settled, save it, and close.
    fn screenshot_step(&mut self, ctx: &egui::Context) {
        let Some((shot, frames)) = self.screenshot.as_mut() else {
            return;
        };
        *frames += 1;
        if *frames == 1 {
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(Vec2::new(1280.0, 720.0)));
        }
        if *frames == 30 {
            let path = shot.path.clone();
            let ctx2 = ctx.clone();
            ctx.send_viewport_cmd(egui::ViewportCommand::Screenshot(
                egui::ScreenshotCallback::new(move |image| {
                    match save_png(&path, &image) {
                        Ok(()) => crate::log::line(&format!("screenshot: {}", path.display())),
                        Err(e) => crate::log::line(&format!("screenshot: {e}")),
                    }
                    ctx2.send_viewport_cmd(egui::ViewportCommand::Close);
                }),
            ));
        }
    }

    fn say(&mut self, text: impl Into<String>, color: Color32) {
        self.message = Some((text.into(), color, Instant::now()));
    }

    fn save(&mut self) {
        let text = self.settings.to_ini().to_string();
        if let Err(e) = std::fs::write(self.paths.settings(), text) {
            self.say(format!("COULD NOT SAVE SETTINGS: {e}"), BAD);
        }
        self.input.send(Command::Configure {
            settings: Box::new(self.settings.clone()),
            profile: self.profile.clone(),
        });
    }

    fn save_profile(&mut self) {
        let result = match &self.profile {
            Some(p) => std::fs::write(self.paths.wheel_profile(), p.to_ini().to_string()),
            None => match std::fs::remove_file(self.paths.wheel_profile()) {
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                r => r,
            },
        };
        if let Err(e) = result {
            self.say(
                format!("COULD NOT WRITE WHEEL.INI - SETTINGS UNCHANGED ({e})"),
                BAD,
            );
        }
        self.save();
    }

    fn go(&mut self, screen: Screen) {
        self.screen = screen;
        self.selected = 0;
    }

    fn back(&mut self, ctx: &egui::Context) {
        match self.screen.parent() {
            Some(p) => {
                let from = self.screen;
                self.screen = p;
                self.selected = items(p, &self.settings)
                    .iter()
                    .position(|i| *i == Item::Open(from))
                    .unwrap_or(0);
                self.wizard = None;
            }
            None if self.screen == Screen::Main => {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close)
            }
            None => {}
        }
    }

    /// Show the backdrop when it exists; otherwise take it once the ROM is
    /// found (not while a game runs, and not in screenshot mode).
    fn tick_backdrop(&mut self, ctx: &egui::Context) {
        if self.backdrop.is_some() {
            return;
        }
        if let Some(job) = self.backdrop_job.take_if(|j| j.is_finished()) {
            match job.join() {
                Ok(Ok(path)) => crate::log::line(&format!("menu: backdrop {}", path.display())),
                Ok(Err(e)) => {
                    crate::log::line(&format!("menu: no backdrop ({e:#})"));
                    self.backdrop_failed = true;
                }
                Err(_) => self.backdrop_failed = true,
            }
        }
        let file = crate::backdrop::path(&self.paths);
        if file.is_file() {
            match crate::art_io::load_png(&file) {
                Ok(img) => {
                    let image = egui::ColorImage::from_rgba_unmultiplied(
                        [img.width as usize, img.height as usize],
                        &img.rgba,
                    );
                    self.backdrop =
                        Some(ctx.load_texture("backdrop", image, egui::TextureOptions::NEAREST));
                }
                Err(e) => {
                    crate::log::line(&format!("menu: backdrop unreadable ({e:#})"));
                    let _ = std::fs::remove_file(&file);
                }
            }
            return;
        }
        let ready = matches!(self.rom, RomStatus::Ready(_));
        if ready
            && self.backdrop_job.is_none()
            && !self.backdrop_failed
            && self.child.is_none()
            && self.screenshot.is_none()
        {
            let (paths, settings) = (self.paths.clone(), self.settings.clone());
            self.backdrop_job = Some(std::thread::spawn(move || {
                crate::backdrop::capture(&paths, &settings)
            }));
        }
    }

    fn rescan_roms(&mut self) {
        let (_, rom) = session::scan(&self.paths, &self.settings);
        self.rom = rom;
        self.backdrop_failed = false;
        let color = if matches!(self.rom, RomStatus::Ready(_)) {
            OK
        } else {
            BAD
        };
        self.say(self.rom.summary(), color);
    }

    fn start_game(&mut self, ctx: &egui::Context) {
        // the backdrop capture may be rebuilding the romset: let it finish
        // (a few seconds, once); its picture is picked up from the file
        if let Some(job) = self.backdrop_job.take() {
            let _ = job.join();
        }
        let monitor = ctx.input(|i| i.viewport().monitor_size).map(|s| {
            let ppp = ctx.pixels_per_point() / ctx.zoom_factor();
            radmobile_core::mame::Screen {
                width: (s.x * ppp) as u32,
                height: (s.y * ppp) as u32,
            }
        });
        let result = session::prepare(&self.paths, &self.settings, monitor).and_then(|p| {
            let child = session::launch(&p)?;
            Ok((p, child))
        });
        match result {
            Ok((prepared, child)) => {
                crate::log::line(&format!("game: {} started", prepared.set.name));
                self.input.send(Command::Attach(prepared.bridge));
                self.child = Some(child);
                self.go(Screen::Running);
            }
            Err(e) => {
                crate::log::line(&format!("game: {e:#}"));
                self.say(format!("{e:#}"), BAD);
            }
        }
    }

    fn poll_child(&mut self) {
        let Some(child) = self.child.as_mut() else {
            return;
        };
        match child.try_wait() {
            Ok(Some(status)) => {
                crate::log::line(&format!("game: MAME exited ({status})"));
                self.child = None;
                self.input.send(Command::Detach);
                let _ = std::fs::remove_file(self.paths.data.join("bridge.shm"));
                if !status.success() {
                    self.say(
                        format!("MAME STOPPED WITH AN ERROR ({status}) - SEE data/mame.log"),
                        BAD,
                    );
                }
                self.go(Screen::Main);
            }
            Ok(None) => {}
            Err(e) => crate::log::line(&format!("game: {e}")),
        }
    }

    /// The wheel in use and up to three other non-pad devices (separate
    /// pedals, a button box), in the order the wizard sees them.
    fn wheel_devices(&mut self) -> Option<Vec<(usize, DeviceId)>> {
        let snap = &self.snapshot;
        let Active::Wheel(w) = snap.active else {
            self.say("CONNECT A WHEEL AND TRY AGAIN", BAD);
            return None;
        };
        let mut devices = vec![(w, snap.devices[w].id.clone())];
        devices.extend(
            snap.devices
                .iter()
                .enumerate()
                .filter(|(i, d)| *i != w && d.kind != Kind::Pad)
                .take(3)
                .map(|(i, d)| (i, d.id.clone())),
        );
        Some(devices)
    }

    fn steer_axis(&self) -> usize {
        self.snapshot.profile.as_ref().map_or(0, |p| p.steer_axis)
    }

    /// The readings of `devices` (indices into the snapshot).
    fn device_states(&self, devices: &[(usize, DeviceId)]) -> Vec<wheel::DeviceState> {
        devices
            .iter()
            .map(|(i, _)| self.snapshot.states.get(*i).cloned().unwrap_or_default())
            .collect()
    }

    /// The wheel profile in use (saved or detected) for the wizard's wheel.
    fn base_profile(&self, devices: &[(usize, DeviceId)]) -> WheelProfile {
        let (index, id) = &devices[0];
        let mut profile = self.snapshot.profile.clone().unwrap_or_else(|| {
            WheelProfile::guess(
                id.clone(),
                &self
                    .snapshot
                    .states
                    .get(*index)
                    .cloned()
                    .unwrap_or_default(),
            )
        });
        profile.wheel = id.clone();
        profile
    }

    fn start_wizard(&mut self) {
        let Some(devices) = self.wheel_devices() else {
            return;
        };
        let steer_axis = self.steer_axis();
        self.wizard = Some(WizardRun {
            wizard: Wizard::new(steer_axis),
            started: Instant::now(),
            devices,
            status: Status::WarmingUp,
            done_at: None,
        });
        self.go(Screen::Wizard);
    }

    fn tick_wizard(&mut self) {
        let Some(run) = self.wizard.as_ref() else {
            return;
        };
        if run.done_at.is_some() {
            return;
        }
        let states = self.device_states(&run.devices);
        let elapsed = run.started.elapsed().as_millis() as u64;
        let Some(run) = self.wizard.as_mut() else {
            return;
        };
        run.status = run.wizard.tick(elapsed, &states);
        if let Status::Done(captures) = run.status.clone() {
            run.done_at = Some(Instant::now());
            let devices = run.devices.clone();
            let mut profile = self.base_profile(&devices);
            // a full setup starts over on the second device
            profile.forget_aux();
            let ids: Vec<DeviceId> = devices.into_iter().map(|(_, id)| id).collect();
            profile.apply(&captures, &ids);
            self.profile = Some(profile);
            self.settings.wheel.invert_accel = false;
            self.settings.wheel.invert_brake = false;
            self.save_profile();
            crate::log::line(&format!("wizard: done, skipped {:?}", captures.skipped));
        }
    }

    /// Start waiting for the new input of `action` on this screen's device.
    fn start_remap(&mut self, action: Action) {
        let on = if self.screen == Screen::PadMap {
            if self.snapshot.pads.is_empty() {
                self.say("CONNECT A GAMEPAD AND TRY AGAIN", BAD);
                return;
            }
            RemapOn::Pad(PadCapture::default())
        } else {
            let Some(devices) = self.wheel_devices() else {
                return;
            };
            let step = match action {
                Action::Accelerate => Step::Accelerator,
                Action::Brake => Step::Brake,
                Action::Press(b) => Step::Button(b),
            };
            RemapOn::Wheel {
                wizard: Box::new(Wizard::single(step, self.steer_axis())),
                devices,
            }
        };
        self.remap = Some(Remap {
            action,
            started: Instant::now(),
            on,
        });
        self.remap_status = Some("PRESS…".into());
    }

    fn end_remap(&mut self, message: String, color: Color32) {
        self.remap = None;
        self.remap_status = None;
        self.quiet_until = Some(Instant::now() + REMAP_QUIET);
        self.say(message, color);
    }

    fn tick_remap(&mut self, ctx: &egui::Context) {
        let Some(remap) = self.remap.as_ref() else {
            return;
        };
        let action = remap.action;
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.end_remap(format!("{} UNCHANGED", action.label()), DIM);
            return;
        }
        let elapsed = remap.started.elapsed().as_millis() as u64;
        let states = match &remap.on {
            RemapOn::Wheel { devices, .. } => self.device_states(devices),
            RemapOn::Pad(_) => Vec::new(),
        };
        let pads = self.snapshot.pads.clone();
        let Some(remap) = self.remap.as_mut() else {
            return;
        };
        match &mut remap.on {
            RemapOn::Pad(capture) => match capture.tick(elapsed, &pads) {
                Capture::Got(input) => {
                    self.settings.pad.map.assign(action, input);
                    self.save();
                    self.end_remap(format!("{} SET TO {}", action.label(), input.label()), OK);
                }
                Capture::TimedOut => self.end_remap(
                    format!("NOTHING PRESSED - {} UNCHANGED", action.label()),
                    DIM,
                ),
                Capture::Releasing => self.remap_status = Some("LET GO…".into()),
                Capture::Waiting { remaining_ms } => {
                    self.remap_status = Some(format!("PRESS… {}", remaining_ms.div_ceil(1000)))
                }
            },
            RemapOn::Wheel { wizard, devices } => match wizard.tick(elapsed, &states) {
                Status::Done(captures) if captures.skipped.is_empty() => {
                    let devices = devices.clone();
                    let mut profile = self.base_profile(&devices);
                    let ids: Vec<DeviceId> = devices.into_iter().map(|(_, id)| id).collect();
                    profile.apply(&captures, &ids);
                    let shown = profile.describe(action);
                    self.profile = Some(profile);
                    self.save_profile();
                    self.end_remap(format!("{} SET TO {shown}", action.label()), OK);
                }
                Status::Done(_) => self.end_remap(
                    format!("NOTHING PRESSED - {} UNCHANGED", action.label()),
                    DIM,
                ),
                Status::Waiting { remaining_ms, .. } => {
                    self.remap_status = Some(format!("PRESS… {}", remaining_ms.div_ceil(1000)))
                }
                Status::Pressing { .. } => self.remap_status = Some("NOW LET GO".into()),
                Status::WarmingUp => {}
            },
        }
    }

    /// Change or activate an item; `d` is −1/+1 for left/right, 0 for confirm.
    fn act(&mut self, item: Item, d: i32, ctx: &egui::Context) {
        use Item::*;
        let s = &mut self.settings;
        let dd = if d == 0 { 1 } else { d };
        let mut changed = true;
        match item {
            Start if d == 0 => {
                changed = false;
                self.start_game(ctx);
            }
            OpenSettings if d == 0 => {
                changed = false;
                self.go(Screen::Settings);
            }
            Quit if d == 0 => {
                changed = false;
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Open(sc) if d == 0 => {
                changed = false;
                self.go(sc);
            }
            Back if d == 0 => {
                changed = false;
                self.back(ctx);
            }
            Display => {
                s.display = if s.display == cfg::Display::Fullscreen {
                    cfg::Display::Window
                } else {
                    cfg::Display::Fullscreen
                };
                ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(
                    s.display == cfg::Display::Fullscreen,
                ));
            }
            Resolution => s.resolution = cycle(&cfg::Resolution::ALL, s.resolution, dd),
            Aspect => s.aspect = cycle(&cfg::Aspect::ALL, s.aspect, dd),
            WideTarget => s.wide_target = cycle(&cfg::WideTarget::ALL, s.wide_target, dd),
            Filter => s.filter = cycle(&cfg::Filter::ALL, s.filter, dd),
            Bezel => s.bezel = !s.bezel,
            CrtCurvature => s.crt_curvature = !s.crt_curvature,
            Vsync => s.vsync = !s.vsync,
            Brightness => s.brightness = step_u8(s.brightness, dd, 5, 50, 150),
            Volume => s.volume = step_u8(s.volume, dd, 5, 0, 100),
            AttractSound => s.game.attract_sound = !s.game.attract_sound,
            InputDevice => s.input = cycle(&InputMode::ALL, s.input, dd),
            SteerAssist => s.pad.steer_assist = !s.pad.steer_assist,
            StickDeadzone => {
                s.pad.stick_deadzone =
                    (i32::from(s.pad.stick_deadzone) + dd * 500).clamp(0, 16_000) as u16
            }
            StickLinearity => {
                s.pad.stick_linearity = step_u8(s.pad.stick_linearity, dd, 10, 0, 100)
            }
            TriggerDeadzone => {
                s.pad.trigger_deadzone = step_u8(s.pad.trigger_deadzone, dd, 4, 0, 64)
            }
            TriggerLinearity => {
                s.pad.trigger_linearity = step_u8(s.pad.trigger_linearity, dd, 10, 0, 100)
            }
            Rumble => s.ffb.rumble = if s.ffb.rumble > 0 { 0 } else { 100 },
            FfbStrength => s.ffb.strength = step_u8(s.ffb.strength, dd, 25, 0, 100),
            FfbInvert => s.ffb.invert = !s.ffb.invert,
            SteeringLock => s.wheel.lock = s.wheel.lock.step(dd),
            InvertAccel => s.wheel.invert_accel = !s.wheel.invert_accel,
            InvertBrake => s.wheel.invert_brake = !s.wheel.invert_brake,
            Region => {
                s.region = if s.region == "radm" {
                    "radmu".into()
                } else {
                    "radm".into()
                }
            }
            Difficulty => s.game.difficulty = step_u8(s.game.difficulty, dd, 1, 1, 16),
            Preset => {
                let next = match cfg::Preset::of(s) {
                    Some(p) => cycle(&cfg::Preset::ALL, p, dd),
                    None if dd > 0 => cfg::Preset::Easy,
                    None => cfg::Preset::Arcade,
                };
                next.apply(s);
            }
            Timer => s.game.timer = step_u8(s.game.timer, dd, 1, 40, 60),
            RaceTime => s.race_time = cycle(&cfg::RaceTime::ALL, s.race_time, dd),
            RivalSpeed => s.game.rival_speed = step_u8(s.game.rival_speed, dd, 1, 1, 16),
            PoliceSpeed => s.game.police_speed = step_u8(s.game.police_speed, dd, 1, 1, 16),
            FreePlay => s.game.free_play = !s.game.free_play,
            CreditsStart => s.game.credits_to_start = step_u8(s.game.credits_to_start, dd, 1, 1, 5),
            CreditsContinue => {
                s.game.credits_to_continue = step_u8(s.game.credits_to_continue, dd, 1, 1, 5)
            }
            WheelSetup if d == 0 => {
                changed = false;
                self.start_wizard();
            }
            TestFfb if d == 0 => {
                changed = false;
                self.input.send(Command::TestFfb);
            }
            ResetWheel if d == 0 => {
                changed = false;
                self.profile = None;
                self.save_profile();
                self.say("WHEEL BACK TO AUTOMATIC SETUP", OK);
            }
            Map(action) if d == 0 => {
                changed = false;
                self.start_remap(action);
            }
            ResetPadMap if d == 0 => {
                s.pad.map = PadMap::default();
                self.say("GAMEPAD BUTTONS BACK TO DEFAULTS", OK);
            }
            RomFolder if d == 0 => {
                changed = false;
                let text = self.rom_dir_text.trim();
                self.settings.rom_dir = (!text.is_empty()).then(|| text.into());
                self.save();
                self.rescan_roms();
            }
            Rescan if d == 0 => {
                changed = false;
                self.rescan_roms();
            }
            _ => changed = false,
        }
        if changed {
            if item == Item::Region {
                self.rescan_roms();
            }
            self.save();
        }
    }

    fn value(&self, item: Item) -> Option<String> {
        use Item::*;
        let s = &self.settings;
        Some(match item {
            Display => match s.display {
                cfg::Display::Fullscreen => "FULLSCREEN".into(),
                cfg::Display::Window => "WINDOW".into(),
            },
            Resolution => s.resolution.label().to_uppercase(),
            Aspect => s.aspect.label().to_uppercase(),
            WideTarget => s.wide_target.label().into(),
            Filter => s.filter.label().to_uppercase(),
            Bezel => on_off(s.bezel),
            CrtCurvature => on_off(s.crt_curvature),
            Vsync => on_off(s.vsync),
            Brightness => format!("{}%", s.brightness),
            Volume => format!("{}%", s.volume),
            AttractSound => on_off(s.game.attract_sound),
            InputDevice => s.input.label().to_uppercase(),
            SteerAssist => steering_label(s.pad.steer_assist),
            StickDeadzone => format!("{}", s.pad.stick_deadzone),
            StickLinearity => wheel::linearity_label(s.pad.stick_linearity).to_uppercase(),
            TriggerDeadzone => format!("{}", s.pad.trigger_deadzone),
            TriggerLinearity => wheel::linearity_label(s.pad.trigger_linearity).to_uppercase(),
            Rumble => on_off(s.ffb.rumble > 0),
            FfbStrength => {
                if s.ffb.strength == 0 {
                    "OFF".into()
                } else {
                    format!("{}%", s.ffb.strength)
                }
            }
            FfbInvert => on_off(s.ffb.invert),
            SteeringLock => s.wheel.lock.label().to_uppercase(),
            InvertAccel => on_off(s.wheel.invert_accel),
            InvertBrake => on_off(s.wheel.invert_brake),
            Region => {
                if s.region == "radmu" {
                    "US".into()
                } else {
                    "WORLD".into()
                }
            }
            Difficulty => format!("{}", s.game.difficulty),
            Preset => cfg::Preset::of(s)
                .map_or("CUSTOM", |p| p.label())
                .to_uppercase(),
            Timer => format!("{} SEC", s.game.timer),
            RaceTime => s.race_time.label().to_uppercase(),
            RivalSpeed => format!("{}", s.game.rival_speed),
            PoliceSpeed => format!("{}", s.game.police_speed),
            FreePlay => on_off(s.game.free_play),
            CreditsStart => format!("{}", s.game.credits_to_start),
            CreditsContinue => format!("{}", s.game.credits_to_continue),
            Map(action) => {
                if let Some(remap) = &self.remap
                    && remap.action == action
                {
                    return self.remap_status.clone();
                }
                if self.screen == Screen::PadMap {
                    s.pad.map.describe(action)
                } else {
                    match &self.snapshot.profile {
                        Some(profile) => profile.describe(action),
                        None => "NO WHEEL".into(),
                    }
                }
            }
            _ => return None,
        })
    }

    fn handle_nav(&mut self, nav: Nav, ctx: &egui::Context) {
        if self.screen == Screen::Running
            || self.remap.is_some()
            || self.quiet_until.is_some_and(|t| Instant::now() < t)
        {
            return;
        }
        if self.screen == Screen::Wizard {
            if nav == Nav::Back && self.wizard.as_ref().is_none_or(|w| w.done_at.is_none()) {
                self.say("WHEEL SETUP CANCELLED", DIM);
                self.back(ctx);
            } else if self.wizard.as_ref().is_some_and(|w| w.done_at.is_some()) {
                self.back(ctx);
            }
            return;
        }
        let list = items(self.screen, &self.settings);
        if list.is_empty() {
            return;
        }
        self.selected = self.selected.min(list.len() - 1);
        match nav {
            Nav::Up => self.selected = (self.selected + list.len() - 1) % list.len(),
            Nav::Down => self.selected = (self.selected + 1) % list.len(),
            Nav::Left => self.act(list[self.selected], -1, ctx),
            Nav::Right => self.act(list[self.selected], 1, ctx),
            Nav::Confirm => self.act(list[self.selected], 0, ctx),
            Nav::Back => self.back(ctx),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // scale everything to the window: designed for 720 lines
        let zoom = ctx.zoom_factor();
        let height = ctx.input(|i| i.viewport_rect().height() * i.pixels_per_point) / zoom;
        ctx.set_zoom_factor((height / 720.0).clamp(0.75, 4.0));

        self.snapshot = self.input.snapshot();
        self.screenshot_step(&ctx);
        self.poll_child();
        self.tick_wizard();
        self.tick_remap(&ctx);

        let mut navs: Vec<Nav> = self.input.nav.try_iter().collect();
        let typing = self.screen == Screen::Roms && ctx.memory(|m| m.focused().is_some());
        if !typing {
            ctx.input(|i| {
                for (key, nav) in [
                    (egui::Key::ArrowUp, Nav::Up),
                    (egui::Key::ArrowDown, Nav::Down),
                    (egui::Key::ArrowLeft, Nav::Left),
                    (egui::Key::ArrowRight, Nav::Right),
                    (egui::Key::Enter, Nav::Confirm),
                    (egui::Key::Space, Nav::Confirm),
                    (egui::Key::Escape, Nav::Back),
                    (egui::Key::Backspace, Nav::Back),
                ] {
                    if i.key_pressed(key) {
                        navs.push(nav);
                    }
                }
            });
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F11)) {
            let full = ctx.input(|i| i.viewport().fullscreen.unwrap_or(false));
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(!full));
        }
        for nav in navs {
            self.handle_nav(nav, &ctx);
        }

        let rect = ui.max_rect();
        let painter = ui.painter().clone();
        self.tick_backdrop(&ctx);
        draw_background(
            &painter,
            rect,
            ctx.input(|i| i.time),
            self.backdrop.as_ref(),
        );
        self.draw(ui, rect);

        ctx.request_repaint_after(Duration::from_millis(16));
    }
}

/// The menu backdrop, cover-fit and centred; its pixels keep the game's
/// pixel aspect. Falls back to a drawn sunset while there is none.
fn draw_background(p: &egui::Painter, r: Rect, time: f64, backdrop: Option<&egui::TextureHandle>) {
    if let Some(tex) = backdrop {
        let aspect = art::GAME_WIDTH as f32 * radmobile_core::mame::PIXEL_ASPECT as f32
            / art::BACKDROP_ROWS as f32;
        let size = if r.width() / r.height() > aspect {
            Vec2::new(r.width(), r.width() / aspect)
        } else {
            Vec2::new(r.height() * aspect, r.height())
        };
        let shown = Rect::from_center_size(r.center(), size);
        let uv = Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0));
        p.image(tex.id(), shown, uv, Color32::WHITE);
        return;
    }
    draw_sunset(p, r, time);
}

fn draw_sunset(p: &egui::Painter, r: Rect, time: f64) {
    let horizon = r.top() + r.height() * 0.58;
    // sky: vertical gradient in bands
    let bands = 48;
    for i in 0..bands {
        let t0 = i as f32 / bands as f32;
        let y0 = r.top() + (horizon - r.top()) * t0;
        let y1 = r.top() + (horizon - r.top()) * (i + 1) as f32 / bands as f32;
        let c = if t0 < 0.55 {
            lerp(SKY_TOP, SKY_MID, t0 / 0.55)
        } else {
            lerp(SKY_MID, HORIZON, (t0 - 0.55) / 0.45)
        };
        p.rect_filled(Rect::from_x_y_ranges(r.x_range(), y0..=y1 + 1.0), 0.0, c);
    }
    // the sun
    let sun = Pos2::new(r.center().x, horizon);
    let radius = r.height() * 0.2;
    for i in 0..14 {
        let y = horizon - radius + i as f32 * radius / 14.0;
        let half = (radius * radius - (horizon - y).powi(2)).max(0.0).sqrt();
        let h = radius / 14.0 * if i % 3 == 2 { 0.45 } else { 1.0 };
        p.rect_filled(
            Rect::from_min_max(Pos2::new(sun.x - half, y), Pos2::new(sun.x + half, y + h)),
            0.0,
            lerp(GOLD, rgb(palette::ORANGE), i as f32 / 14.0),
        );
    }
    // ground with a moving perspective grid
    p.rect_filled(
        Rect::from_min_max(Pos2::new(r.left(), horizon), r.max),
        0.0,
        GROUND,
    );
    let grid = Stroke::new(1.5, GRID.gamma_multiply(0.55));
    let lines = 22;
    for i in -lines..=lines {
        let x = sun.x + i as f32 * r.width() / 10.0;
        p.line_segment(
            [
                Pos2::new(sun.x + i as f32 * 6.0, horizon),
                Pos2::new(x, r.bottom()),
            ],
            grid,
        );
    }
    let scroll = (time * 0.6).fract() as f32;
    for i in 0..14 {
        let z = (i as f32 + 1.0 - scroll) / 14.0;
        let y = horizon + (r.bottom() - horizon) * z * z;
        p.line_segment([Pos2::new(r.left(), y), Pos2::new(r.right(), y)], grid);
    }
    p.line_segment(
        [Pos2::new(r.left(), horizon), Pos2::new(r.right(), horizon)],
        Stroke::new(2.0, GOLD),
    );
}

fn lerp(a: Color32, b: Color32, t: f32) -> Color32 {
    let t = t.clamp(0.0, 1.0);
    let m = |x: u8, y: u8| (f32::from(x) + (f32::from(y) - f32::from(x)) * t) as u8;
    Color32::from_rgb(m(a.r(), b.r()), m(a.g(), b.g()), m(a.b(), b.b()))
}

fn text(p: &egui::Painter, pos: Pos2, align: Align2, s: &str, size: f32, color: Color32) -> Rect {
    p.text(
        pos + Vec2::splat(size * 0.08),
        align,
        s,
        FontId::proportional(size),
        Color32::from_black_alpha(160),
    );
    p.text(pos, align, s, FontId::proportional(size), color)
}

fn bar(p: &egui::Painter, r: Rect, fill: f32, centred: bool, color: Color32) {
    p.rect_filled(r, CornerRadius::same(3), Color32::from_black_alpha(140));
    p.rect_stroke(
        r,
        CornerRadius::same(3),
        Stroke::new(1.0, DIM.gamma_multiply(0.6)),
        StrokeKind::Inside,
    );
    let inner = if centred {
        let mid = r.center().x;
        let x = mid + fill.clamp(-1.0, 1.0) * r.width() / 2.0;
        Rect::from_x_y_ranges(mid.min(x)..=mid.max(x), r.y_range())
    } else {
        Rect::from_min_size(
            r.min,
            Vec2::new(r.width() * fill.clamp(0.0, 1.0), r.height()),
        )
    };
    p.rect_filled(inner.shrink(2.0), CornerRadius::same(2), color);
    if centred {
        p.line_segment(
            [
                Pos2::new(r.center().x, r.top()),
                Pos2::new(r.center().x, r.bottom()),
            ],
            Stroke::new(1.0, TEXT),
        );
    }
}

impl App {
    fn draw(&mut self, ui: &mut egui::Ui, r: Rect) {
        let p = ui.painter().clone();
        // designed for 16:9; a narrower window scales down instead of overlapping
        let unit = (r.height() / 100.0).min(r.width() / 177.0);

        // header bar
        let header = Rect::from_min_size(r.min, Vec2::new(r.width(), unit * 13.0));
        let r_width = r.width();
        p.rect_filled(header, 0.0, Color32::from_black_alpha(150));
        p.line_segment(
            [header.left_bottom(), header.right_bottom()],
            Stroke::new(unit * 0.4, GOLD),
        );
        // the logo's colours: chrome "RAD", gold "MOBILE"
        let rad = text(
            &p,
            Pos2::new(r.left() + unit * 4.0, header.center().y),
            Align2::LEFT_CENTER,
            "RAD ",
            unit * 7.5,
            CHROME,
        );
        text(
            &p,
            Pos2::new(rad.right(), header.center().y),
            Align2::LEFT_CENTER,
            "MOBILE",
            unit * 7.5,
            GOLD,
        );
        text(
            &p,
            Pos2::new(r.right() - unit * 4.0, header.center().y),
            Align2::RIGHT_CENTER,
            self.screen.title(),
            unit * 3.6,
            TEXT,
        );

        // content window
        let side = matches!(
            self.screen,
            Screen::Wheel | Screen::Controller | Screen::PadMap | Screen::WheelMap
        );
        let content_w = if side {
            r_width * 0.5
        } else {
            (r_width * 0.56).max(unit * 100.0).min(r_width - unit * 8.0)
        };
        let content = Rect::from_min_size(
            Pos2::new(r.left() + unit * 4.0, header.bottom() + unit * 4.0),
            Vec2::new(
                content_w,
                r.bottom() - unit * 20.0 - (header.bottom() + unit * 4.0),
            ),
        );
        p.rect_filled(content, CornerRadius::same(6), PANEL);
        p.rect_stroke(
            content,
            CornerRadius::same(6),
            Stroke::new(1.5, GRID.gamma_multiply(0.7)),
            StrokeKind::Inside,
        );

        match self.screen {
            Screen::Running => self.draw_running(&p, content, unit),
            Screen::Wizard => self.draw_wizard(&p, content, unit),
            _ => self.draw_menu(ui, content, unit),
        }
        if side {
            let monitor = Rect::from_min_size(
                Pos2::new(content.right() + unit * 3.0, content.top()),
                Vec2::new(r.right() - content.right() - unit * 7.0, content.height()),
            );
            self.draw_monitor(&p, monitor, unit);
        }

        // status and tooltip bar
        let tip = Rect::from_min_max(Pos2::new(r.left(), r.bottom() - unit * 16.0), r.max);
        p.rect_filled(tip, 0.0, Color32::from_black_alpha(170));
        p.line_segment(
            [tip.left_top(), tip.right_top()],
            Stroke::new(unit * 0.3, GRID),
        );
        let list = items(self.screen, &self.settings);
        let help_text = match self.screen {
            Screen::Running => "MAME is running. Use your BACK TO MENU button (or Esc) to return.",
            Screen::Wizard => {
                "Steering, rotation and paddles are detected automatically. B: cancel."
            }
            _ => list.get(self.selected).map_or("", |i| help(*i)),
        };
        text(
            &p,
            Pos2::new(r.left() + unit * 4.0, tip.top() + unit * 3.5),
            Align2::LEFT_CENTER,
            help_text,
            unit * 2.8,
            TEXT,
        );

        let (rom_color, device_line) = self.status_lines();
        let message = self
            .message
            .as_ref()
            .filter(|(_, _, at)| at.elapsed() < Duration::from_secs(6));
        let (line, color) = match message {
            Some((m, c, _)) => (m.clone(), *c),
            None => (self.rom.summary(), rom_color),
        };
        text(
            &p,
            Pos2::new(r.left() + unit * 4.0, tip.top() + unit * 8.0),
            Align2::LEFT_CENTER,
            &line,
            unit * 2.6,
            color,
        );
        text(
            &p,
            Pos2::new(r.left() + unit * 4.0, tip.top() + unit * 12.0),
            Align2::LEFT_CENTER,
            &device_line.0,
            unit * 2.6,
            device_line.1,
        );
        let hints = match self.screen {
            _ if self.remap.is_some() => "PRESS THE NEW BUTTON   ESC: CANCEL",
            Screen::PadMap | Screen::WheelMap => "A: REMAP   B: BACK",
            Screen::Running => "",
            Screen::Wizard => "B: CANCEL",
            Screen::Main => "A: SELECT   B: QUIT",
            _ => "A: SELECT   B: BACK   LEFT/RIGHT: CHANGE",
        };
        text(
            &p,
            Pos2::new(r.right() - unit * 4.0, tip.top() + unit * 8.0),
            Align2::RIGHT_CENTER,
            hints,
            unit * 2.6,
            GOLD,
        );
    }

    fn status_lines(&self) -> (Color32, (String, Color32)) {
        let rom_color = if matches!(self.rom, RomStatus::Ready(_)) {
            OK
        } else {
            BAD
        };
        let snap = &self.snapshot;
        let device = if let Some(e) = &snap.error {
            (format!("INPUT UNAVAILABLE: {e}"), BAD)
        } else {
            match snap.active {
                Active::Wheel(i) => {
                    let d = &snap.devices[i];
                    let ffb = if d.ffb.is_some() {
                        "FORCE FEEDBACK READY"
                    } else {
                        "NO FORCE FEEDBACK"
                    };
                    (format!("WHEEL DETECTED: {}  ({ffb})", d.id.name), OK)
                }
                Active::Pad(i) if snap.wheel_missing => (
                    format!(
                        "NO WHEEL FOUND - USING CONTROLLER: {}",
                        snap.devices[i].id.name
                    ),
                    GOLD,
                ),
                Active::Pad(i) => (format!("CONTROLLER: {}", snap.devices[i].id.name), OK),
                Active::None => ("NO WHEEL OR CONTROLLER FOUND - KEYBOARD ONLY".into(), GOLD),
            }
        };
        (rom_color, device)
    }

    fn draw_menu(&mut self, ui: &mut egui::Ui, area: Rect, unit: f32) {
        let p = ui.painter().clone();
        let list = items(self.screen, &self.settings);
        self.selected = self.selected.min(list.len().saturating_sub(1));
        let big = self.screen == Screen::Main;
        let fit = (area.height() - unit * 5.0) / list.len().max(1) as f32;
        let row_h = if big {
            unit * 10.0
        } else {
            (unit * 6.2).min(fit)
        };
        let size = if big {
            unit * 5.5
        } else {
            (unit * 3.4).min(row_h * 0.55)
        };
        let top = area.top() + unit * 3.0;
        let mut clicked = None;
        for (i, item) in list.iter().enumerate() {
            let row = Rect::from_min_size(
                Pos2::new(area.left() + unit * 2.0, top + i as f32 * row_h),
                Vec2::new(area.width() - unit * 4.0, row_h * 0.86),
            );
            let response = ui.interact(
                row,
                ui.scope_id().with(("row", self.screen as u8, i)),
                egui::Sense::click(),
            );
            if response.hovered() && ui.ctx().input(|inp| inp.pointer.delta() != Vec2::ZERO) {
                self.selected = i;
            }
            if response.clicked() {
                clicked = Some(i);
            }
            let selected = i == self.selected;
            if selected {
                p.rect_filled(row, CornerRadius::same(4), SELECT);
                p.rect_filled(
                    Rect::from_min_size(row.min, Vec2::new(unit * 0.8, row.height())),
                    0.0,
                    ACCENT,
                );
            }
            // dark on the gold row (no shadow), light elsewhere
            let write = |pos: Pos2, align: Align2, s: &str, color: Color32| {
                if selected {
                    p.text(pos, align, s, FontId::proportional(size), ON_SELECT);
                } else {
                    text(&p, pos, align, s, size, color);
                }
            };
            write(
                Pos2::new(row.left() + unit * 2.5, row.center().y),
                Align2::LEFT_CENTER,
                label(*item),
                TEXT,
            );
            if let Some(v) = self.value(*item) {
                // a mapping row is not changed with left and right
                let arrows = selected && !matches!(item, Item::Map(_));
                let v = if arrows { format!("<  {v}  >") } else { v };
                write(
                    Pos2::new(row.right() - unit * 2.0, row.center().y),
                    Align2::RIGHT_CENTER,
                    &v,
                    GOLD,
                );
            }
            if *item == Item::RomFolder {
                let edit = Rect::from_min_size(
                    Pos2::new(row.left() + unit * 2.5, row.bottom() + unit * 0.5),
                    Vec2::new(row.width() - unit * 4.0, unit * 4.5),
                );
                let mut child = ui.new_child(egui::UiBuilder::new().max_rect(edit));
                let resp = child.add_sized(
                    edit.size(),
                    egui::TextEdit::singleline(&mut self.rom_dir_text)
                        .hint_text("e.g. /mnt/roms/mame or D:\\MAME\\roms"),
                );
                if resp.lost_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) {
                    clicked = Some(i);
                }
            }
        }
        if self.screen == Screen::Roms {
            let y = top + list.len() as f32 * row_h + unit * 6.0;
            let dirs = session::rom_dirs(&self.paths, &self.settings);
            for (n, d) in dirs.iter().enumerate() {
                text(
                    &p,
                    Pos2::new(area.left() + unit * 4.0, y + n as f32 * unit * 4.0),
                    Align2::LEFT_CENTER,
                    &format!("SEARCHED: {}", d.display()),
                    unit * 2.4,
                    DIM,
                );
            }
        }
        if let Some(i) = clicked {
            self.selected = i;
            let ctx = ui.ctx().clone();
            self.act(list[i], 0, &ctx);
        }
    }

    fn draw_running(&self, p: &egui::Painter, area: Rect, unit: f32) {
        text(
            p,
            area.center() - Vec2::new(0.0, unit * 6.0),
            Align2::CENTER_CENTER,
            "RACING!",
            unit * 9.0,
            GOLD,
        );
        let tele = match self.snapshot.telemetry {
            Some(t) if t.playing => format!(
                "SPEED {:>3}   FFB {:+.2}",
                t.speed as i32, self.snapshot.ffb.constant
            ),
            Some(_) => "ATTRACT MODE - PRESS START".into(),
            None => "WAITING FOR MAME…".into(),
        };
        text(
            p,
            area.center() + Vec2::new(0.0, unit * 4.0),
            Align2::CENTER_CENTER,
            &tele,
            unit * 3.4,
            TEXT,
        );
    }

    fn draw_wizard(&self, p: &egui::Painter, area: Rect, unit: f32) {
        let Some(run) = &self.wizard else { return };
        text(
            p,
            Pos2::new(area.left() + unit * 4.0, area.top() + unit * 6.0),
            Align2::LEFT_CENTER,
            "DIRECTINPUT WHEEL SETUP",
            unit * 3.6,
            GOLD,
        );
        text(
            p,
            Pos2::new(area.left() + unit * 4.0, area.top() + unit * 11.0),
            Align2::LEFT_CENTER,
            &format!("WHEEL: {}", run.devices[0].1.name),
            unit * 2.6,
            DIM,
        );
        let (line, sub, fill) = match &run.status {
            Status::WarmingUp => ("GET READY…".to_string(), String::new(), 0.0),
            Status::Waiting {
                step,
                index,
                remaining_ms,
            } => (
                step.prompt().to_uppercase(),
                format!(
                    "STEP {} OF {}   TIMEOUT SKIPS IN {}s",
                    index + 1,
                    run.wizard.step_count(),
                    remaining_ms / 1000 + 1
                ),
                0.0,
            ),
            Status::Pressing {
                step,
                index,
                travel,
            } => (
                step.prompt().to_uppercase(),
                format!(
                    "STEP {} OF {}   NOW LET GO",
                    index + 1,
                    run.wizard.step_count()
                ),
                *travel,
            ),
            Status::Done(c) => (
                "SETUP COMPLETE".into(),
                if c.skipped.is_empty() {
                    "SAVED TO WHEEL.INI".into()
                } else {
                    format!("SAVED. SKIPPED: {}", c.skipped.len())
                },
                1.0,
            ),
        };
        text(
            p,
            area.center() - Vec2::new(0.0, unit * 4.0),
            Align2::CENTER_CENTER,
            &line,
            unit * 3.6,
            TEXT,
        );
        text(
            p,
            area.center() + Vec2::new(0.0, unit * 2.0),
            Align2::CENTER_CENTER,
            &sub,
            unit * 2.6,
            DIM,
        );
        let b = Rect::from_center_size(
            area.center() + Vec2::new(0.0, unit * 9.0),
            Vec2::new(area.width() * 0.7, unit * 3.0),
        );
        bar(p, b, fill, false, OK);
        if run.done_at.is_some() {
            text(
                p,
                area.center() + Vec2::new(0.0, unit * 16.0),
                Align2::CENTER_CENTER,
                "PRESS ANY MENU BUTTON",
                unit * 2.6,
                GOLD,
            );
        }
    }

    /// Live readings: what the game will get, and the forces sent.
    fn draw_monitor(&self, p: &egui::Painter, area: Rect, unit: f32) {
        p.rect_filled(area, CornerRadius::same(6), PANEL);
        p.rect_stroke(
            area,
            CornerRadius::same(6),
            Stroke::new(1.5, GRID.gamma_multiply(0.7)),
            StrokeKind::Inside,
        );
        let s = &self.snapshot;
        let x = area.left() + unit * 3.0;
        let w = area.width() - unit * 6.0;
        let mut y = area.top() + unit * 5.0;
        text(
            p,
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            "INPUT MONITOR",
            unit * 3.2,
            GOLD,
        );
        y += unit * 6.0;
        let rows: [(&str, f32, bool, Color32); 4] = [
            ("STEERING", s.cabinet.steer, true, SELECT),
            ("ACCELERATOR", s.cabinet.accel, false, OK),
            ("BRAKE", s.cabinet.brake, false, BAD),
            ("FORCE", s.ffb.constant, true, GRID),
        ];
        for (name, v, centred, color) in rows {
            text(
                p,
                Pos2::new(x, y),
                Align2::LEFT_CENTER,
                name,
                unit * 2.4,
                DIM,
            );
            bar(
                p,
                Rect::from_min_size(Pos2::new(x, y + unit * 2.0), Vec2::new(w, unit * 3.0)),
                v,
                centred,
                color,
            );
            y += unit * 8.0;
        }
        let pressed: Vec<&str> = Button::ALL
            .iter()
            .filter(|b| s.cabinet.is_pressed(**b))
            .map(|b| b.label())
            .collect();
        let line = if pressed.is_empty() {
            "BUTTONS: -".to_string()
        } else {
            format!("BUTTONS: {}", pressed.join(", ").to_uppercase())
        };
        text(
            p,
            Pos2::new(x, y),
            Align2::LEFT_CENTER,
            &line,
            unit * 2.4,
            TEXT,
        );
        y += unit * 5.0;
        if let Some(prof) = &s.profile
            && matches!(s.active, Active::Wheel(_))
        {
            let saved = self
                .profile
                .as_ref()
                .is_some_and(|sp| sp.wheel.vid == prof.wheel.vid && sp.wheel.pid == prof.wheel.pid);
            let lines = [
                format!(
                    "ROTATION {}°   LOCK {}",
                    prof.rotation,
                    self.settings.wheel.lock.label()
                ),
                format!(
                    "PEDALS {}",
                    if prof.gas.is_some() {
                        if saved {
                            "FROM WHEEL SETUP"
                        } else {
                            "AUTO-DETECTED"
                        }
                    } else {
                        "NOT FOUND - RUN WHEEL SETUP"
                    }
                ),
            ];
            for l in lines {
                text(p, Pos2::new(x, y), Align2::LEFT_CENTER, &l, unit * 2.2, DIM);
                y += unit * 3.6;
            }
        }
    }
}

/// Load the saved settings and wheel profile (defaults when missing).
pub fn load(paths: &AppPaths) -> (Settings, Option<WheelProfile>) {
    let settings = std::fs::read_to_string(paths.settings())
        .map(|t| Settings::from_ini(&Ini::parse(&t)))
        .unwrap_or_default();
    let profile = std::fs::read_to_string(paths.wheel_profile())
        .ok()
        .and_then(|t| WheelProfile::from_ini(&Ini::parse(&t)));
    (settings, profile)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_menu_ends_with_back_or_quit() {
        let s = Settings::default();
        for screen in [
            Screen::Main,
            Screen::Settings,
            Screen::Video,
            Screen::Sound,
            Screen::Controller,
            Screen::PadMap,
            Screen::Wheel,
            Screen::WheelMap,
            Screen::Game,
            Screen::Roms,
        ] {
            let list = items(screen, &s);
            assert!(
                matches!(list.last(), Some(Item::Back | Item::Quit)),
                "{screen:?}"
            );
            for item in list {
                assert!(!label(item).is_empty());
            }
        }
    }

    #[test]
    fn screen_shape_only_shows_for_wide_pictures() {
        let mut s = Settings::default();
        assert!(!items(Screen::Video, &s).contains(&Item::WideTarget));
        s.aspect = cfg::Aspect::TrueWidescreen;
        assert!(items(Screen::Video, &s).contains(&Item::WideTarget));
    }

    #[test]
    fn values_cycle_and_clamp() {
        assert_eq!(
            cycle(&cfg::Filter::ALL, cfg::Filter::CrtDeluxe, 1),
            cfg::Filter::Sharp
        );
        assert_eq!(
            cycle(&cfg::Filter::ALL, cfg::Filter::Sharp, -1),
            cfg::Filter::CrtDeluxe
        );
        assert_eq!(step_u8(100, 1, 25, 0, 100), 100);
        assert_eq!(step_u8(0, -1, 25, 0, 100), 0);
        assert_eq!(step_u8(50, 1, 25, 0, 100), 75);
    }

    #[test]
    fn screen_names_for_screenshots() {
        assert_eq!(screen_by_name("wheel"), Screen::Wheel);
        assert_eq!(screen_by_name("nonsense"), Screen::Main);
        assert_eq!(Screen::Wizard.parent(), Some(Screen::Wheel));
        assert_eq!(Screen::Video.parent(), Some(Screen::Settings));
        assert_eq!(screen_by_name("padmap"), Screen::PadMap);
        assert_eq!(Screen::PadMap.parent(), Some(Screen::Controller));
        assert_eq!(Screen::WheelMap.parent(), Some(Screen::Wheel));
    }

    #[test]
    fn every_control_can_be_remapped_on_both_devices() {
        let s = Settings::default();
        for screen in [Screen::PadMap, Screen::WheelMap] {
            let list = items(screen, &s);
            for action in Action::ALL {
                assert!(list.contains(&Item::Map(action)), "{screen:?} {action:?}");
            }
        }
        assert!(items(Screen::Controller, &s).contains(&Item::Open(Screen::PadMap)));
        assert!(items(Screen::Wheel, &s).contains(&Item::Open(Screen::WheelMap)));
    }
}
