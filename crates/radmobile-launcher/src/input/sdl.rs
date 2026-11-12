//! A thin, safe layer over SDL2's joystick, game controller and haptic
//! APIs. Everything here runs on the input thread, which owns SDL.

use std::ffi::{CStr, CString};
use std::ptr::NonNull;

use radmobile_core::wheel::{DeviceId, DeviceState, PadState, known};
use sdl2::sys;

/// SDL with the joystick, game controller and haptic subsystems, no video.
pub struct Sdl;

impl Sdl {
    pub fn init() -> anyhow::Result<Self> {
        let hints = [
            // MAME's window has focus while we feed it the wheel
            ("SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS", "1"),
            // wheels report through HID; keep SDL's HIDAPI drivers for pads only
            ("SDL_JOYSTICK_HIDAPI_WHEELS", "0"),
            ("SDL_JOYSTICK_RAWINPUT", "0"),
            // SDL would otherwise swallow SIGINT/SIGTERM as a quit event
            ("SDL_NO_SIGNAL_HANDLERS", "1"),
        ];
        for (k, v) in hints {
            let (k, v) = (CString::new(k)?, CString::new(v)?);
            // SAFETY: valid C strings; SDL copies them
            unsafe { sys::SDL_SetHint(k.as_ptr(), v.as_ptr()) };
        }
        let flags = sys::SDL_INIT_JOYSTICK
            | sys::SDL_INIT_GAMECONTROLLER
            | sys::SDL_INIT_HAPTIC
            | sys::SDL_INIT_EVENTS;
        // SAFETY: plain init call
        if unsafe { sys::SDL_Init(flags) } != 0 {
            anyhow::bail!("SDL_Init: {}", error());
        }
        Ok(Sdl)
    }

    /// Process pending device events and refresh every device's state.
    /// Returns true when devices were added or removed.
    pub fn pump(&self) -> bool {
        let mut changed = false;
        // SAFETY: SDL is initialised; the event is plain data
        unsafe {
            sys::SDL_PumpEvents();
            let mut ev = std::mem::zeroed::<sys::SDL_Event>();
            while sys::SDL_PollEvent(&mut ev) != 0 {
                let t = ev.type_;
                if t == sys::SDL_EventType::SDL_JOYDEVICEADDED as u32
                    || t == sys::SDL_EventType::SDL_JOYDEVICEREMOVED as u32
                {
                    changed = true;
                }
            }
            sys::SDL_JoystickUpdate();
            sys::SDL_GameControllerUpdate();
        }
        changed
    }

    pub fn device_count(&self) -> usize {
        // SAFETY: plain query
        unsafe { sys::SDL_NumJoysticks().max(0) as usize }
    }
}

impl Drop for Sdl {
    fn drop(&mut self) {
        // SAFETY: every device wrapper is closed before (they are owned by
        // the input thread's state, dropped first)
        unsafe { sys::SDL_Quit() };
    }
}

pub fn error() -> String {
    // SAFETY: SDL_GetError returns a valid, static C string
    unsafe {
        CStr::from_ptr(sys::SDL_GetError())
            .to_string_lossy()
            .into_owned()
    }
}

/// What kind of device this is, as far as the launcher cares.
pub use radmobile_core::wheel::Kind;

/// An opened joystick, and its game-controller view when SDL has one.
pub struct Device {
    joy: NonNull<sys::SDL_Joystick>,
    pad: Option<NonNull<sys::_SDL_GameController>>,
    pub instance: i32,
    pub id: DeviceId,
    /// The OS device path: tells apart two collections of one wheel.
    pub path: String,
    /// It takes force feedback.
    pub haptic: bool,
    pub kind: Kind,
    pub initial: DeviceState,
    pub state: DeviceState,
    pub pad_state: PadState,
}

// SAFETY: the pointers are only used on the input thread that created them;
// `Device` is moved into that thread's state and never shared.
unsafe impl Send for Device {}

impl Device {
    /// Open device `index` (0..device_count).
    pub fn open(index: usize) -> Option<Self> {
        let index = index as i32;
        // SAFETY: index is in range; null is handled
        unsafe {
            let joy = NonNull::new(sys::SDL_JoystickOpen(index))?;
            let j = joy.as_ptr();
            let name = cstr(sys::SDL_JoystickName(j));
            let mut guid = [0i8; 64];
            sys::SDL_JoystickGetGUIDString(
                sys::SDL_JoystickGetGUID(j),
                guid.as_mut_ptr() as _,
                guid.len() as i32,
            );
            let id = DeviceId {
                name,
                vid: sys::SDL_JoystickGetVendor(j),
                pid: sys::SDL_JoystickGetProduct(j),
                guid: CStr::from_ptr(guid.as_ptr() as _)
                    .to_string_lossy()
                    .into_owned(),
            };
            let is_wheel = known::lookup(id.vid, id.pid).is_some()
                || known::wheel_vendor(id.vid)
                || sys::SDL_JoystickGetType(j) == sys::SDL_JoystickType::SDL_JOYSTICK_TYPE_WHEEL;
            let pad = if !is_wheel && sys::SDL_IsGameController(index) == sys::SDL_bool::SDL_TRUE {
                NonNull::new(sys::SDL_GameControllerOpen(index))
            } else {
                None
            };
            let kind = if is_wheel {
                Kind::Wheel
            } else if pad.is_some() {
                Kind::Pad
            } else {
                Kind::Other
            };
            let axes = sys::SDL_JoystickNumAxes(j).max(0);
            let mut initial = DeviceState {
                axes: Vec::new(),
                buttons: vec![false; sys::SDL_JoystickNumButtons(j).max(0) as usize],
            };
            for a in 0..axes {
                let mut v = 0i16;
                let has =
                    sys::SDL_JoystickGetAxisInitialState(j, a, &mut v) == sys::SDL_bool::SDL_TRUE;
                initial.axes.push(if has {
                    v
                } else {
                    sys::SDL_JoystickGetAxis(j, a)
                });
            }
            let mut dev = Device {
                joy,
                pad,
                instance: sys::SDL_JoystickInstanceID(j),
                id,
                path: cstr(sys::SDL_JoystickPath(j)),
                haptic: sys::SDL_JoystickIsHaptic(j) == 1,
                kind,
                state: initial.clone(),
                initial,
                pad_state: PadState::default(),
            };
            dev.read();
            Some(dev)
        }
    }

