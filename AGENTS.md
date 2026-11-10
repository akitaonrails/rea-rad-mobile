# Rad Mobile launcher — instructions for coding agents

This repository runs the player's own **Rad Mobile** (Sega, 1990, System 32)
ROM set on Windows and Linux with a bundled, patched MAME, a Wanszai-style
settings menu, true widescreen, and full steering-wheel support with force
feedback. It never ships ROMs.

## Always

- ROMs, EEPROM dumps, and anything copied out of them are never committed,
  embedded or published. Tests use synthetic bytes only.
- Everything outside this repository is read-only (the ROM folders, other
  repos, home configs). Scratch work goes in `.work/` (ignored).
- Headless by default: tests and MAME probe runs use `-video none` or
  `gamescope --backend headless`; open a window only on purpose and say so.
- Never edit `vendor/mame` by hand without regenerating
  `patches/mame-0289-rea-rad-mobile.patch` (see `docs/MAME.md`).

## Layout

| Path | What |
|---|---|
| `crates/radmobile-core` | Pure logic: settings, ROM identification, EEPROM, wheel maths, FFB synthesis, bridge layout, MAME command line |
| `crates/radmobile-launcher` | The `RadMobile` executable: SDL2 devices and haptics, the bridge loop, the egui menu |
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

Windows builds run in the `gaming` distrobox (MinGW, lld, Wine):
`distrobox enter gaming -- sh -c 'cd /absolute/path && …'`.
