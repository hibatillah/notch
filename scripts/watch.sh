#!/usr/bin/env bash
# Rebuilds and relaunches the notch (debug build) whenever a source file is saved. Stop it with Ctrl+C.
#
#   scripts/watch.sh
#
# A debug build after a small edit takes about 2 s and looking for saved files every 2 s costs next to nothing.
# A debug build is slower and larger than a release build, so use scripts/reload.sh and scripts/perf.sh for numbers.
set -uo pipefail
cd "$(dirname "$0")/.."

watched=(src Cargo.toml Cargo.lock)
stamp="$(mktemp)"
trap 'rm -f "$stamp"' EXIT

rebuild() {
    bash scripts/reload.sh --debug || echo "build failed; waiting for the next save"
}

rebuild
while true; do
    sleep 2
    if [ -n "$(find "${watched[@]}" -type f -newer "$stamp" -print -quit)" ]; then
        # Marked before building, so a save during the build starts another round.
        touch "$stamp"
        # Editors often write several files for one save.
        sleep 0.5
        rebuild
    fi
done
