#!/usr/bin/env bash
# Debug builds skew performance; use reload.sh and perf.sh for release measurements.
set -uo pipefail
cd "$(dirname "$0")/.."

watched=(src Cargo.toml Cargo.lock build.rs assets/branding/logo.ico assets/branding/logo.rc)
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
        sleep 0.5
        rebuild
    fi
done