    pub fn raw(&self) -> *mut sys::SDL_Joystick {
        self.joy.as_ptr()
    }

    pub fn is_attached(&self) -> bool {
        // SAFETY: the joystick is open
        unsafe { sys::SDL_JoystickGetAttached(self.raw()) == sys::SDL_bool::SDL_TRUE }
    }

    /// Copy the current readings (after `Sdl::pump`).
    pub fn read(&mut self) {
        let j = self.raw();
        // SAFETY: the joystick (and controller) are open
        unsafe {
            for (i, v) in self.state.axes.iter_mut().enumerate() {
                *v = sys::SDL_JoystickGetAxis(j, i as i32);
            }
            for (i, b) in self.state.buttons.iter_mut().enumerate() {
                *b = sys::SDL_JoystickGetButton(j, i as i32) != 0;
            }
            if let Some(pad) = self.pad {
                use sys::SDL_GameControllerAxis as A;
                use sys::SDL_GameControllerButton as B;
                let p = pad.as_ptr();
                let axis = |a: A| sys::SDL_GameControllerGetAxis(p, a);
                let button = |b: B| sys::SDL_GameControllerGetButton(p, b) != 0;
                self.pad_state = PadState {
                    left_x: axis(A::SDL_CONTROLLER_AXIS_LEFTX),
                    left_trigger: axis(A::SDL_CONTROLLER_AXIS_TRIGGERLEFT),
                    right_trigger: axis(A::SDL_CONTROLLER_AXIS_TRIGGERRIGHT),
                    a: button(B::SDL_CONTROLLER_BUTTON_A),
                    b: button(B::SDL_CONTROLLER_BUTTON_B),
                    x: button(B::SDL_CONTROLLER_BUTTON_X),
                    y: button(B::SDL_CONTROLLER_BUTTON_Y),
                    start: button(B::SDL_CONTROLLER_BUTTON_START),
                    back: button(B::SDL_CONTROLLER_BUTTON_BACK),
                    guide: button(B::SDL_CONTROLLER_BUTTON_GUIDE),
                    left_shoulder: button(B::SDL_CONTROLLER_BUTTON_LEFTSHOULDER),
                    right_shoulder: button(B::SDL_CONTROLLER_BUTTON_RIGHTSHOULDER),
                    left_stick: button(B::SDL_CONTROLLER_BUTTON_LEFTSTICK),
                    right_stick: button(B::SDL_CONTROLLER_BUTTON_RIGHTSTICK),
                    dpad_left: button(B::SDL_CONTROLLER_BUTTON_DPAD_LEFT),
                    dpad_right: button(B::SDL_CONTROLLER_BUTTON_DPAD_RIGHT),
                    dpad_up: button(B::SDL_CONTROLLER_BUTTON_DPAD_UP),
                    dpad_down: button(B::SDL_CONTROLLER_BUTTON_DPAD_DOWN),
                    left_y: axis(A::SDL_CONTROLLER_AXIS_LEFTY),
                };
            }
        }
    }

    /// The first hat's direction as (x, y), each −1, 0 or 1 (y down).
    pub fn hat(&self) -> (i8, i8) {
        // SAFETY: the joystick is open
        let h = unsafe {
            if sys::SDL_JoystickNumHats(self.raw()) < 1 {
                return (0, 0);
            }
            u32::from(sys::SDL_JoystickGetHat(self.raw(), 0))
        };
        let x = if h & sys::SDL_HAT_LEFT != 0 {
            -1
        } else if h & sys::SDL_HAT_RIGHT != 0 {
            1
        } else {
            0
        };
        let y = if h & sys::SDL_HAT_UP != 0 {
            -1
        } else if h & sys::SDL_HAT_DOWN != 0 {
            1
        } else {
            0
        };
        (x, y)
    }

    pub fn is_pad(&self) -> bool {
        self.pad.is_some()
    }

    /// Gamepad rumble, both motors at `strength` (0..1) for `ms`.
    pub fn rumble(&self, strength: f32, ms: u32) {
        if let Some(pad) = self.pad {
            let level = (strength.clamp(0.0, 1.0) * 65535.0) as u16;
            // SAFETY: the controller is open
            unsafe { sys::SDL_GameControllerRumble(pad.as_ptr(), level, level, ms) };
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        // SAFETY: we opened these
        unsafe {
            if let Some(pad) = self.pad {
                sys::SDL_GameControllerClose(pad.as_ptr());
            }
            sys::SDL_JoystickClose(self.raw());
        }
    }
}

fn cstr(p: *const std::os::raw::c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        // SAFETY: SDL returns a valid C string or null
        unsafe { CStr::from_ptr(p).to_string_lossy().into_owned() }
    }
}
