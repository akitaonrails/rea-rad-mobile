//! The input thread: it owns SDL, reads the wheel, pedals and gamepads,
//! writes the cabinet controls into MAME's bridge, and drives force
//! feedback from the telemetry MAME sends back. The menu talks to it with
//! [`Command`]s and reads a [`Snapshot`] of what it sees.

mod haptic;
mod sdl;

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use radmobile_core::assist::Assist;
use radmobile_core::bridge::Bridge;
use radmobile_core::controls::{self, Button, Cabinet};
use radmobile_core::ffb::{self, FfbEngine, FfbOutput, Telemetry};
use radmobile_core::settings::{InputMode, Settings};
use radmobile_core::wheel::{self, DeviceId, DeviceShape, DeviceState, PadState, WheelProfile};
use rea_launcher::pad::{self, PadNav};

pub use sdl::Kind;

const TICK: Duration = Duration::from_millis(4);
/// Telemetry older than this means MAME is not reporting.
const TELEMETRY_STALE: Duration = Duration::from_millis(500);
const NAV_REPEAT_DELAY: Duration = Duration::from_millis(400);
const NAV_REPEAT_EVERY: Duration = Duration::from_millis(120);
/// Devices are rescanned at most this often after a plug or unplug.
const RESCAN_EVERY: Duration = Duration::from_millis(500);

#[derive(Debug, Clone, PartialEq)]
pub struct DeviceInfo {
    pub id: DeviceId,
    pub path: String,
    /// It takes force feedback.
    pub haptic: bool,
    pub kind: Kind,
    /// The force feedback effects found, when it has any.
    pub ffb: Option<String>,
}

/// Which device drives the cabinet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Active {
    #[default]
    None,
    Wheel(usize),
    Pad(usize),
}

/// What the menu shows; refreshed every tick.
#[derive(Debug, Clone, Default)]
pub struct Snapshot {
    pub devices: Vec<DeviceInfo>,
    pub states: Vec<DeviceState>,
    /// Every gamepad's buttons and sticks (for remapping in the menu).
    pub pads: Vec<PadState>,
    pub active: Active,
    /// The wheel mode was chosen but no wheel is connected.
    pub wheel_missing: bool,
    pub profile: Option<WheelProfile>,
    pub cabinet: Cabinet,
    pub ffb: FfbOutput,
    pub telemetry: Option<Telemetry>,
    pub error: Option<String>,
}

/// Menu navigation from wheels and gamepads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Nav {
    Up,
    Down,
    Left,
    Right,
    Confirm,
    Back,
}

pub enum Command {
    Configure {
        settings: Box<Settings>,
        profile: Option<WheelProfile>,
    },
    Attach(Bridge),
    Detach,
    TestFfb,
}

pub struct InputService {
    tx: Sender<Command>,
    pub nav: Receiver<Nav>,
    snapshot: Arc<Mutex<Snapshot>>,
    thread: Option<JoinHandle<()>>,
}

impl InputService {
    pub fn start(settings: Settings, profile: Option<WheelProfile>) -> Self {
        let (tx, rx) = mpsc::channel();
        let (nav_tx, nav) = mpsc::channel();
        let snapshot = Arc::new(Mutex::new(Snapshot::default()));
        let shared = Arc::clone(&snapshot);
        let thread = std::thread::Builder::new()
            .name("input".into())
            .spawn(move || run(rx, nav_tx, shared, settings, profile))
            .expect("spawn input thread");
        Self {
            tx,
            nav,
            snapshot,
            thread: Some(thread),
        }
    }

    pub fn send(&self, cmd: Command) {
        let _ = self.tx.send(cmd);
    }

    pub fn snapshot(&self) -> Snapshot {
        self.snapshot.lock().map(|s| s.clone()).unwrap_or_default()
    }
}

