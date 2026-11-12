#!/bin/sh
# Compile our bgfx shaders (bgfx/src/*) into bgfx/shaders/<backend>/chains/*
# for every backend MAME 0.289 ships: dx9, dx11, essl, glsl, metal, spirv.
# The .bin files are committed, like MAME's own, so a normal build does not
# need this script; run it after editing a .sc file.
#
#   scripts/build-shaders.sh            all backends
#   SKIP_DX=1 scripts/build-shaders.sh  only the non-Windows backends
#
# shaderc comes from vendor/mame/3rdparty/bgfx and is built once under
# .work/shaderc (the submodule stays untouched):
#   - a native shaderc for essl, glsl, metal and spirv;
#   - a MinGW shaderc.exe for dx9 and dx11, run under Wine. shaderc compiles
#     HLSL through d3dcompiler_47.dll, and Wine's builtin one rejects bgfx's
#     shader prelude (no `reversebits`), so Microsoft's DLL is required: set
#     D3DCOMPILER_DLL to an x64 d3dcompiler_47.dll (Windows SDK
#     "Redist/D3D/x64", or winetricks d3dcompiler_47). It is copied next to
#     shaderc.exe under .work/ and never committed.
# MinGW and Wine come from the host or, when missing, the `gaming` distrobox.
set -eu

root=$(cd "$(dirname "$0")/.." && pwd -P)
bgfx3p="$root/vendor/mame/3rdparty"
work="$root/.work/shaderc"
src="$root/bgfx/src"
out="$root/bgfx/shaders"
jobs=${JOBS:-$(nproc)}

die() { echo "build-shaders: $*" >&2; exit 1; }

[ -d "$bgfx3p/bgfx/tools/shaderc" ] || die "vendor/mame is missing: git submodule update --init --depth 1 vendor/mame"

# Run a command on the host when it has $1, otherwise inside the gaming box.
in_box() {
	tool=$1; shift
	if command -v "$tool" >/dev/null 2>&1; then
		sh -c "$*"
	else
		command -v distrobox >/dev/null 2>&1 || die "$tool not found (and no distrobox)"
		(cd / && distrobox enter gaming -- sh -c "$*")
	fi
}

# Private copy of bgfx + bx + bimg so the build output stays out of vendor/.
copy_bgfx() {
	dir=$1
	if [ ! -d "$dir/bgfx" ]; then
		mkdir -p "$dir"
		for d in bgfx bx bimg; do
			cp -r "$bgfx3p/$d" "$dir/$d"
		done
		rm -rf "$dir/bgfx/.build" "$dir/bgfx/examples/runtime"
	fi
}

shaderc="${SHADERC:-$work/bin/shaderc}"
if [ ! -x "$shaderc" ]; then
	echo "build-shaders: building native shaderc"
	copy_bgfx "$work/linux"
	(cd "$work/linux/bgfx" &&
		../bx/tools/bin/linux/genie --with-tools --gcc=linux-gcc gmake >/dev/null &&
		make -C .build/projects/gmake-linux config=release64 -j"$jobs" shaderc >"$work/linux-build.log" 2>&1) ||
		die "native shaderc build failed, see $work/linux-build.log"
	mkdir -p "$work/bin"
	cp "$work/linux/bgfx/.build/linux64_gcc/bin/shadercRelease" "$work/bin/shaderc"
	shaderc="$work/bin/shaderc"
fi

