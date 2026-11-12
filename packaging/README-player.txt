RAD MOBILE
==========

Sega's 1990 arcade racer (System 32), on Windows and Linux, with a
proper front end: 4K rendering, true widescreen, CRT filters, steering
wheel and pedal support with force feedback, and the arcade's own
settings. Inspired by Wanszai's arcade ports.

No ROMs are supplied. Bring your own.


GETTING STARTED
---------------
1. Put your Rad Mobile ROM zip in the "roms" folder (any name works).
2. Plug in your wheel or gamepad BEFORE starting.
3. Run RadMobile.exe (Windows) or ./RadMobile (Linux).
4. START GAME. The game runs on Free Play: press Start to race.

Back to the menu during a race: your BACK TO MENU button, Back+Start on
a gamepad, or Esc.


WHEELS
------
Wheels are recognised automatically (Logitech, Thrustmaster, Fanatec,
MOZA and others). Steering works at once, pedals are guessed from where
they rest, and the shift paddles are the lights and the wiper. Start,
Coin and Back to menu differ from wheel to wheel, so give them buttons
once:

  SETTINGS > WHEEL SETTINGS > WHEEL BUTTONS
  pick a control, press its new button (or pedal). Pedals on a separate
  USB device and button boxes work too.

WHEEL SETUP on the same page asks for every pedal and button in turn.
RESET WHEEL TO AUTOMATIC forgets both.

STEERING LOCK  270 degrees feels like the arcade cabinet. The launcher
               scales your wheel's full rotation, so leave your wheel's
               own software at its maximum.
FFB STRENGTH   Rad Mobile never had a steering motor; the force is made
               from the car's speed, crashes and the barriers.
INVERT FFB     If the wheel pushes the wrong way.
WHEEL          Which wheel to use. AUTO picks one with force feedback;
               some wheels (Fanatec CSL DD on Windows) show up twice and
               only one moves. If the indicators don't move, pick the
               other #. RadMobile --list-devices shows every path.

Gamepads: left stick steers, right trigger or A accelerates, left
trigger or B brakes, X or LB lights, Y or RB wiper, Start starts, Back
inserts a coin, Guide (or Back + Start) returns to the menu. Change any
of them in SETTINGS > CONTROLLER SETTINGS > GAMEPAD BUTTONS.
STICK STEERING (CONTROLLER page): ASSISTED, the default, makes the stick
move the car across the road; let go and the car keeps its line, through
bends too. ARCADE makes the stick the cabinet wheel: the car keeps its
angle until you steer back, as on the real machine.
Keyboard: arrows steer, Left Ctrl accelerates, Left Alt brakes,
Space lights, Left Shift wiper, 1 starts, Esc quits.


VIDEO
-----
RESOLUTION     Desktop, 1080p, 1440p or 4K.
ASPECT RATIO   4:3 Original, Stretch, or True widescreen: the game draws
               more of the world on both sides (experimental: objects
               can appear at the very edge of the screen).
CABINET BEZEL  4:3 only: the picture inside an arcade monitor's frame,
               with art after the upright cabinet around it: stars and
               stripes, a star roundel, and the control panel's sunset
               bars with its START, WIPER and LIGHT buttons.
FILTER         Sharp, Smooth, Arcade CRT, Trinitron CRT, CRT, CRT
               Deluxe. Arcade and Trinitron follow Wanszai's ports
               (scanlines, phosphor mask, glow); CRT CURVATURE bends
               their glass.

The menu's background is a snapshot of your own ROM's intro, taken the
first time the ROM is found (data/menu/backdrop.png; delete it to take
it again).


GAME SETTINGS
-------------
The arcade operator's settings: difficulty, starting timer, rival and
police car speed, free play and credits, attract sound. They are
written to the game before every run.
RACE CLOCK     1.5x, 2x or 3x makes every second on the clock last that
               much longer: the start time and every checkpoint bonus.
PRESET         EASY sets the lowest difficulty, the slowest rivals and
               police, 60 seconds and a 2x clock. ARCADE restores the
               factory settings.


FILES
-----
data/settings.ini   your settings
data/wheel.ini      your wheel's pedals and buttons (hand edits stay
                    until you remap them in the menu)
data/launcher.log   what the launcher saw (attach it to bug reports)
data/mame.log       MAME's output

Powered by MAME 0.289 (GPL-2.0-or-later), patched; the patch and
licences are in the "licenses" folder.
