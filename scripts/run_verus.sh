#!/usr/bin/env bash
# Verus runner (adapted from codex/verus-connection-proof). proof.rs is compiled
# standalone by the Verus binary, never by cargo.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERUS="${VERUS:-/home/holmes/verus-install/verus-x86-linux/verus}"
CARGO_VERUS="${CARGO_VERUS:-/home/holmes/verus-install/verus-x86-linux/cargo-verus}"

"$VERUS" --crate-type=lib "$ROOT/src/proof.rs"

if [ ! -x "$CARGO_VERUS" ] && ! command -v "$CARGO_VERUS" >/dev/null 2>&1; then
    echo "ERROR: cargo-verus not found at $CARGO_VERUS"
    exit 1
fi

(cd "$ROOT/verus-core" && "$CARGO_VERUS" verify)
echo "verus: ok"
