#!/usr/bin/env bash
# Builds the notch and swaps the running copy for the new build without killing it from outside.
#
#   scripts/reload.sh           release build
#   scripts/reload.sh --debug   debug build, about ten times quicker to build (used by scripts/watch.sh)
# The new copy shows its window first, then asks the previous copy to close itself.
set -euo pipefail
cd "$(dirname "$0")/.."

build_arguments=(--release)
profile_directory="release"
if [ "${1:-}" = "--debug" ]; then
    build_arguments=()
    profile_directory="debug"
fi

cargo build "${build_arguments[@]}"

run_directory="run"
mkdir -p "$run_directory"

# A running .exe is locked on Windows, so each launch uses its own copy of the build output.
built_executable="target/x86_64-pc-windows-msvc/$profile_directory/notch.exe"
launched_executable="$run_directory/notch-$(date +%H%M%S-%N | cut -c1-10).exe"
cp "$built_executable" "$launched_executable"

# `start` detaches the notch from this shell; its output is redirected so a pipe reading this script can finish.
cmd.exe //c start "" "$(cygpath -w "$PWD/$launched_executable")" > /dev/null 2>&1 < /dev/null

sleep 2
# Old copies exit after the handoff; copies that are still running stay locked and are skipped.
for old_executable in "$run_directory"/notch-*.exe; do
    [ "$old_executable" = "$launched_executable" ] || rm -f "$old_executable" 2>/dev/null || true
done
echo "running: $PWD/$launched_executable"
