#!/usr/bin/env bash
# Verus runner for the executable proof-facing core that the runtime calls.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_VERUS="${CARGO_VERUS:-/home/holmes/verus-install/verus-x86-linux/cargo-verus}"

if [ ! -x "$CARGO_VERUS" ] && ! command -v "$CARGO_VERUS" >/dev/null 2>&1; then
    echo "ERROR: cargo-verus not found at $CARGO_VERUS"
    exit 1
fi

(cd "$ROOT/verus-core" && "$CARGO_VERUS" verify)
echo "verus: ok"
