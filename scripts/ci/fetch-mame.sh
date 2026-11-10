#!/bin/sh
# Fetch the pinned MAME submodule shallowly, with its release tag (the build
# scripts check that vendor/mame is exactly that tag).
set -eu
tag=mame0289
git config --global --add safe.directory '*'
git submodule update --init --depth 1 vendor/mame
git -C vendor/mame fetch --depth 1 origin "refs/tags/$tag:refs/tags/$tag"
[ "$(git -C vendor/mame describe --tags --exact-match)" = "$tag" ]