impl Drop for InputService {
    fn drop(&mut self) {
        // closing the channel stops the thread
        let (tx, _) = mpsc::channel();
        drop(std::mem::replace(&mut self.tx, tx));
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

struct State {
    devices: Vec<sdl::Device>,
    haptic: Option<(i32, haptic::Haptic)>,
    settings: Settings,
    saved_profile: Option<WheelProfile>,
    bridge: Option<Bridge>,
    last_mame_seq: u32,
    last_mame_change: Instant,
    engine: FfbEngine,
    /// Steering assist memory for the race in progress.
    assist: Assist,
    /// When the menu's force feedback test began ([`ffb::test_pattern`]).
    test_started: Option<Instant>,
    nav: PadNav,
}

fn run(
    rx: Receiver<Command>,
    nav_tx: Sender<Nav>,
    shared: Arc<Mutex<Snapshot>>,
    settings: Settings,
    profile: Option<WheelProfile>,
) {
    let sdl = match sdl::Sdl::init() {
        Ok(s) => s,
        Err(e) => {
            crate::log::line(&format!("input: {e}"));
            if let Ok(mut s) = shared.lock() {
                s.error = Some(e.to_string());
            }
            // keep answering commands so the menu still works without SDL
            while rx.recv().is_ok() {}
            return;
        }
    };
    let mut st = State {
        devices: Vec::new(),
        haptic: None,
        settings,
        saved_profile: profile,
        bridge: None,
        last_mame_seq: 0,
        last_mame_change: Instant::now(),
        engine: FfbEngine::new(),
        assist: Assist::default(),
        test_started: None,
        nav: PadNav::with_repeat(NAV_REPEAT_DELAY, NAV_REPEAT_EVERY),
    };
    rescan(&sdl, &mut st);
    let mut last = Instant::now();
    let mut last_rescan = Instant::now();
    loop {
        loop {
            match rx.try_recv() {
                Ok(Command::Configure { settings, profile }) => {
                    st.settings = *settings;
                    st.saved_profile = profile;
                }
                Ok(Command::Attach(bridge)) => {
                    st.last_mame_seq = bridge.mame_seq();
                    st.last_mame_change = Instant::now();
                    st.engine = FfbEngine::new();
                    st.bridge = Some(bridge);
                }
                Ok(Command::Detach) => st.bridge = None,
                Ok(Command::TestFfb) => st.test_started = Some(Instant::now()),
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if let Some((_, h)) = st.haptic.as_mut() {
                        h.stop();
                    }
                    return;
                }
            }
        }

        let changed = sdl.pump();
        let lost = st.devices.iter().any(|d| !d.is_attached());
        if (changed || lost) && last_rescan.elapsed() >= RESCAN_EVERY {
            rescan(&sdl, &mut st);
            last_rescan = Instant::now();
        }
        for d in &mut st.devices {
            d.read();
        }

        let now = Instant::now();
        let dt = now.duration_since(last).as_secs_f32();
        last = now;
        let snap = tick(&mut st, dt, now, &nav_tx);
        if let Ok(mut s) = shared.lock() {
            *s = snap;
        }
        std::thread::sleep(TICK);
    }
}

fn rescan(sdl: &sdl::Sdl, st: &mut State) {
    st.haptic = None;
    st.devices.clear();
    let devices: Vec<sdl::Device> = (0..sdl.device_count())
        .filter_map(sdl::Device::open)
        .collect();
    let shapes: Vec<DeviceShape> = devices
        .iter()
        .map(|d| DeviceShape {
            vid: d.id.vid,
            pid: d.id.pid,
            kind: d.kind,
            axes: d.state.axes.len(),
        })
        .collect();
    let spare: Vec<bool> = (0..shapes.len())
        .map(|i| wheel::is_spare_interface(&shapes, i))
        .collect();
    for (d, _) in devices.iter().zip(&spare).filter(|(_, spare)| **spare) {
        crate::log::line(&format!(
            "input: skipped \"{}\" {:04x}:{:04x}, a spare interface of another device",
            d.id.name, d.id.vid, d.id.pid
        ));
    }
    st.devices = devices
        .into_iter()
        .zip(spare)
        .filter_map(|(d, spare)| (!spare).then_some(d))
        .collect();
    for d in &st.devices {
        crate::log::line(&format!(
            "input: {:?} \"{}\" {:04x}:{:04x} axes={} buttons={} ffb={} path={}",
            d.kind,
            d.id.name,
            d.id.vid,
            d.id.pid,
            d.state.axes.len(),
            d.state.buttons.len(),
            d.haptic,
            d.path
        ));
    }
}

/// The wheel to use (see [`wheel::pick_wheel`]), as a device index.
fn pick_wheel(st: &State) -> Option<usize> {
    let wheels: Vec<usize> = (0..st.devices.len())
        .filter(|&i| st.devices[i].kind == Kind::Wheel)
        .collect();
    let candidates: Vec<wheel::WheelCandidate> = wheels
        .iter()
        .map(|&i| {
            let d = &st.devices[i];
            wheel::WheelCandidate {
                vid: d.id.vid,
                pid: d.id.pid,
                path: &d.path,
                haptic: d.haptic,
            }
        })
        .collect();
    let saved = st.saved_profile.as_ref().map(|p| &p.wheel);
    wheel::pick_wheel(&candidates, saved, &st.settings.wheel.device).map(|w| wheels[w])
}

/// The aux device of a profile (see [`wheel::find_aux`]), as a device index.
fn find_aux(st: &State, aux: &DeviceId, wheel: usize) -> Option<usize> {
    let ids: Vec<DeviceId> = st.devices.iter().map(|d| d.id.clone()).collect();
    wheel::find_aux(&ids, aux, wheel)
}

fn tick(st: &mut State, dt: f32, now: Instant, nav_tx: &Sender<Nav>) -> Snapshot {
    let wheel = pick_wheel(st);
    let pad = st.devices.iter().position(|d| d.is_pad());
    let (active, wheel_missing) = match st.settings.input {
        InputMode::Auto => (
            wheel
                .map(Active::Wheel)
                .or(pad.map(Active::Pad))
                .unwrap_or_default(),
            false,
        ),
        InputMode::Wheel => (
            wheel
                .map(Active::Wheel)
                .or(pad.map(Active::Pad))
                .unwrap_or_default(),
            wheel.is_none(),
        ),
        InputMode::Controller => (pad.map(Active::Pad).unwrap_or_default(), false),
    };

    // the profile for the wheel in use: the saved one when it is this model
    let profile = wheel.map(|w| {
        let d = &st.devices[w];
        match &st.saved_profile {
            Some(p) if p.wheel.vid == d.id.vid && p.wheel.pid == d.id.pid => p.clone(),
            _ => wheel::guess(d.id.clone(), &d.initial),
        }
    });

    let telemetry = poll_telemetry(st, now);
    // only a pad in charge of the car is assisted, never one beside a wheel
    let assisted = telemetry
        .filter(|t| t.playing && st.settings.steer_assist && matches!(active, Active::Pad(_)));
    match &assisted {
        Some(t) => st.assist.observe(t),
        None => st.assist = Assist::default(),
    }

    let mut cabinet = Cabinet::default();
    let mut wheel_steer = 0.0;
    if let (Active::Wheel(w), Some(p)) = (active, &profile) {
        let aux = p
            .aux
            .as_ref()
            .and_then(|a| find_aux(st, a, w))
            .map(|i| &st.devices[i].state);
        cabinet = p.read(&st.settings.wheel, &st.devices[w].state, aux);
        wheel_steer = cabinet.steer;
    }
    for d in st.devices.iter().filter(|d| d.is_pad()) {
        let use_pad = match active {
            Active::Pad(_) => true,
            Active::Wheel(_) => st.settings.input == InputMode::Auto,
            Active::None => false,
        };
        if use_pad {
            let mut pad = wheel::read_pad(&st.settings.pad, &d.pad_state);
            if let Some(t) = &assisted {
                pad.steer = st.assist.steer(pad.steer, t);
            }
            cabinet = cabinet.merge(pad);
        }
    }

    if let Some(bridge) = &st.bridge {
        bridge.write(&controls::to_frame(&cabinet));
    }

    // force feedback
    let mut ffb = st
        .engine
        .update(dt, wheel_steer, telemetry, &st.settings.ffb);
    if let Some(started) = st.test_started {
        let elapsed = now.saturating_duration_since(started).as_millis() as u64;
        match ffb::test_pattern(elapsed, &st.settings.ffb) {
            Some(test) => {
                ffb.constant = test.constant;
                ffb.rumble = test.rumble;
            }
            None => st.test_started = None,
        }
    }
    match active {
        Active::Wheel(w) => {
            let instance = st.devices[w].instance;
            if st.haptic.as_ref().is_none_or(|(i, _)| *i != instance) {
                st.haptic = haptic::Haptic::open(st.devices[w].raw()).map(|h| {
                    crate::log::line(&format!(
                        "ffb: {} on \"{}\"",
                        h.description, st.devices[w].id.name
                    ));
                    (instance, h)
                });
            }
            if let Some((_, h)) = st.haptic.as_mut() {
                h.apply(&ffb);
            }
        }
        Active::Pad(p) => {
            if let Some((_, mut h)) = st.haptic.take() {
                h.stop();
            }
            if ffb.rumble > 0.02 {
                st.devices[p].rumble(ffb.rumble, 60);
            }
        }
        Active::None => {}
    }

    // menu navigation (only while no game runs)
    if st.bridge.is_none() {
        let input = nav_input(st, active, profile.as_ref());
        for n in st
            .nav
            .update_at(input, now)
            .into_iter()
            .filter_map(menu_nav)
        {
            let _ = nav_tx.send(n);
        }
    }

    Snapshot {
        devices: st
            .devices
            .iter()
            .map(|d| DeviceInfo {
                id: d.id.clone(),
                path: d.path.clone(),
                haptic: d.haptic,
                kind: d.kind,
                ffb: st
                    .haptic
                    .as_ref()
                    .filter(|(i, _)| *i == d.instance)
                    .map(|(_, h)| h.description.clone()),
            })
            .collect(),
        states: st.devices.iter().map(|d| d.state.clone()).collect(),
        pads: st
            .devices
            .iter()
            .filter(|d| d.is_pad())
            .map(|d| d.pad_state)
            .collect(),
        active,
        wheel_missing,
        profile,
        cabinet,
        ffb,
        telemetry,
        error: None,
    }
}

/// What the game reported for its last frame, while MAME is alive.
fn poll_telemetry(st: &mut State, now: Instant) -> Option<Telemetry> {
    let bridge = st.bridge.as_ref()?;
    let seq = bridge.mame_seq();
    if seq != st.last_mame_seq {
        st.last_mame_seq = seq;
        st.last_mame_change = now;
    }
    if now.duration_since(st.last_mame_change) < TELEMETRY_STALE {
        read_telemetry(bridge)
    } else {
        None
    }
}

fn read_telemetry(bridge: &Bridge) -> Option<Telemetry> {
    let mut t = Telemetry::default();
    let mut found = false;
    for o in bridge.outputs() {
        match o.name.rsplit(':').next() {
            Some("rea_speed") => {
                t.speed = o.value as f32;
                found = true;
            }
            Some("rea_lateral") => t.lateral = o.value as f32,
            Some("rea_heading") => t.heading = o.value as f32,
            Some("rea_playing") => t.playing = o.value != 0,
            Some("rea_frame") => t.frame = o.value as u32,
            Some("rea_bound0") => t.barrier_left = o.value as f32,
            Some("rea_bound1") => t.edge_left = o.value as f32,
            Some("rea_bound2") => t.edge_right = o.value as f32,
            Some("rea_bound3") => t.barrier_right = o.value as f32,
            _ => {}
        }
    }
    found.then_some(t)
}

/// Directions and buttons that move through the menu, from every pad and
/// from the wheel's hat and its bound buttons, as XInput bits for
/// [`PadNav`]: the d-pad, A to confirm and B to go back.
fn nav_input(st: &State, active: Active, profile: Option<&WheelProfile>) -> u16 {
    let mut held = [false; 6];
    for d in st.devices.iter().filter(|d| d.is_pad()) {
        let p = &d.pad_state;
        held[0] |= p.dpad_up || p.left_y < -20_000;
        held[1] |= p.dpad_down || p.left_y > 20_000;
        held[2] |= p.dpad_left || p.left_x < -20_000;
        held[3] |= p.dpad_right || p.left_x > 20_000;
        held[4] |= p.a || p.start;
        held[5] |= p.b || p.back;
    }
    if let (Active::Wheel(w), Some(p)) = (active, profile) {
        let d = &st.devices[w];
        let (hx, hy) = d.hat();
        held[0] |= hy < 0;
        held[1] |= hy > 0;
        held[2] |= hx < 0;
        held[3] |= hx > 0;
        let pressed = |b: Button| {
            p.binding(b).is_some_and(|bind| {
                bind.source == wheel::Source::Wheel && d.state.button(bind.button)
            })
        };
        held[4] |= pressed(Button::Start) || pressed(Button::Light);
        held[5] |= pressed(Button::Exit) || pressed(Button::Wiper);
    }
    const BITS: [u16; 6] = [
        pad::DPAD_UP,
        pad::DPAD_DOWN,
        pad::DPAD_LEFT,
        pad::DPAD_RIGHT,
        pad::A,
        pad::B,
    ];
    held.iter()
        .zip(BITS)
        .filter(|(down, _)| **down)
        .fold(0, |bits, (_, bit)| bits | bit)
}

/// The menu's steps from [`PadNav`]'s: directions repeat after
/// [`NAV_REPEAT_DELAY`], every [`NAV_REPEAT_EVERY`]; A confirms, B goes back.
fn menu_nav(nav: pad::Nav) -> Option<Nav> {
    match nav {
        pad::Nav::Up => Some(Nav::Up),
        pad::Nav::Down => Some(Nav::Down),
        pad::Nav::Left => Some(Nav::Left),
        pad::Nav::Right => Some(Nav::Right),
        pad::Nav::Press => Some(Nav::Confirm),
        pad::Nav::Back => Some(Nav::Back),
        pad::Nav::PreviousTab | pad::Nav::NextTab | pad::Nav::Play => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The menu's navigation as 0.5.0's own tracker did it: on press, and
    /// held directions again after 400 ms, then every 120 ms.
    #[test]
    fn nav_fires_on_press_and_repeats_directions_only() {
        let mut t = PadNav::with_repeat(NAV_REPEAT_DELAY, NAV_REPEAT_EVERY);
        let mut update = |bits: u16, at: Instant| -> Vec<Nav> {
            t.update_at(bits, at)
                .into_iter()
                .filter_map(menu_nav)
                .collect()
        };
        let t0 = Instant::now();
        let down = pad::DPAD_DOWN | pad::A;
        assert_eq!(update(down, t0), vec![Nav::Down, Nav::Confirm]);
        assert!(update(down, t0 + Duration::from_millis(100)).is_empty());
        assert_eq!(update(down, t0 + NAV_REPEAT_DELAY), vec![Nav::Down]);
        assert!(update(down, t0 + NAV_REPEAT_DELAY + Duration::from_millis(50)).is_empty());
        assert_eq!(
            update(down, t0 + NAV_REPEAT_DELAY + NAV_REPEAT_EVERY),
            vec![Nav::Down]
        );
        assert!(update(0, t0 + Duration::from_secs(2)).is_empty());
        assert_eq!(
            update(down, t0 + Duration::from_secs(3)),
            vec![Nav::Down, Nav::Confirm]
        );
        assert_eq!(update(pad::B, t0 + Duration::from_secs(4)), vec![Nav::Back]);
    }
}
