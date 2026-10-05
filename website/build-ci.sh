#!/bin/sh
set -eu

if [ "$(uname -s)" != Linux ] || [ "$(uname -m)" != x86_64 ] || [ "${CI-}" != true ]; then
    echo "This unsandboxed build is for disposable x86_64 Linux CI runners. Locally, use make build-website." >&2
    exit 1
fi

site_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
project_root=$(dirname "$site_dir")
cd "$project_root"
. "$project_root/tools/web-build-settings.sh"

export PATH="$HOME/.cargo/bin:$PATH"
if ! command -v rustup >/dev/null 2>&1; then
    installer=$(mktemp)
    trap 'rm -f "$installer"' EXIT HUP INT TERM
    curl --proto '=https' --tlsv1.2 -fsSL https://sh.rustup.rs -o "$installer"
    sh "$installer" -y --profile minimal --default-toolchain none --no-modify-path
fi
rustup toolchain install "$web_toolchain" --profile minimal --component rust-src,llvm-tools

# The editor shows which commit it was built from.
PROGRED_COMMIT=$("$project_root/tools/progred-commit")
export PROGRED_COMMIT

# This is the deliberate CI exception to the local Cargo tripwire.
export RUSTC_WRAPPER=
export CARGO_NET_OFFLINE=false
export CARGO_TARGET_DIR="$project_root/target/ci-web"
export CARGO_NET_GIT_FETCH_WITH_CLI=true
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUSTFLAGS="$web_rustflags"
cargo +"$web_toolchain" build --release --locked --bin progred -p progred \
    -Z "build-std=$web_build_std" --target "$web_target"
"$project_root/tools/wasm-bindgen" --target web --no-typescript \
    --out-dir web/pkg --out-name progred \
    "$CARGO_TARGET_DIR/$web_target/release/progred.wasm"

python3 -B -m unittest discover -s website -p 'test_*.py'
node --test website/test_*.cjs
python3 -B website/package.py
