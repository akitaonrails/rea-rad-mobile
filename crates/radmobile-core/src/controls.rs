//! The Rad Mobile cabinet's controls, how they travel through the bridge,
//! and the MAME controller file that maps them onto the game.

use rea_launcher::wheel::Control;

use crate::bridge::{AXIS_MAX, AXIS_MIN, Frame};

/// Bridge axis carrying the steering wheel (MAME `JOYCODE_1_XAXIS`).
pub const AXIS_STEER: usize = 0;
/// Bridge axis carrying the accelerator (`JOYCODE_1_YAXIS`).
pub const AXIS_ACCEL: usize = 1;
/// Bridge axis carrying the brake (`JOYCODE_1_ZAXIS`).
pub const AXIS_BRAKE: usize = 2;

/// Cabinet buttons and the MAME functions the launcher can press.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Button {
    /// Headlights (the game's button 1).
    Light,
    /// Windscreen wiper (button 2).
    Wiper,
    Start,
    Coin,
    /// The operator's service button (moves the test-menu cursor).
    Service,
    /// The operator's test switch (enters the test menu).
    Test,
    /// MAME pause.
    Pause,
    /// Leave the game and return to the launcher.
    Exit,
}

impl Button {
    /// In bridge order: [`Button::bit`], MAME's `JOYCODE_1_BUTTONn` and the
    /// controller file follow the declaration order, so never reorder the
    /// enum. The menu's order is [`Control::ALL`].
    pub const ALL: [Button; 8] = [
        Button::Light,
        Button::Wiper,
        Button::Start,
        Button::Coin,
        Button::Service,
        Button::Test,
        Button::Pause,
        Button::Exit,
    ];

    /// Bit in the bridge's button word (MAME's `Button n+1`).
    pub fn bit(self) -> u32 {
        1 << (self as u32)
    }

    /// MAME input type token, as used in `.cfg` files.
    pub fn mame_type(self) -> &'static str {
        match self {
            Button::Light => "P1_BUTTON1",
            Button::Wiper => "P1_BUTTON2",
            Button::Start => "START1",
            Button::Coin => "COIN1",
            Button::Service => "SERVICE1",
            Button::Test => "SERVICE",
            Button::Pause => "UI_PAUSE",
            Button::Exit => "UI_CANCEL",
        }
    }

    /// The game's own input (port tag, bit) for a button its MAME driver
    /// gives a fixed key (`PORT_CODE`). A fixed key beats the defaults in
    /// the controller file's "default" section, so these inputs are mapped
    /// again under the game's own name.
    pub fn radm_field(self) -> Option<(&'static str, u32)> {
        match self {
            Button::Light => Some((":mainpcb:P1_A", 0x02)),
            Button::Wiper => Some((":mainpcb:P1_A", 0x04)),
            _ => None,
        }
    }

    /// The name in settings files.
    pub fn key(self) -> &'static str {
        match self {
            Button::Light => "light",
            Button::Wiper => "wiper",
            Button::Start => "start",
            Button::Coin => "coin",
            Button::Service => "service",
            Button::Test => "test",
            Button::Pause => "pause",
            Button::Exit => "exit",
        }
    }

    /// The keyboard key that keeps working next to the bridge.
    pub fn mame_key(self) -> &'static str {
        match self {
            Button::Light => "KEYCODE_SPACE",
            Button::Wiper => "KEYCODE_LSHIFT",
            Button::Start => "KEYCODE_1",
            Button::Coin => "KEYCODE_5",
            Button::Service => "KEYCODE_9",
            Button::Test => "KEYCODE_F2",
            Button::Pause => "KEYCODE_P",
            Button::Exit => "KEYCODE_ESC",
        }
    }

    /// Name shown in the menu.
    pub fn label(self) -> &'static str {
        match self {
            Button::Light => "Lights",
            Button::Wiper => "Wiper",
            Button::Start => "Start",
            Button::Coin => "Coin",
            Button::Service => "Service",
            Button::Test => "Test menu",
            Button::Pause => "Pause",
            Button::Exit => "Back to menu",
        }
    }
}

/// The cabinet's buttons for [`rea_launcher::wheel`]: the settings keys
/// (`btn_<key>`, `pad_<key>`), the menu names and the bridge bits.
impl Control for Button {
    /// The menu's order (also the order of the `pad_*` keys in a new
    /// `settings.ini`), which is not the bridge's: Back to menu comes right
    /// after Coin.
    const ALL: &'static [Button] = &[
        Button::Light,
        Button::Wiper,
        Button::Start,
        Button::Coin,
        Button::Exit,
        Button::Pause,
        Button::Service,
        Button::Test,
    ];

    fn key(self) -> &'static str {
        Button::key(self)
    }

    fn label(self) -> &'static str {
        Button::label(self)
    }

    fn bit(self) -> u32 {
        Button::bit(self)
    }
}

