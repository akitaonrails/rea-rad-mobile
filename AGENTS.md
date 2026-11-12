# Rad Mobile launcher — instructions for coding agents

This repository runs the player's own **Rad Mobile** (Sega, 1990, System 32)
ROM set on Windows and Linux with a bundled, patched MAME, a Wanszai-style
settings menu, true widescreen, and full steering-wheel support with force
feedback. It never ships ROMs.

Read `sdk/docs/STANDARDS.md` before any work; the playbooks are in
`sdk/docs/playbooks/` (for this launcher: `wheels-and-ffb.md` and
`arcade-mame.md`). The shared code comes from
[rea-launcher-sdk](https://github.com/akitaonrails/rea-launcher-sdk),
pinned to a tag in the root `Cargo.toml` (`rea-launcher`,
`rea-launcher-native`, Windows-only `rea-launcher-win32`, and
`rea-launcher-build`/`rea-launcher-ui` for the build and the tests).
`sdk/` is vendored by the SDK's `scripts/sdk-sync`: never edit it by
hand. A change the SDK should have goes there first (its
`docs/playbooks/sdk-change.md`), then the launcher bumps the tag.

## Always

- ROMs, EEPROM dumps, and anything copied out of them are never committed,
  embedded or published. Tests use synthetic bytes only.
  The one exception, at the owner's request: the README screenshots in
  `docs/screenshots/` (downscaled, taken from the owner's own ROM).
- Everything outside this repository is read-only (the ROM folders, other
  repos, home configs). Scratch work goes in `.work/` (ignored).
- Headless by default: tests and MAME probe runs use `-video none` or
  `gamescope --backend headless`; open a window only on purpose and say so.
- Never edit `vendor/mame` by hand without regenerating
  `patches/mame-0289-rea-rad-mobile.patch` (see `docs/MAME.md`).

## Layout

| Path | What |
|---|---|
| `crates/radmobile-core` | Pure logic: settings, ROM identification, EEPROM, the game's side of the wheel code (cabinet buttons, wizard steps, telemetry → force feedback), bridge layout, MAME command line. Wheel maths, pad maps, the wizard and synthetic FFB are `rea_launcher::{wheel, ffb}` |
| `crates/radmobile-launcher` | The `RadMobile` executable: the input thread (`rea_launcher_native::sdl`), the bridge loop, the egui menu |
| `sdk/` | Vendored SDK scripts and docs (`sdk/VERSION`); `.rea-check-waivers` holds the STANDARDS rules that don't fit, with why |
| `patches/` | Our MAME 0.289 patch (bridge input/output providers, optional EEPROM, true widescreen) |
| `scripts/build-mame.sh`, `scripts/package.sh` | Builds and release packages |
| `.github/workflows/` | `CI` (checks) and `Release` (packages; a `vX.Y.Z` tag publishes a release) |
| `docs/` | Research (Wanszai study, EEPROM map, RAM map), runtime notes |

## Checks (all must pass before a commit)

```sh
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Windows code is run, not only linted: Windows-target clippy and the tests
under Wine (headless), in the `gaming` distrobox (MinGW, lld, Wine):

```sh
cargo clippy --workspace --all-targets --target x86_64-pc-windows-gnu -- -D warnings
scripts/build-launcher.sh sdl2       # the pinned SDL2 MinGW package, once
CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=lld -L $PWD/dist/sdl2/SDL2-2.32.10/x86_64-w64-mingw32/lib" \
WINEPATH="Z:$(printf %s "$PWD/dist/sdl2" | tr / '\\')" \
  sdk/scripts/wine-test --target x86_64-pc-windows-gnu --locked --workspace
sh sdk/scripts/launcher-check .
```

`distrobox enter --no-workdir gaming -- sh -c 'cd /absolute/path && …'`.
Golden snapshots (`crates/radmobile-core/tests/{snapshots,behaviour}.rs`,
the launcher's `--help`) guard what the launcher writes and how the
controls behave; `REA_UPDATE_SNAPSHOTS=1` rewrites them, and a changed
snapshot is reviewed like code and explained in the commit.
