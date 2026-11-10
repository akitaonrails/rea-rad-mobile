# Third-party components

| Component | Version | Licence | How it is used |
|---|---|---|---|
| [MAME](https://github.com/mamedev/mame) | 0.289 (`vendor/mame`, tag `mame0289`) | GPL-2.0-or-later (as a whole); most files BSD-3-Clause | Built from source with `patches/mame-0289-rea-rad-mobile.patch` (only the Sega System 32 driver) and bundled as `mame/radmobile`. The package ships the patch and MAME's `COPYING`; the source is the upstream tag plus that patch. |
| MAME bgfx shaders and chains (`bgfx/`, `artwork/bgfx/`) | 0.289 | per file, mostly BSD-3-Clause / MIT (see each file and `bgfx/chains/LICENSE`) | Bundled for the Smooth and CRT filters. |
| [SDL2](https://www.libsdl.org) | 2.32.10 (Windows: official MinGW build, pinned in `packaging/sdl2.pin`); Linux: the distribution's, bundled from Debian bookworm | zlib | Wheels, pedals, gamepads and force feedback in the launcher; MAME's Linux front end. |
| SDL2_ttf, FreeType, HarfBuzz (Linux package) | Debian bookworm | zlib / FTL / MIT | MAME's UI fonts on Linux. |
| [egui / eframe](https://github.com/emilk/egui) | 0.37 | MIT OR Apache-2.0 | The menu. |
| Rust crates (`Cargo.lock`) | see lock file | MIT / Apache-2.0 / BSD / zlib | Build dependencies. |

## Inspiration, not code

[Wanszai](https://github.com/wanszai)'s arcade wrappers (Virtua Racing,
Sega Rally Championship, Ridge Racer Collection, Scud Race) are the model
for the front end, the wheel setup and the force feedback behaviour. Their
PC builds were studied statically (docs/WANSZAI_WHEELS.md); no code, art,
font or sound from them is used.

## Never included

No ROM, EEPROM dump, artwork or sound from Rad Mobile is part of this
repository or its packages. The EEPROM the launcher writes is built from
the setting values documented in docs/RESEARCH.md.
