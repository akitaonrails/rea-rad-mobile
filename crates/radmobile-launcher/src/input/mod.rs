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

use radmobile_core::bridge::Bridge;
use radmobile_core::controls::{Button, Cabinet};
use radmobile_core::ffb::{FfbEngine, FfbOutput, Telemetry};
use radmobile_core::settings::{InputMode, Settings};
use radmobile_core::wheel::{self, DeviceId, DeviceState, WheelProfile};

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
    test_until: Option<Instant>,
    nav: NavTracker,
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
        test_until: None,
        nav: NavTracker::default(),
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
                Ok(Command::TestFfb) => {
                    st.test_until = Some(Instant::now() + Duration::from_millis(1500))
                }
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
    st.devices = (0..sdl.device_count())
        .filter_map(sdl::Device::open)
        .collect();
    for d in &st.devices {
        crate::log::line(&format!(
            "input: {:?} \"{}\" {:04x}:{:04x} axes={} buttons={}",
            d.kind,
            d.id.name,
            d.id.vid,
            d.id.pid,
            d.state.axes.len(),
            d.state.buttons.len()
        ));
    }
}

/// The wheel to use: the saved profile's model first, then any wheel.
fn pick_wheel(st: &State) -> Option<usize> {
    let wheels = || {
        st.devices
            .iter()
            .enumerate()
            .filter(|(_, d)| d.kind == Kind::Wheel)
    };
    let saved = st.saved_profile.as_ref().map(|p| &p.wheel);
    wheels()
        .find(|(_, d)| saved.is_some_and(|s| s.vid == d.id.vid && s.pid == d.id.pid))
        .or_else(|| wheels().next())
        .map(|(i, _)| i)
}

/// The aux device of a profile: same GUID and model, or the same model.
fn find_aux(st: &State, aux: &DeviceId, wheel: usize) -> Option<usize> {
    let candidates = || st.devices.iter().enumerate().filter(|(i, _)| *i != wheel);
    candidates()
        .find(|(_, d)| d.id.vid == aux.vid && d.id.pid == aux.pid && d.id.guid == aux.guid)
        .or_else(|| candidates().find(|(_, d)| d.id.vid == aux.vid && d.id.pid == aux.pid))
        .map(|(i, _)| i)
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
            _ => WheelProfile::guess(d.id.clone(), &d.initial),
        }
    });

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
            cabinet = cabinet.merge(wheel::read_pad(&st.settings.pad, &d.pad_state));
        }
    }

    // the game
    let mut telemetry = None;
    if let Some(bridge) = &st.bridge {
        bridge.write(&cabinet.to_frame());
        let seq = bridge.mame_seq();
        if seq != st.last_mame_seq {
            st.last_mame_seq = seq;
            st.last_mame_change = now;
        }
        if now.duration_since(st.last_mame_change) < TELEMETRY_STALE {
            telemetry = read_telemetry(bridge);
        }
    }

    // force feedback
    let mut ffb = st
        .engine
        .update(dt, wheel_steer, telemetry, &st.settings.ffb);
    if let Some(until) = st.test_until {
        if now < until {
            let left = until.duration_since(now).as_millis();
            let push = f32::from(st.settings.ffb.strength.max(25)) / 100.0 * 0.6;
            ffb.constant = if left > 1000 {
                -push
            } else if left > 500 {
                push
            } else {
                0.0
            };
            ffb.rumble = 0.6;
        } else {
            st.test_until = None;
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
        for n in st.nav.update(input, now) {
            let _ = nav_tx.send(n);
        }
    }

    Snapshot {
        devices: st
            .devices
            .iter()
            .map(|d| DeviceInfo {
                id: d.id.clone(),
                kind: d.kind,
                ffb: st
                    .haptic
                    .as_ref()
                    .filter(|(i, _)| *i == d.instance)
                    .map(|(_, h)| h.description.clone()),
            })
            .collect(),
        states: st.devices.iter().map(|d| d.state.clone()).collect(),
        active,
        wheel_missing,
        profile,
        cabinet,
        ffb,
        telemetry,
        error: None,
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
            Some("rea_playing") => t.playing = o.value != 0,
            _ => {}
        }
    }
    found.then_some(t)
}

/// Directions and buttons that move through the menu, from every pad and
/// from the wheel's hat and its bound buttons.
fn nav_input(st: &State, active: Active, profile: Option<&WheelProfile>) -> [bool; 6] {
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
    held
}

/// Turns held inputs into navigation events, with key-repeat on directions.
#[derive(Default)]
struct NavTracker {
    held_since: [Option<Instant>; 6],
    last_repeat: [Option<Instant>; 6],
}

impl NavTracker {
    const NAVS: [Nav; 6] = [
        Nav::Up,
        Nav::Down,
        Nav::Left,
        Nav::Right,
        Nav::Confirm,
        Nav::Back,
    ];

    fn update(&mut self, held: [bool; 6], now: Instant) -> Vec<Nav> {
        let mut out = Vec::new();
        for (i, &down) in held.iter().enumerate() {
            match (down, self.held_since[i]) {
                (true, None) => {
                    self.held_since[i] = Some(now);
                    self.last_repeat[i] = Some(now);
                    out.push(Self::NAVS[i]);
                }
                (true, Some(since)) if i < 4 => {
                    let last = self.last_repeat[i].unwrap_or(since);
                    if now.duration_since(since) >= NAV_REPEAT_DELAY
                        && now.duration_since(last) >= NAV_REPEAT_EVERY
                    {
                        self.last_repeat[i] = Some(now);
                        out.push(Self::NAVS[i]);
                    }
                }
                (false, _) => {
                    self.held_since[i] = None;
                    self.last_repeat[i] = None;
                }
                _ => {}
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nav_fires_on_press_and_repeats_directions_only() {
        let mut t = NavTracker::default();
        let t0 = Instant::now();
        let down = [false, true, false, false, true, false];
        assert_eq!(t.update(down, t0), vec![Nav::Down, Nav::Confirm]);
        assert!(t.update(down, t0 + Duration::from_millis(100)).is_empty());
        assert_eq!(t.update(down, t0 + NAV_REPEAT_DELAY), vec![Nav::Down]);
        assert!(
            t.update(down, t0 + NAV_REPEAT_DELAY + Duration::from_millis(50))
                .is_empty()
        );
        assert_eq!(
            t.update(down, t0 + NAV_REPEAT_DELAY + NAV_REPEAT_EVERY),
            vec![Nav::Down]
        );
        assert!(t.update([false; 6], t0 + Duration::from_secs(2)).is_empty());
        assert_eq!(
            t.update(down, t0 + Duration::from_secs(3)),
            vec![Nav::Down, Nav::Confirm]
        );
    }
}
