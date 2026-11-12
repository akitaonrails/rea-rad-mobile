//! Gamepad bindings ([`rea_launcher::wheel::pad_map`]): which pad buttons
//! and triggers drive each cabinet control, how they are saved
//! (`pad_<action>` in `settings.ini`), and how a new one is captured in
//! the menu. Steering always comes from the left stick and the d-pad.
//! Back + Start together always leaves the game, whatever is bound
//! ([`crate::wheel::read_pad`]).

pub use rea_launcher::wheel::pad_map::{
    CAPTURE_TIMEOUT_MS, Capture, PadCapture, PadInput, TRIGGER_PRESS, hat_bits,
};

use crate::controls::{Action, Button};

/// For every cabinet control, the pad inputs that drive it.
pub type PadMap = rea_launcher::wheel::PadMap<Button>;

/// Rad Mobile's layout: right trigger or A accelerates, left trigger or B
/// brakes, X or LB lights, Y or RB wiper, Start starts, Back inserts a
/// coin, Guide returns to the menu. (`PadMap::default()` binds nothing.)
pub fn layout() -> PadMap {
    use PadInput::*;
    PadMap::with(&[
        (Action::Accelerate, &[RightTrigger, A]),
        (Action::Brake, &[LeftTrigger, B]),
        (Action::Press(Button::Light), &[X, LeftShoulder]),
        (Action::Press(Button::Wiper), &[Y, RightShoulder]),
        (Action::Press(Button::Start), &[Start]),
        (Action::Press(Button::Coin), &[Back]),
        (Action::Press(Button::Exit), &[Guide]),
    ])
}

/// The map in a `settings.ini`'s text: controls without a `pad_<action>`
/// key keep [`layout`]'s inputs; unknown input names are skipped.
pub fn from_cfg(text: &str) -> PadMap {
    PadMap::from_cfg(text, &layout())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_layout_is_the_documented_one() {
        let map = layout();
        let described: Vec<String> = Action::all().map(|a| map.describe(a)).collect();
        assert_eq!(
            described,
            [
                "A / RT", "B / LT", "X / LB", "Y / RB", "START", "BACK", "GUIDE", "-", "-", "-"
            ]
        );
    }

    #[test]
    fn missing_or_unknown_entries_keep_the_layout() {
        let map = from_cfg("PAD_LIGHT=x,bogus\n");
        assert_eq!(map.describe(Action::Press(Button::Light)), "X");
        assert_eq!(map.describe(Action::Accelerate), "A / RT");
    }

    #[test]
    fn maps_round_trip_through_cfg() {
        let mut map = layout();
        map.assign(Action::Press(Button::Pause), PadInput::RightStick);
        map.set(Action::Press(Button::Coin), &[]);
        let text = map.to_cfg("# settings.ini\ndifficulty = 3\n");
        assert!(text.starts_with("# settings.ini\ndifficulty = 3\n"));
        assert!(text.contains("\npad_coin = none\n"));
        assert_eq!(from_cfg(&text), map);
    }
}
