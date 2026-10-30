#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
# The settings crate builds only with the MSVC host toolchain, so the dependency graph is read with it.
RUSTUP_TOOLCHAIN=stable-x86_64-pc-windows-msvc uv run --no-project scripts/third_party_licenses.py
