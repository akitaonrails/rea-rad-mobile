#!/bin/sh
# Build the RadMobile launcher.
#
#   scripts/build-launcher.sh linux     target/release/radmobile-launcher
#   scripts/build-launcher.sh windows   target/x86_64-pc-windows-gnu/release/radmobile-launcher.exe
#                                       (+ dist/sdl2/SDL2.dll)
#
# The Windows build runs in the `gaming` distrobox (MinGW-w64, LLD and its
# own Rust toolchain), against the pinned SDL2 MinGW package.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
target=${1:-linux}
die() { echo "build-launcher: $*" >&2; exit 1; }

pin() { sed -n "s/^$1=//p" "$root/packaging/sdl2.pin"; }

fetch_sdl2() {
	url=$(pin url)
	sum=$(pin sha256)
	dir="$root/dist/sdl2"
	archive="$dir/$(basename "$url")"
	mkdir -p "$dir"
	if [ ! -f "$archive" ] || ! echo "$sum  $archive" | sha256sum -c --status; then
		curl -fsSL -o "$archive.part" "$url"
		echo "$sum  $archive.part" | sha256sum -c --status || die "SDL2 download does not match its pin"
		mv "$archive.part" "$archive"
	fi
	[ -d "$dir/$(pin dir)" ] || tar xzf "$archive" -C "$dir"
	cp "$dir/$(pin dir)/bin/SDL2.dll" "$dir/SDL2.dll"
}

case $target in
linux)
	cargo build --release --locked -p radmobile-launcher --manifest-path "$root/Cargo.toml"
	;;
windows)
	if ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then
		command -v distrobox >/dev/null 2>&1 || die "x86_64-w64-mingw32-gcc not found"
		fetch_sdl2
		cd / && exec distrobox enter gaming -- sh -c "cd '$root' && scripts/build-launcher.sh windows"
	fi
	fetch_sdl2
	# the box keeps its own Rust toolchain in its home
	export CARGO_HOME="$HOME/.cargo" RUSTUP_HOME="$HOME/.rustup" PATH="$HOME/.cargo/bin:$PATH"
	sdl="$root/dist/sdl2/$(pin dir)/lib"
	export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_LINKER=x86_64-w64-mingw32-gcc
	export CARGO_TARGET_X86_64_PC_WINDOWS_GNU_RUSTFLAGS="-C link-arg=-fuse-ld=lld -L $sdl"
	cargo build --release --locked -p radmobile-launcher --target x86_64-pc-windows-gnu --manifest-path "$root/Cargo.toml"
	;;
*)
	die "unknown target '$target' (linux or windows)"
	;;
esac
