#!/usr/bin/env bash
# Show the new window before asking the previous instance to close.
set -euo pipefail
cd "$(dirname "$0")/.."

build_arguments=(--release)
profile_directory="release"
if [ "${1:-}" = "--debug" ]; then
    build_arguments=()
    profile_directory="debug"
fi

cargo build "${build_arguments[@]}"
# GPUI builds only with the MSVC host toolchain; the tray's Settings… item starts this program from the notch's folder.
cargo +stable-x86_64-pc-windows-msvc build "${build_arguments[@]}" -p notch-settings

run_directory="run"
mkdir -p "$run_directory"
# A settings window that is still open locks its copy, so a failed copy keeps the old one.
cp "target/x86_64-pc-windows-msvc/$profile_directory/notch-settings.exe" "$run_directory/notch-settings.exe" 2>/dev/null || true

# A running .exe is locked on Windows, so each launch uses its own copy of the build output.
built_executable="target/x86_64-pc-windows-msvc/$profile_directory/notch.exe"
launched_executable="$run_directory/notch-$(date +%H%M%S-%N | cut -c1-10).exe"
cp "$built_executable" "$launched_executable"

# `start` detaches the notch from this shell; its output is redirected so a pipe reading this script can finish.
cmd.exe //c start "" "$(cygpath -w "$PWD/$launched_executable")" > /dev/null 2>&1 < /dev/null

sleep 2
# Old copies exit after the handoff; copies that are still running stay locked and are skipped.
for old_executable in "$run_directory"/notch-[0-9]*.exe; do
    [ "$old_executable" = "$launched_executable" ] || rm -f "$old_executable" 2>/dev/null || true
done
echo "running: $PWD/$launched_executable"
