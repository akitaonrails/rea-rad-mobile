#!/bin/sh
# Build MAME and the launcher for Linux inside Debian bookworm (glibc
# 2.36), so the release runs on older distributions and SteamOS, not only
# on the machine that built it. The steps are scripts/build-linux-release.sh;
# this wrapper only provides the container.
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

# the build image: bookworm with MAME's and SDL's development packages (cached)
docker build -q -t rea-rad-mobile-build - >/dev/null <<DOCKERFILE
FROM $image
RUN apt-get update && apt-get install -y --no-install-recommends \\
	build-essential python3 libsdl2-dev libsdl2-ttf-dev libfontconfig-dev \\
	libxinerama-dev libxi-dev libgl-dev pkg-config ca-certificates git \\
	&& rm -rf /var/lib/apt/lists/*
DOCKERFILE

# genie, the MAME project generator, must run on the bookworm glibc; one
# built there still runs on the host. A separate object directory keeps the
# host build intact.
docker run --rm \
	-u "$(id -u):$(id -g)" \
	-e HOME=/tmp \
	-e CARGO_HOME=/cargo \
	-e RUSTUP_HOME=/rustup \
	-e RUSTUP_TOOLCHAIN="$toolchain" \
	-e MAME_BUILDDIR=build-portable \
	-e JOBS="${JOBS:-$(nproc)}" \
	-v "$root:/src" \
	-v "$root/.work/cargo-portable:/cargo" \
	-v "$rustup_home:/rustup:ro" \
	-w /src \
	rea-rad-mobile-build sh -c '
set -eu
export PATH="/rustup/toolchains/'"$toolchain"'-x86_64-unknown-linux-gnu/bin:$PATH"
git config --global --add safe.directory "*"
rm -f vendor/mame/3rdparty/genie/bin/linux/genie
scripts/build-linux-release.sh
'
echo "build-linux-portable: $root/dist/linux-portable"
