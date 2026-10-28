#!/usr/bin/env bash
# Measures memory and CPU; moves the pointer while checking the limits in scripts/perf.rs.
#
#   scripts/perf.sh                 build and check (about two minutes)
#   scripts/perf.sh --no-build      reuse the release build
#   scripts/perf.sh --with-media    also sample normal pages with music playing
#   scripts/perf.sh --extended      also sample hover, swipes and recovery
#   scripts/perf.sh --lyrics-only   compare live synced lyrics on/off
#
# Hands off the mouse while it runs: it moves the pointer to open and close the notch.
# The test starts its own notch, which asks a running notch to close; start yours again with scripts/reload.sh.
set -euo pipefail
cd "$(dirname "$0")/.."

build=1
extra_arguments=()
for argument in "$@"; do
    case "$argument" in
        --no-build) build=0 ;;
        --with-media) extra_arguments+=(--with-media) ;;
        --extended) extra_arguments+=(--extended) ;;
        --lyrics-only) extra_arguments+=(--lyrics-only) ;;
        -h | --help) sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
        *) echo "unknown option: $argument" >&2; exit 2 ;;
    esac
done

[ "$build" = 1 ] && cargo build --release

mkdir -p run
cp target/x86_64-pc-windows-msvc/release/notch.exe run/notch-perf.exe

cargo run --release --quiet --example perf -- \
    --exe "$(cygpath -w "$PWD/run/notch-perf.exe")" \
    --log "$(cygpath -w "$PWD/run/perf.log")" \
    "${extra_arguments[@]}"
