#!/bin/sh
set -eu
ROOT=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$ROOT"
if [ -x "$ROOT/.cargo/bin/cargo" ]; then
  export CARGO_HOME="$ROOT/.cargo" RUSTUP_HOME="$ROOT/.rustup"
  export PATH="$CARGO_HOME/bin:$PATH"
fi
export MACOSX_DEPLOYMENT_TARGET=12.0
# Remove local source and toolchain paths from panic strings and other build metadata.
export RUSTFLAGS="${RUSTFLAGS:+$RUSTFLAGS }--remap-path-prefix=$ROOT=/src/screensaver-timer"
exec cargo "$@"
