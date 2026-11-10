#!/bin/sh
# Build the Linux release binaries on the machine it runs on. Run it on an
# old distribution (Debian bookworm, glibc 2.36) so the result runs almost
# anywhere: scripts/build-linux-portable.sh does that through Docker, and
# the release workflow runs it in a bookworm container.
#
# Needs MAME's build dependencies, SDL2 and SDL2_ttf development files, and
# cargo on PATH. Output: dist/linux-portable/{radmobile,radmobile-launcher,lib/}
# plus dist/mame-linux/ (bgfx shaders and artwork) from scripts/build-mame.sh.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
out="$root/dist/linux-portable"

"$root/scripts/build-mame.sh" linux
mkdir -p "$out/lib"
cp "$root/dist/mame-linux/radmobile" "$out/radmobile"

target_dir=${CARGO_TARGET_DIR:-$root/target/portable}
CARGO_TARGET_DIR="$target_dir" cargo build --release --locked -p radmobile-launcher \
	--manifest-path "$root/Cargo.toml"
cp "$target_dir/release/radmobile-launcher" "$out/"

# the libraries a player may not have; glibc, GL and X11 stay the system ones
for lib in libSDL2-2.0.so.0 libSDL2_ttf-2.0.so.0; do
	path=$(ldconfig -p | awk -v l="$lib" '$1 == l { print $NF; exit }')
	[ -n "$path" ] || { echo "build-linux-release: $lib not found" >&2; exit 1; }
	cp -L "$path" "$out/lib/"
done
echo "build-linux-release: $out"
