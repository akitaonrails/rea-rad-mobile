//! Rad Mobile's wheels, pedals and pads on [`rea_launcher::wheel`]: the
//! maths, the known-wheel table, profiles, the setup wizard and pad maps
//! are the SDK's (from this launcher, docs/WANSZAI_WHEELS.md §2–3); what
//! stays here is the game's part: the cabinet's buttons ([`Button`]), which
//! of them the shift paddles become (lights and wiper), the wizard's steps
//! and prompts, and the way out of a game on a pad.

use rea_launcher::wheel::pad_map::Exit;
pub use rea_launcher::wheel::{
    Binding, Controls, DeviceFacts, DeviceId, DeviceShape, DeviceState, Kind, PEDAL_DEADZONE,
    PadState, PedalAxis, RAW_MAX, Source, SteeringLock, WheelCandidate, WheelSettings, apply_lock,
    classify, curve, find_aux, guess_pedals, is_spare_interface, known, linearity_label,
    pick_wheel, shape,
};

use crate::controls::Button;

/// Everything the setup wizard learns about a wheel; saved as `wheel.ini`.
pub type WheelProfile = rea_launcher::wheel::WheelProfile<Button>;
/// How a gamepad drives the cabinet (stick and trigger curves, the map).
pub type PadSettings = rea_launcher::wheel::PadSettings<Button>;

/// The shift paddles drive the lights and the wiper (Wanszai's arcade
/// wrappers do the same with their games' two view/gear buttons).
pub const PADDLES: Option<(Button, Button)> = Some((Button::Light, Button::Wiper));

/// The comment at the top of a new `wheel.ini`.
pub const WHEEL_INI_HEADER: &str = "wheel.ini - written by the Rad Mobile launcher's wheel setup\n\
                                    hand edits stay until the setup runs again";

/// A first guess for a wheel nobody has set up: steering on the first
/// axis, pedals from their resting position, paddles from the table
/// (buttons 0 and 1 for a wheel it does not list, as Wanszai's generic
/// row) as lights and wiper.
pub fn guess(wheel: DeviceId, initial: &DeviceState) -> WheelProfile {
    WheelProfile::guess(wheel, initial, PADDLES)
}

/// A gamepad's cabinet controls. The left stick (or the d-pad) steers;
/// everything else follows [`PadSettings::map`], by default right trigger
/// or A accelerates, left trigger or B brakes, X or LB lights, Y or RB
/// wiper, Start starts, Back inserts a coin and Guide returns to the menu.
/// Back + Start together always returns to the menu, and while the menu
/// is asked for, Start and Coin are not pressed.
pub fn read_pad(settings: &PadSettings, pad: &PadState) -> Controls {
    rea_launcher::wheel::read_pad(
        settings,
        pad,
        Some(Exit {
            button: Button::Exit,
            suppress: &[Button::Start, Button::Coin],
        }),
    )
}

/// The wheel setup wizard ([`rea_launcher::wheel::wizard`]) with Rad
/// Mobile's steps and prompts.
pub mod wizard {
    pub use rea_launcher::wheel::wizard::{
        ARM_DELTA, AXIS_TIMEOUT_MS, BUTTON_TIMEOUT_MS, RELEASE_DELTA, WARM_UP_TICKS,
    };

    use crate::controls::Button;

    pub type Step = rea_launcher::wheel::wizard::Step<Button>;
    pub type Captures = rea_launcher::wheel::wizard::Captures<Button>;
    pub type Status = rea_launcher::wheel::wizard::Status<Button>;
    pub type Wizard = rea_launcher::wheel::wizard::Wizard<Button>;

    /// The full setup: the pedals, then the buttons a cabinet needs.
    pub const STEPS: [Step; 6] = [
        Step::Accelerator,
        Step::Brake,
        Step::Button(Button::Light),
        Step::Button(Button::Wiper),
        Step::Button(Button::Start),
        Step::Button(Button::Exit),
    ];

    /// The full setup over [`STEPS`].
    pub fn full(steer_axis: usize) -> Wizard {
        Wizard::new(STEPS.to_vec(), steer_axis)
    }

    /// What the menu asks the player to do.
    pub fn prompt(step: Step) -> String {
        match step {
            Step::Accelerator => "Press the ACCELERATOR all the way, then let go".into(),
            Step::Brake => "Press the BRAKE all the way, then let go".into(),
            Step::Button(b) => format!("Press the button for {}", b.label().to_uppercase()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::controls::Action;

    fn csl() -> DeviceId {
        DeviceId {
            name: "FANATEC CSL DD".into(),
            vid: 0x0eb7,
            pid: 0x0020,
            guid: String::new(),
        }
    }

    #[test]
    fn the_paddles_are_lights_and_wiper() {
        let p = guess(csl(), &DeviceState::default());
        assert_eq!(p.describe(Action::Press(Button::Light)), "BUTTON 1");
        assert_eq!(p.describe(Action::Press(Button::Wiper)), "BUTTON 2");
        let unknown = DeviceId {
            vid: 0x1234,
            ..csl()
        };
        let p = guess(unknown, &DeviceState::default());
        assert_eq!(p.rotation, known::GENERIC_ROTATION);
        assert_eq!(p.describe(Action::Press(Button::Wiper)), "BUTTON 2");
    }

    #[test]
    fn back_and_start_leave_the_game_without_starting_or_coining() {
        let settings = crate::settings::Settings::default().pad;
        let both = PadState {
            back: true,
            start: true,
            ..Default::default()
        };
        let c = read_pad(&settings, &both);
        assert!(c.is_pressed(Button::Exit));
        assert!(!c.is_pressed(Button::Start) && !c.is_pressed(Button::Coin));
        let start = PadState {
            start: true,
            ..Default::default()
        };
        assert!(read_pad(&settings, &start).is_pressed(Button::Start));
    }

    #[test]
    fn the_wizard_asks_for_the_pedals_then_the_cabinet_buttons() {
        let w = wizard::full(0);
        assert_eq!(w.step_count(), 6);
        assert_eq!(w.current(), Some(wizard::Step::Accelerator));
        assert_eq!(
            wizard::prompt(wizard::Step::Button(Button::Exit)),
            "Press the button for BACK TO MENU"
        );
    }
}
