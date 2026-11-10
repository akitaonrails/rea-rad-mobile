# The bundled MAME

`vendor/mame` is upstream MAME at tag `mame0289` (shallow submodule).
Everything we change is in `patches/mame-0289-rea-rad-mobile.patch`,
applied by `scripts/build-mame.sh`. Changes are marked `rea-rad-mobile`.

| Change | Files | Why |
|---|---|---|
| `-joystickprovider reabridge` | `osd/modules/input/input_reabridge.cpp`, `osd/modules/lib/reabridge.h` | The launcher owns the wheel; MAME reads finished cabinet values from shared memory as one joystick |
| `-output reabridge` | `osd/modules/output/reabridge.cpp` | Lamps and telemetry back to the launcher for force feedback |
| Rad Mobile telemetry outputs | `mame/sega/segas32.cpp/.h`, `segas32_v.cpp` | `rea_speed`, `rea_lateral`, `rea_heading`, `rea_playing` from work RAM every vblank (docs/RAM_MAP.md) |
| Steering rate limit | `mame/sega/segas32.cpp/.h` | Rad Mobile ignores steering samples that jump more than 0x40 (MAME Testers 08052), so a released stick kept steering; the ADC now gets the control moved at most 0x30 a sample, as fast as a real wheel |
| Optional factory EEPROM | `mame/sega/segas32.cpp` | Most romsets lack `eeprom-radm.ic76`; the region is blank (0xFF) and the game initialises it |
| True widescreen (`REA_S32_WIDE_MARGIN`) | `mame/sega/segas32_v.cpp`, `segas32.h`, screen size in `segas32.cpp` | Wider visible area with tilemaps, sprites and clip windows shifted (docs/RESEARCH.md) |
| No startup boxes (`REA_SKIP_STARTUP_SCREENS`) | `frontend/mame/ui/ui.cpp`, `emu/romload.cpp` | MAME's "imperfect graphics" box waits for a key; the launcher is the front end |
| MinGW audio property keys | `osd/modules/sound/mmdevice_helpers.cpp`, `scripts/src/osd/windows.lua` | MinGW has no library defining them |

## Changing the patch

```sh
scripts/build-mame.sh linux               # applies the patch if needed
# edit files under vendor/mame
git -C vendor/mame add -N <new files>
git -C vendor/mame diff > patches/mame-0289-rea-rad-mobile.patch
scripts/build-mame.sh linux && scripts/build-mame.sh windows
```

## Builds

| Script | Output |
|---|---|
| `scripts/build-mame.sh linux` | `dist/mame-linux/` (this machine's glibc) |
| `scripts/build-mame.sh windows` | `dist/mame-windows/` (MinGW-w64 in the `gaming` distrobox) |
| `scripts/build-linux-portable.sh` | `dist/linux-portable/` (Debian bookworm, glibc 2.36, with the launcher and SDL2), by running `scripts/build-linux-release.sh` in Docker |
| `.github/workflows/release.yml` | Both packages on GitHub Actions (`scripts/build-linux-release.sh` in bookworm; `build-mame.sh windows` + `build-launcher.sh windows` on Arch) |