/// A cabinet control a player can bind to a pad or a wheel; `Action::all()`
/// is in menu order.
pub type Action = rea_launcher::wheel::Action<Button>;

/// What the menu calls a control.
pub fn action_label(action: Action) -> &'static str {
    match action {
        Action::Accelerate => "ACCELERATOR",
        Action::Brake => "BRAKE",
        Action::Press(Button::Light) => "LIGHTS",
        Action::Press(Button::Wiper) => "WIPER",
        Action::Press(Button::Start) => "START",
        Action::Press(Button::Coin) => "COIN",
        Action::Press(Button::Exit) => "BACK TO MENU",
        Action::Press(Button::Pause) => "PAUSE",
        Action::Press(Button::Service) => "SERVICE",
        Action::Press(Button::Test) => "TEST MENU",
    }
}

/// The cabinet's state after the launcher has applied steering lock,
/// curves and calibration: steering −1..1, pedals 0..1, [`Button::bit`]s.
pub type Cabinet = rea_launcher::wheel::Controls;

/// The cabinet as the bridge carries it to MAME.
pub fn to_frame(cab: &Cabinet) -> Frame {
    let mut frame = Frame::released();
    frame.axes[AXIS_STEER] = to_axis_signed(cab.steer);
    frame.axes[AXIS_ACCEL] = to_axis_pedal(cab.accel);
    frame.axes[AXIS_BRAKE] = to_axis_pedal(cab.brake);
    frame.buttons = cab.buttons;
    frame
}

fn to_axis_signed(v: f32) -> i32 {
    (v.clamp(-1.0, 1.0) * AXIS_MAX as f32).round() as i32
}

/// MAME pedals run from the axis minimum (up) to the maximum (floored).
fn to_axis_pedal(v: f32) -> i32 {
    let span = (AXIS_MAX - AXIS_MIN) as f32;
    (AXIS_MIN as f32 + v.clamp(0.0, 1.0) * span).round() as i32
}

/// The controller file (`ctrlr/rea.cfg`) that maps the bridge joystick
/// onto the cabinet while keeping MAME's keyboard defaults.
pub fn ctrlr_cfg() -> String {
    let mut ports = String::new();
    let analog = [
        (
            "P1_PADDLE",
            "JOYCODE_1_XAXIS",
            "KEYCODE_LEFT",
            "KEYCODE_RIGHT",
        ),
        ("P1_PEDAL", "JOYCODE_1_YAXIS", "", "KEYCODE_LCONTROL"),
        ("P1_PEDAL2", "JOYCODE_1_ZAXIS", "", "KEYCODE_LALT"),
    ];
    for (port, axis, dec, inc) in analog {
        ports.push_str(&format!("      <port type=\"{port}\">\n"));
        ports.push_str(&format!(
            "        <newseq type=\"standard\">{axis}</newseq>\n"
        ));
        if !dec.is_empty() {
            ports.push_str(&format!(
                "        <newseq type=\"decrement\">{dec}</newseq>\n"
            ));
        }
        ports.push_str(&format!(
            "        <newseq type=\"increment\">{inc}</newseq>\n"
        ));
        ports.push_str("      </port>\n");
    }
    let mut fixed_ports = String::new();
    for button in Button::ALL {
        let seq = button_seq(button);
        ports.push_str(&format!(
            "      <port type=\"{}\">\n        <newseq type=\"standard\">{seq}</newseq>\n      </port>\n",
            button.mame_type(),
        ));
        // the field is active low, so its default value is its own bit
        if let Some((tag, bit)) = button.radm_field() {
            fixed_ports.push_str(&format!(
                "      <port tag=\"{tag}\" type=\"{}\" mask=\"{bit}\" defvalue=\"{bit}\">\n        <newseq type=\"standard\">{seq}</newseq>\n      </port>\n",
                button.mame_type(),
            ));
        }
    }
    let mut systems =
        format!("  <system name=\"default\">\n    <input>\n{ports}    </input>\n  </system>\n");
    for set in SETS {
        systems.push_str(&format!(
            "  <system name=\"{set}\">\n    <input>\n{fixed_ports}    </input>\n  </system>\n"
        ));
    }
    format!(
        "<?xml version=\"1.0\"?>\n<!-- written by the Rad Mobile launcher; changes are overwritten -->\n<mameconfig version=\"10\">\n{systems}</mameconfig>\n"
    )
}

/// The MAME sets the launcher runs (World and US); both share one input map.
const SETS: [&str; 2] = ["radm", "radmu"];

