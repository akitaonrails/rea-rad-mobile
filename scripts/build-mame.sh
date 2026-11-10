#!/bin/sh
# Build the bundled MAME: the pinned upstream release (vendor/mame), our patch
# (patches/), and only the Sega System 32 driver.
#
#   scripts/build-mame.sh linux     native build, binary at dist/mame-linux/radmobile
#   scripts/build-mame.sh windows   MinGW-w64 cross build, dist/mame-windows/radmobile.exe
#
# The Windows build needs x86_64-w64-mingw32-gcc. When the host lacks it, the
# script re-runs itself inside the `gaming` distrobox (see README).
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
target=${1:-linux}
mame="$root/vendor/mame"
patch="$root/patches/mame-0289-rea-rad-mobile.patch"
tag=mame0289
jobs=${JOBS:-$(nproc)}

die() { echo "build-mame: $*" >&2; exit 1; }

[ -d "$mame/src" ] || die "vendor/mame is missing: git submodule update --init --depth 1 vendor/mame"
[ "$(git -C "$mame" describe --tags --exact-match 2>/dev/null || true)" = "$tag" ] ||
	die "vendor/mame is not at $tag"

# Apply the patch once; a checkout that already has it is left alone.
if git -C "$mame" apply --reverse --check "$patch" 2>/dev/null; then
	echo "build-mame: patch already applied"
else
	git -C "$mame" apply --check "$patch" || die "patch does not apply to vendor/mame"
	git -C "$mame" apply "$patch"
	echo "build-mame: patch applied"
fi

common="SUBTARGET=radmobile SOURCES=src/mame/sega/segas32.cpp NO_USE_MIDI=1 NO_USE_PORTAUDIO=1 USE_QTDEBUG=0 OPTIMIZE=2 REGENIE=1"

case $target in
linux)
	# shellcheck disable=SC2086 # $common is a word list on purpose
	make -C "$mame" $common NO_USE_PULSEAUDIO=1 NO_USE_PIPEWIRE=1 \
		LDOPTS="-static-libstdc++ -static-libgcc" -j"$jobs"
	out="$root/dist/mame-linux"
	mkdir -p "$out"
	cp "$mame/radmobile" "$out/radmobile"
	strip "$out/radmobile"
	;;
windows)
	if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
		command -v distrobox >/dev/null 2>&1 || die "x86_64-w64-mingw32-gcc not found"
		cd / && exec distrobox enter gaming -- sh -c "cd '$root' && JOBS=$jobs scripts/build-mame.sh windows"
	fi
	# shellcheck disable=SC2086
	make -C "$mame" $common TARGETOS=windows PTR64=1 CROSS_BUILD=1 MINGW64=/usr \
		OVERRIDE_CC=x86_64-w64-mingw32-gcc OVERRIDE_CXX=x86_64-w64-mingw32-g++ \
		OVERRIDE_LD=x86_64-w64-mingw32-g++ OVERRIDE_AR=x86_64-w64-mingw32-ar \
		-j"$jobs"
	out="$root/dist/mame-windows"
	mkdir -p "$out"
	cp "$mame/radmobile.exe" "$out/radmobile.exe"
	x86_64-w64-mingw32-strip "$out/radmobile.exe"
	;;
*)
	die "unknown target '$target' (linux or windows)"
	;;
esac

# MAME's prebuilt bgfx shaders and chains, for the CRT filters
rm -rf "$out/bgfx"
mkdir -p "$out/bgfx"
cp -r "$mame/bgfx/chains" "$mame/bgfx/effects" "$mame/bgfx/shaders" "$out/bgfx/"
# textures the CRT chains load through the artwork path
rm -rf "$out/artwork"
mkdir -p "$out/artwork"
cp -r "$mame/artwork/bgfx" "$out/artwork/"
echo "build-mame: $out"
