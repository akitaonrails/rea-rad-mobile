//! Gamepad bindings: which pad buttons and triggers drive each cabinet
//! control, how they are saved, and how a new one is captured in the menu.
//!
//! Steering always comes from the left stick and the d-pad. Every other
//! control can be moved; the defaults are the layout documented on
//! [`crate::wheel::read_pad`]. Back + Start together always leaves the game,
//! whatever is bound.

use crate::controls::{Action, Button};
use rea_launcher::cfg;

use crate::keys::Keys;
use crate::wheel::PadState;

/// A pad button or trigger, as SDL's controller layer names them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PadInput {
    A,
    B,
    X,
    Y,
    LeftShoulder,
    RightShoulder,
    LeftTrigger,
    RightTrigger,
    Back,
    Start,
    Guide,
    LeftStick,
    RightStick,
}

/// A trigger pulled past half way counts as a press.
const TRIGGER_PRESS: f32 = 0.5;

impl PadInput {
    pub const ALL: [PadInput; 13] = [
        PadInput::A,
        PadInput::B,
        PadInput::X,
        PadInput::Y,
        PadInput::LeftShoulder,
        PadInput::RightShoulder,
        PadInput::LeftTrigger,
        PadInput::RightTrigger,
        PadInput::Back,
        PadInput::Start,
        PadInput::Guide,
        PadInput::LeftStick,
        PadInput::RightStick,
    ];

    pub fn label(self) -> &'static str {
        match self {
            PadInput::A => "A",
            PadInput::B => "B",
            PadInput::X => "X",
            PadInput::Y => "Y",
            PadInput::LeftShoulder => "LB",
            PadInput::RightShoulder => "RB",
            PadInput::LeftTrigger => "LT",
            PadInput::RightTrigger => "RT",
            PadInput::Back => "BACK",
            PadInput::Start => "START",
            PadInput::Guide => "GUIDE",
            PadInput::LeftStick => "L3",
            PadInput::RightStick => "R3",
        }
    }

    fn key(self) -> &'static str {
        match self {
            PadInput::A => "a",
            PadInput::B => "b",
            PadInput::X => "x",
            PadInput::Y => "y",
            PadInput::LeftShoulder => "lb",
            PadInput::RightShoulder => "rb",
            PadInput::LeftTrigger => "lt",
            PadInput::RightTrigger => "rt",
            PadInput::Back => "back",
            PadInput::Start => "start",
            PadInput::Guide => "guide",
            PadInput::LeftStick => "l3",
            PadInput::RightStick => "r3",
        }
    }

    fn parse(s: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|i| i.key() == s)
    }

    fn bit(self) -> u16 {
        1 << (self as u16)
    }

    /// How far a trigger is pulled (0..=1, raw), or None for a button.
    pub fn trigger(self, pad: &PadState) -> Option<f32> {
        let raw = match self {
            PadInput::LeftTrigger => pad.left_trigger,
            PadInput::RightTrigger => pad.right_trigger,
            _ => return None,
        };
        Some((f32::from(raw.max(0)) / crate::wheel::RAW_MAX).min(1.0))
    }

    pub fn is_down(self, pad: &PadState) -> bool {
        if let Some(pull) = self.trigger(pad) {
            return pull > TRIGGER_PRESS;
        }
        match self {
            PadInput::A => pad.a,
            PadInput::B => pad.b,
            PadInput::X => pad.x,
            PadInput::Y => pad.y,
            PadInput::LeftShoulder => pad.left_shoulder,
            PadInput::RightShoulder => pad.right_shoulder,
            PadInput::Back => pad.back,
            PadInput::Start => pad.start,
            PadInput::Guide => pad.guide,
            PadInput::LeftStick => pad.left_stick,
            PadInput::RightStick => pad.right_stick,
            PadInput::LeftTrigger | PadInput::RightTrigger => unreachable!("handled above"),
        }
    }
}

/// For every [`Action`], the pad inputs that drive it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PadMap {
    bits: [u16; Action::COUNT],
}