/// A button's keyboard key or its bridge joystick button.
fn button_seq(button: Button) -> String {
    format!(
        "{} OR JOYCODE_1_BUTTON{}",
        button.mame_key(),
        button as u32 + 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cabinet_maps_to_mame_axis_ranges() {
        let frame = to_frame(&Cabinet {
            steer: -1.0,
            accel: 1.0,
            brake: 0.0,
            buttons: 0,
        });
        assert_eq!(frame.axes[AXIS_STEER], AXIS_MIN);
        assert_eq!(frame.axes[AXIS_ACCEL], AXIS_MAX);
        assert_eq!(frame.axes[AXIS_BRAKE], AXIS_MIN);

        let half = to_frame(&Cabinet {
            steer: 0.5,
            accel: 0.5,
            ..Default::default()
        });
        assert_eq!(half.axes[AXIS_STEER], 32_768);
        assert_eq!(half.axes[AXIS_ACCEL], 0);
    }

    #[test]
    fn out_of_range_values_are_clamped() {
        let frame = to_frame(&Cabinet {
            steer: 3.0,
            accel: -2.0,
            brake: 9.0,
            buttons: 0,
        });
        assert_eq!(frame.axes[AXIS_STEER], AXIS_MAX);
        assert_eq!(frame.axes[AXIS_ACCEL], AXIS_MIN);
        assert_eq!(frame.axes[AXIS_BRAKE], AXIS_MAX);
    }

    #[test]
    fn buttons_use_distinct_bits_in_order() {
        let mut cab = Cabinet::default();
        cab.press(Button::Light);
        cab.press(Button::Exit);
        assert_eq!(to_frame(&cab).buttons, 0b1000_0001);
        assert!(cab.is_pressed(Button::Exit) && !cab.is_pressed(Button::Start));
    }

    #[test]
    fn the_menu_order_is_not_the_bridge_order() {
        let menu: Vec<_> = Action::all().map(action_label).collect();
        assert_eq!(
            menu,
            [
                "ACCELERATOR",
                "BRAKE",
                "LIGHTS",
                "WIPER",
                "START",
                "COIN",
                "BACK TO MENU",
                "PAUSE",
                "SERVICE",
                "TEST MENU"
            ]
        );
        // every button once in each order; the bits follow the enum
        let mut menu_buttons = <Button as Control>::ALL.to_vec();
        menu_buttons.sort();
        assert_eq!(menu_buttons, Button::ALL);
        for (i, b) in Button::ALL.into_iter().enumerate() {
            assert_eq!(b.bit(), 1 << i);
            assert_eq!(Control::bit(b), b.bit());
        }
    }

    #[test]
    fn ctrlr_file_maps_every_cabinet_input() {
        let cfg = ctrlr_cfg();
        assert!(cfg.contains("<port type=\"P1_PADDLE\">"));
        assert!(cfg.contains("<newseq type=\"standard\">JOYCODE_1_XAXIS</newseq>"));
        assert!(cfg.contains("<newseq type=\"standard\">JOYCODE_1_ZAXIS</newseq>"));
        assert!(cfg.contains("KEYCODE_SPACE OR JOYCODE_1_BUTTON1"));
        assert!(cfg.contains("KEYCODE_ESC OR JOYCODE_1_BUTTON8"));
        let fixed = Button::ALL
            .iter()
            .filter(|b| b.radm_field().is_some())
            .count();
        assert_eq!(
            cfg.matches("<port ").count(),
            3 + Button::ALL.len() + SETS.len() * fixed
        );
    }

    #[test]
    fn ctrlr_file_remaps_the_buttons_the_driver_fixes_to_keys() {
        // MAME's radm driver gives Light and Wiper their own keys, which
        // beat the "default" section: without these the pad never lights up
        let cfg = ctrlr_cfg();
        for set in SETS {
            let section = cfg
                .split(&format!("<system name=\"{set}\">"))
                .nth(1)
                .and_then(|rest| rest.split("</system>").next())
                .unwrap_or_else(|| panic!("no section for {set}"));
            assert!(section.contains(
                "<port tag=\":mainpcb:P1_A\" type=\"P1_BUTTON1\" mask=\"2\" defvalue=\"2\">"
            ));
            assert!(section.contains("KEYCODE_SPACE OR JOYCODE_1_BUTTON1"));
            assert!(section.contains(
                "<port tag=\":mainpcb:P1_A\" type=\"P1_BUTTON2\" mask=\"4\" defvalue=\"4\">"
            ));
            assert!(section.contains("KEYCODE_LSHIFT OR JOYCODE_1_BUTTON2"));
        }
    }
}