shaderc_exe="${SHADERC_EXE:-$work/winbin/shaderc.exe}"
if [ "${SKIP_DX:-0}" != 1 ]; then
	if [ ! -f "$shaderc_exe" ]; then
		echo "build-shaders: building shaderc.exe (MinGW)"
		copy_bgfx "$work/win"
		# --os=windows: genie otherwise picks glslang's Unix sources
		in_box x86_64-w64-mingw32-gcc "cd '$work/win/bgfx' &&
			MINGW=/usr ../bx/tools/bin/linux/genie --with-tools --gcc=mingw-gcc --os=windows gmake >/dev/null &&
			MINGW=/usr make -C .build/projects/gmake-mingw-gcc config=release64 -j$jobs shaderc >'$work/win-build.log' 2>&1" ||
			die "shaderc.exe build failed, see $work/win-build.log"
		mkdir -p "$work/winbin"
		cp "$work/win/bgfx/.build/win64_mingw-gcc/bin/shadercRelease.exe" "$work/winbin/shaderc.exe"
		shaderc_exe="$work/winbin/shaderc.exe"
	fi
	dll="$(dirname "$shaderc_exe")/d3dcompiler_47.dll"
	if [ ! -f "$dll" ]; then
		[ -n "${D3DCOMPILER_DLL:-}" ] && [ -f "$D3DCOMPILER_DLL" ] ||
			die "dx9/dx11 need Microsoft's x64 d3dcompiler_47.dll: set D3DCOMPILER_DLL=/path/to/it (or SKIP_DX=1)"
		cp "$D3DCOMPILER_DLL" "$dll"
	fi
	mkdir -p "$work/wineprefix"
fi

includes="$bgfx3p/bgfx/src/;$bgfx3p/bgfx/examples/common/"

# backend  shaderc flags (vertex | fragment), as in MAME's shader.mk
flags() {
	case $1/$2 in
	dx9/vertex)   echo "--platform windows -p vs_3_0 -O 3" ;;
	dx9/fragment) echo "--platform windows -p ps_3_0 -O 3" ;;
	dx11/vertex)  echo "--platform windows -p vs_5_0 -O 3" ;;
	dx11/fragment) echo "--platform windows -p ps_5_0 -O 3" ;;
	essl/*)       echo "--platform android" ;;
	glsl/*)       echo "--platform linux -p 120" ;;
	metal/*)      echo "--platform osx -p metal" ;;
	spirv/*)      echo "--platform linux -p spirv" ;;
	esac
}

backends="essl glsl metal spirv"
[ "${SKIP_DX:-0}" = 1 ] || backends="dx9 dx11 $backends"

# One Wine session for all HLSL compiles is much faster than one per shader.
wine_jobs="$work/wine-jobs.sh"
: >"$wine_jobs"

for chain in "$src"/*/; do
	chain=$(basename "$chain")
	for backend in $backends; do
		dest="$out/$backend/chains/$chain"
		mkdir -p "$dest"
		for sc in "$src/$chain"/vs_*.sc "$src/$chain"/fs_*.sc; do
			[ -f "$sc" ] || continue
			name=$(basename "$sc" .sc)
			case $name in vs_*) type=vertex ;; *) type=fragment ;; esac
			rm -f "$dest/$name.bin"
			args="$(flags "$backend" "$type") --type $type -i '$includes' --varyingdef '$src/$chain/varying.def.sc' -f '$sc' -o '$dest/$name.bin'"
			case $backend in
			dx*) echo "echo '[$backend] $chain/$name' && wine '$shaderc_exe' $args || exit 1" >>"$wine_jobs" ;;
			*)
				echo "[$backend] $chain/$name"
				eval "'$shaderc' $args" || die "$backend $chain/$name failed"
				;;
			esac
		done
	done
done

if [ -s "$wine_jobs" ]; then
	in_box wine "WINEPREFIX='$work/wineprefix' WINEDEBUG=-all WINEDLLOVERRIDES=d3dcompiler_47=n sh -e '$wine_jobs' 2>&1 | grep -v -e '^MESA' -e '^pci id'" ||
		die "HLSL compile failed"
	# grep hides Wine's noise but also its exit status: check the outputs
	while read -r line; do
		bin=$(printf '%s\n' "$line" | sed -n "s/.* -o '\([^']*\)'.*/\1/p")
		[ -s "$bin" ] || die "missing $bin"
	done <"$wine_jobs"
fi
echo "build-shaders: done ($backends)"
