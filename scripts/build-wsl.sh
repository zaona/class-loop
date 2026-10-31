#!/bin/sh
# Self-contained WSL entry; the SDK is a sibling of this repository.
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
WORKSPACE_ROOT=$(CDPATH= cd -- "$ROOT/.." && pwd)
. "$HOME/.cargo/env"
export CANOPUS_ROOT="${CANOPUS_ROOT:-$WORKSPACE_ROOT/Canopus}"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/canopus/host}"
export PATH="$CARGO_TARGET_DIR/debug:$PATH"
export PYTHONUTF8=1
# Build the CLI from the same checkout as the SDK: a cached CLI from main
# otherwise silently retains the old Rust linker pipeline after a branch switch.
if [ -z "${CANOPUS_CLI:-}" ]; then
    cargo build --manifest-path "$CANOPUS_ROOT/Cargo.toml" -p canopus-cli
    export CANOPUS_CLI="$CARGO_TARGET_DIR/debug/canopus"
fi
exec python3 "$ROOT/scripts/build.py" "$@"
