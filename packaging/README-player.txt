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
MOZA and others). Steering works at once; pedals are guessed from where
they rest. For the best result:

  SETTINGS > WHEEL SETTINGS > WHEEL SETUP
  press each pedal and button when asked (pedals on a separate USB
  device and button boxes are supported).

STEERING LOCK  270 degrees feels like the arcade cabinet. The launcher
               scales your wheel's full rotation, so leave your wheel's
               own software at its maximum.
FFB STRENGTH   Rad Mobile never had a steering motor; the force is made
               from the car's speed, crashes and the barriers.
INVERT FFB     If the wheel pushes the wrong way.

Gamepads: left stick steers, right trigger accelerates, left trigger
brakes, X or LB lights, Y or RB wiper, Start starts, Back inserts a coin.
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
FILTER         Sharp, Smooth, CRT, CRT Deluxe.


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
data/wheel.ini      what WHEEL SETUP learned (hand edits stay until the
                    setup runs again)
data/launcher.log   what the launcher saw (attach it to bug reports)
data/mame.log       MAME's output

Powered by MAME 0.289 (GPL-2.0-or-later), patched; the patch and
licences are in the "licenses" folder.
