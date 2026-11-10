#!/bin/sh
# Build MAME and the launcher for Linux inside Debian bookworm (glibc
# 2.36), so the release runs on older distributions and SteamOS, not only
# on the machine that built it.
#
#   scripts/build-linux-portable.sh
#
# Output: dist/linux-portable/{radmobile,radmobile-launcher,lib/*.so*}.
# Needs Docker. The host's rustup toolchain is mounted read-only (rustup
# toolchains run on old glibc); crates are cached in .work/cargo-portable.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
image=debian:bookworm-slim
rustup_home=${RUSTUP_HOME:-$HOME/.rustup}
toolchain=$(rustc --version | awk '{print $2}')
mkdir -p "$root/.work/cargo-portable" "$root/dist/linux-portable"

# the patch must already be applied (scripts/build-mame.sh does it)
git -C "$root/vendor/mame" apply --reverse --check "$root/patches/mame-0289-rea-rad-mobile.patch" ||
	{ echo "build-linux-portable: run scripts/build-mame.sh linux first (applies the patch)" >&2; exit 1; }

# the build image: bookworm with MAME's and SDL's development packages (cached)
docker build -q -t rea-rad-mobile-build - >/dev/null <<EOF
FROM $image
RUN apt-get update && apt-get install -y --no-install-recommends \\
	build-essential python3 libsdl2-dev libsdl2-ttf-dev libfontconfig-dev \\
	libxinerama-dev libxi-dev libgl-dev pkg-config ca-certificates git \\
	&& rm -rf /var/lib/apt/lists/*
EOF

docker run --rm \
	-u "$(id -u):$(id -g)" \
	-e HOME=/tmp \
	-e CARGO_HOME=/cargo \
	-e RUSTUP_HOME=/rustup \
	-e RUSTUP_TOOLCHAIN="$toolchain" \
	-v "$root:/src" \
	-v "$root/.work/cargo-portable:/cargo" \
	-v "$rustup_home:/rustup:ro" \
	-w /src \
	rea-rad-mobile-build sh -c '
set -eu
export PATH="/rustup/toolchains/'"$toolchain"'-x86_64-unknown-linux-gnu/bin:$PATH"
# genie, the MAME project generator, must run on the bookworm glibc; one built
# here still runs on the host
rm -f vendor/mame/3rdparty/genie/bin/linux/genie
make -C vendor/mame BUILDDIR=build-portable SUBTARGET=radmobile SOURCES=src/mame/sega/segas32.cpp \
	NO_USE_MIDI=1 NO_USE_PORTAUDIO=1 NO_USE_PULSEAUDIO=1 NO_USE_PIPEWIRE=1 USE_QTDEBUG=0 OPTIMIZE=2 \
	LDOPTS="-static-libstdc++ -static-libgcc" REGENIE=1 -j'"${JOBS:-$(nproc)}"'
out=dist/linux-portable
cp vendor/mame/radmobile "$out/radmobile"
strip "$out/radmobile"
CARGO_TARGET_DIR=target/portable cargo build --release --locked -p radmobile-launcher
cp target/portable/release/radmobile-launcher "$out/"
# the libraries a player may not have; glibc, GL and X11 stay the system ones
mkdir -p "$out/lib"
for lib in libSDL2-2.0.so.0 libSDL2_ttf-2.0.so.0; do
	cp -L "$(ldconfig -p | awk -v l="$lib" "\$1 == l { print \$NF; exit }")" "$out/lib/"
done
'
echo "build-linux-portable: $root/dist/linux-portable"
