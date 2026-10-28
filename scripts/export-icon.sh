#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
uv run --no-project --with pillow scripts/export_icon.py