impl Default for PadMap {
    /// Right trigger or A accelerates, left trigger or B brakes, X or LB
    /// lights, Y or RB wiper, Start starts, Back inserts a coin, Guide
    /// returns to the menu.
    fn default() -> Self {
        use PadInput::*;
        let mut map = PadMap {
            bits: [0; Action::COUNT],
        };
        for (action, inputs) in [
            (Action::Accelerate, &[RightTrigger, A][..]),
            (Action::Brake, &[LeftTrigger, B]),
            (Action::Press(Button::Light), &[X, LeftShoulder]),
            (Action::Press(Button::Wiper), &[Y, RightShoulder]),
            (Action::Press(Button::Start), &[Start]),
            (Action::Press(Button::Coin), &[Back]),
            (Action::Press(Button::Exit), &[Guide]),
        ] {
            map.set(action, inputs);
        }
        map
    }
}

impl PadMap {
    pub fn inputs(&self, action: Action) -> impl Iterator<Item = PadInput> + '_ {
        let bits = self.bits[action.index()];
        PadInput::ALL
            .into_iter()
            .filter(move |i| bits & i.bit() != 0)
    }

    pub fn set(&mut self, action: Action, inputs: &[PadInput]) {
        self.bits[action.index()] = inputs.iter().fold(0, |b, i| b | i.bit());
    }

    /// Bind `input` to `action` alone, taking it away from any other
    /// control so one press never does two things.
    pub fn assign(&mut self, action: Action, input: PadInput) {
        for bits in &mut self.bits {
            *bits &= !input.bit();
        }
        self.bits[action.index()] = input.bit();
    }

    /// Whether any input bound to `action` is held.
    pub fn is_down(&self, action: Action, pad: &PadState) -> bool {
        self.inputs(action).any(|i| i.is_down(pad))
    }

    /// What the menu shows: "RT / A", or "-" when unbound.
    pub fn describe(&self, action: Action) -> String {
        let names: Vec<_> = self.inputs(action).map(PadInput::label).collect();
        if names.is_empty() {
            "-".into()
        } else {
            names.join(" / ")
        }
    }

    /// The map written into a settings file's text: `pad_<action> = a,rt`
    /// (or `none`) for every control; every other line stays.
    pub fn to_cfg(&self, existing: &str) -> String {
        let mut text = existing.to_owned();
        for action in Action::ALL {
            let keys: Vec<_> = self.inputs(action).map(PadInput::key).collect();
            let value = if keys.is_empty() {
                "none".to_string()
            } else {
                keys.join(",")
            };
            text = cfg::set_key(&text, &format!("pad_{}", action.key()), &value);
        }
        text
    }

    /// Missing controls keep their default; unknown names are skipped.
    pub fn from_cfg(text: &str) -> Self {
        let keys = Keys::of(text);
        let mut map = PadMap::default();
        for action in Action::ALL {
            if let Some(value) = keys.get(&format!("pad_{}", action.key())) {
                let inputs: Vec<_> = value
                    .split(',')
                    .filter_map(|s| PadInput::parse(s.trim()))
                    .collect();
                map.set(action, &inputs);
            }
        }
        map
    }
}

/// Waiting in the menu for the pad input a control should move to.
#[derive(Debug, Clone, Default)]
pub struct PadCapture {
    started_ms: Option<u64>,
    /// Every input was let go since the capture began (the button that
    /// opened it is still down at first).
    released: bool,
}

/// How long the menu waits for a press before giving up.
pub const CAPTURE_TIMEOUT_MS: u64 = 6_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Capture<T> {
    /// Waiting for every input to be let go first.
    Releasing,
    Waiting {
        remaining_ms: u64,
    },
    Got(T),
    TimedOut,
}

