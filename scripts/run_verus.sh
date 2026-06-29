#!/usr/bin/env bash
# Verus runner for executable proof-facing modules that runtime code calls.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CARGO_VERUS="${CARGO_VERUS:-/home/holmes/verus-install/verus-x86-linux/cargo-verus}"

if ! grep -R "verus!" "$ROOT/src" >/dev/null 2>&1; then
    echo "ERROR: no running-code Verus proof target exists yet."
    echo "Add Verus proofs to the actual core/fact modules before using this runner."
    exit 1
fi

if [ ! -x "$CARGO_VERUS" ] && ! command -v "$CARGO_VERUS" >/dev/null 2>&1; then
    echo "ERROR: cargo-verus not found at $CARGO_VERUS"
    exit 1
fi

(cd "$ROOT" && "$CARGO_VERUS" verify --lib)
echo "verus: ok"
