#!/bin/sh
set -eu

export PATH="$PATH:$HOME/.cargo/bin:/opt/homebrew/bin:/usr/local/bin"
cd "$(dirname "$0")/.."

echo "Building Progred's browser editor…"
make build-web
exec python3 website/preview.py
