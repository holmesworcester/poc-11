#!/usr/bin/env bash
# Verus runner (adapted from codex/verus-connection-proof). proof.rs is compiled
# standalone by the Verus binary, never by cargo.
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
VERUS="${VERUS:-/home/holmes/verus-install/verus-x86-linux/verus}"

"$VERUS" --crate-type=lib "$ROOT/src/proof.rs"
echo "verus: ok"
