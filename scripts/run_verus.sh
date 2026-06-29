#!/usr/bin/env bash
# Verus runner for executable proof-facing core modules that the runtime calls.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_VERUS="${CARGO_VERUS:-/home/holmes/verus-install/verus-x86-linux/cargo-verus}"

if [ ! -x "$CARGO_VERUS" ] && ! command -v "$CARGO_VERUS" >/dev/null 2>&1; then
    echo "ERROR: cargo-verus not found at $CARGO_VERUS"
    exit 1
fi

(cd "$ROOT" && "$CARGO_VERUS" verify --lib)
echo "verus: ok"