impl PadCapture {
    /// Feed every pad's state; `now_ms` is a monotonic clock.
    pub fn tick(&mut self, now_ms: u64, pads: &[PadState]) -> Capture<PadInput> {
        let started = *self.started_ms.get_or_insert(now_ms);
        let elapsed = now_ms.saturating_sub(started);
        let held = pads
            .iter()
            .find_map(|pad| PadInput::ALL.into_iter().find(|i| i.is_down(pad)));
        if !self.released {
            if held.is_some() {
                return if elapsed >= CAPTURE_TIMEOUT_MS {
                    Capture::TimedOut
                } else {
                    Capture::Releasing
                };
            }
            self.released = true;
        }
        if let Some(input) = held {
            return Capture::Got(input);
        }
        if elapsed >= CAPTURE_TIMEOUT_MS {
            Capture::TimedOut
        } else {
            Capture::Waiting {
                remaining_ms: CAPTURE_TIMEOUT_MS - elapsed,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_the_documented_layout() {
        let map = PadMap::default();
        assert_eq!(map.describe(Action::Accelerate), "A / RT");
        assert_eq!(map.describe(Action::Brake), "B / LT");
        assert_eq!(map.describe(Action::Press(Button::Light)), "X / LB");
        assert_eq!(map.describe(Action::Press(Button::Wiper)), "Y / RB");
        assert_eq!(map.describe(Action::Press(Button::Start)), "START");
        assert_eq!(map.describe(Action::Press(Button::Coin)), "BACK");
        assert_eq!(map.describe(Action::Press(Button::Exit)), "GUIDE");
        assert_eq!(map.describe(Action::Press(Button::Pause)), "-");
    }

    #[test]
    fn every_action_has_its_own_slot_and_key() {
        let mut seen = std::collections::HashSet::new();
        for a in Action::ALL {
            assert!(seen.insert(a.index()), "{a:?}");
            assert!(seen.len() <= Action::COUNT);
        }
        let keys: std::collections::HashSet<_> = Action::ALL.iter().map(|a| a.key()).collect();
        assert_eq!(keys.len(), Action::COUNT);
    }

    #[test]
    fn assigning_moves_the_input_away_from_other_controls() {
        let mut map = PadMap::default();
        map.assign(Action::Press(Button::Light), PadInput::A);
        assert_eq!(map.describe(Action::Press(Button::Light)), "A");
        assert_eq!(map.describe(Action::Accelerate), "RT");
    }

    #[test]
    fn maps_round_trip_through_cfg() {
        let mut map = PadMap::default();
        map.assign(Action::Press(Button::Pause), PadInput::RightStick);
        map.set(Action::Press(Button::Coin), &[]);
        let text = map.to_cfg("# settings.ini\ndifficulty = 3\n");
        assert!(text.starts_with("# settings.ini\ndifficulty = 3\n"));
        assert!(text.contains("\npad_coin = none\n"));
        assert!(text.contains("\npad_accel = a,rt\n"));
        assert_eq!(PadMap::from_cfg(&text), map);
    }

    #[test]
    fn missing_or_unknown_entries_keep_defaults() {
        let map = PadMap::from_cfg("PAD_LIGHT=x,bogus\n");
        assert_eq!(map.describe(Action::Press(Button::Light)), "X");
        assert_eq!(map.describe(Action::Accelerate), "A / RT");
    }

    #[test]
    fn a_trigger_counts_as_pressed_past_half_way() {
        let mut pad = PadState {
            right_trigger: 10_000,
            ..Default::default()
        };
        assert!(!PadInput::RightTrigger.is_down(&pad));
        pad.right_trigger = 30_000;
        assert!(PadInput::RightTrigger.is_down(&pad));
    }

    #[test]
    fn capture_waits_for_the_opening_press_to_be_let_go() {
        let mut cap = PadCapture::default();
        let a = PadState {
            a: true,
            ..Default::default()
        };
        let idle = PadState::default();
        assert_eq!(cap.tick(0, &[a]), Capture::Releasing);
        assert!(matches!(cap.tick(10, &[idle]), Capture::Waiting { .. }));
        let lb = PadState {
            left_shoulder: true,
            ..Default::default()
        };
        assert_eq!(
            cap.tick(20, &[idle, lb]),
            Capture::Got(PadInput::LeftShoulder)
        );
    }

    #[test]
    fn capture_gives_up_after_a_while() {
        let mut cap = PadCapture::default();
        cap.tick(0, &[PadState::default()]);
        assert_eq!(
            cap.tick(CAPTURE_TIMEOUT_MS, &[PadState::default()]),
            Capture::TimedOut
        );
        // a button stuck down from the start times out too
        let mut stuck = PadCapture::default();
        let a = PadState {
            a: true,
            ..Default::default()
        };
        stuck.tick(0, &[a]);
        assert_eq!(stuck.tick(CAPTURE_TIMEOUT_MS, &[a]), Capture::TimedOut);
    }
}
