#!/bin/sh
# Assemble a release package from the built pieces.
#
#   scripts/package.sh linux     dist/pkg/RadMobile-<version>-linux-x86_64.tar.gz
#   scripts/package.sh windows   dist/pkg/RadMobile-<version>-windows-x86_64.zip
#
# Build first: scripts/build-mame.sh <os>, scripts/build-launcher.sh <os>,
# and for a Linux release scripts/build-linux-portable.sh (older glibc; the
# host build is used, with a warning, when it is missing).
# No ROM, EEPROM or other game data is ever packaged.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
target=${1:-linux}
version=$(sed -n 's/^version = "\(.*\)"/\1/p' "$root/Cargo.toml" | head -1)
die() { echo "package: $*" >&2; exit 1; }

case $target in
linux)
	name="RadMobile-$version-linux-x86_64"
	portable="$root/dist/linux-portable"
	if [ -x "$portable/radmobile" ] && [ -x "$portable/radmobile-launcher" ]; then
		mame_bin="$portable/radmobile"
		launcher="$portable/radmobile-launcher"
		libs="$portable/lib"
	else
		echo "package: WARNING dist/linux-portable is missing; using host builds (they need this machine's glibc)" >&2
		mame_bin="$root/dist/mame-linux/radmobile"
		launcher="$root/target/release/radmobile-launcher"
		libs=""
	fi
	mame_dir="$root/dist/mame-linux"
	;;
windows)
	name="RadMobile-$version-windows-x86_64"
	mame_bin="$root/dist/mame-windows/radmobile.exe"
	launcher="$root/target/x86_64-pc-windows-gnu/release/radmobile-launcher.exe"
	mame_dir="$root/dist/mame-windows"
	;;
*) die "unknown target '$target' (linux or windows)" ;;
esac

for f in "$mame_bin" "$launcher" "$mame_dir/bgfx" "$mame_dir/bgfx/chains/rea-crt-arcade.json" "$mame_dir/artwork/bgfx"; do
	[ -e "$f" ] || die "missing $f - build it first"
done

out="$root/dist/pkg/$name"
rm -rf "$out"
mkdir -p "$out/mame" "$out/roms" "$out/licenses"
cp "$mame_bin" "$out/mame/"
cp -r "$mame_dir/bgfx" "$out/mame/bgfx"
mkdir -p "$out/mame/artwork"
cp -r "$mame_dir/artwork/bgfx" "$out/mame/artwork/bgfx"
cp "$root/packaging/README-player.txt" "$out/README.txt"
cp "$root/packaging/roms-README.txt" "$out/roms/README.txt"
cp "$root/LICENSE-MIT" "$root/LICENSE-APACHE" "$root/THIRD_PARTY.md" "$out/licenses/"
cp "$root/vendor/mame/COPYING" "$out/licenses/MAME-COPYING.txt"
cp "$root/patches/mame-0289-rea-rad-mobile.patch" "$out/licenses/"

if [ "$target" = linux ]; then
	cp "$launcher" "$out/radmobile-launcher"
	if [ -n "$libs" ] && [ -d "$libs" ]; then
		mkdir -p "$out/lib"
		cp "$libs"/*.so* "$out/lib/"
	fi
	cat > "$out/RadMobile" <<'EOF'
#!/bin/sh
# Start the Rad Mobile launcher with the libraries shipped beside it.
here=$(dirname "$(readlink -f "$0")")
LD_LIBRARY_PATH="$here/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}" exec "$here/radmobile-launcher" "$@"
EOF
	chmod +x "$out/RadMobile" "$out/radmobile-launcher" "$out/mame/radmobile"
	tar -C "$root/dist/pkg" -czf "$root/dist/pkg/$name.tar.gz" "$name"
	echo "package: $root/dist/pkg/$name.tar.gz"
else
	cp "$launcher" "$out/RadMobile.exe"
	cp "$root/dist/sdl2/SDL2.dll" "$out/"
	cp "$root/dist/sdl2/SDL2-2.32.10/LICENSE.txt" "$out/licenses/SDL2-LICENSE.txt"
	(cd "$root/dist/pkg" && rm -f "$name.zip" && python3 -I -c '
import os, sys, zipfile
name = sys.argv[1]
with zipfile.ZipFile(name + ".zip", "w", zipfile.ZIP_DEFLATED) as z:
    for base, dirs, files in os.walk(name):
        dirs.sort()
        for f in sorted(files):
            z.write(os.path.join(base, f))
' "$name")
	echo "package: $root/dist/pkg/$name.zip"
fi
