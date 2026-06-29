# poc-11 Proof Plan

The project direction is proof-first: choose code shapes that let behavior move
from `_unproven` files into Verus-proven executable kernels. `_unproven` is a
temporary or trusted boundary label, not a normal home for domain logic.

## Current Labels

- `verus-core/` is proven executable Rust. It currently verifies the generic
  projection gate used by `src/core/engine_unproven.rs`.
- `src/core/*_unproven.rs` contains the current operational core shell. These
  files expose the old public module names through `src/core/mod.rs`, but their
  filenames make the proof gap visible.
- `src/facts/link/project_unproven.rs` keeps link codec, extraction, and
  projection together because versioned byte interpretation is part of fact
  meaning.
- `src/facts/link/{author,api,cli}_unproven.rs` contains storage-backed authoring,
  report, and formatting helpers.
- `src/cli_unproven.rs` is app wiring and string/argument handling.

## Target Shape

Move logic toward these proof-backed modules:

- `src/core/types_proven.rs`: proof-friendly ids, edge addresses, validity,
  context, and validated-offer provenance types.
- `src/core/turn_proven.rs`: deterministic `State + Input -> State + Effects`
  transition for admission, query results, projection, and wakeups.
- `src/facts/link/project_proven.rs`: verified link codec, canonical encode/decode,
  extraction, projection validity, emitted facts, and persistence decision.
- `src/facts/link/author_proven.rs`: verified command kernels that construct typed
  link facts from intent arguments.
- `src/helpers/*_unproven.rs`: narrow trusted adapters for crypto assumptions,
  SQLite, TCP sockets, filesystem, clocks, and similar external APIs.

An unsuffixed compatibility module may re-export proven or unproven modules while
the tree is in transition, but invariant-bearing logic should live in `_proven`
or `_unproven` files with an explicit status.

## Migration Order

1. **Prove link project.** Move codec/extract/project from
   `project_unproven.rs` into a Verus-backed `project_proven.rs`. Prove
   `decode(encode(f)) == f`, accepted bytes are canonical, extraction contains
   the offer for the fact id and the parent need when present, and projection
   validates roots and only validates children with a matching validated parent.
2. **Prove shared core types.** Move `FactId`, `EdgeAddr`, `Validity`, and
   validated-offer/context representations toward proof-friendly shared types so
   the adapter between runtime and Verus shrinks instead of growing.
3. **Prove the turn.** Replace the implicit drain loop with a deterministic turn:
   `State + Input -> State + Vec<EffectRequest>`. Prove every transition
   preserves validated-offer provenance and never creates validated state from an
   unready or invalid fact.
4. **Prove admission/extraction persistence contracts.** The verified admission
   transition should request persistence of exactly the verified extraction output
   for durable facts. SQLite remains in `helpers/sqlite_unproven.rs` behind a
   trusted storage contract until replaced.
5. **Prove link authoring.** Move command kernels from `author_unproven.rs` into
   `author_proven.rs`, proving authored facts encode the requested fields and
   name the dependencies that later projection will require.
6. **Shrink app and helper code.** `cli_unproven.rs`, sockets, filesystem, clocks,
   and SQLite should stay thin. If any helper accumulates domain logic, move that
   logic back into `core` or `facts` and prove it.

## Done Criteria

A file can lose `_unproven` only when its invariant-bearing behavior is covered by
Verus-verified executable code or is only a thin wrapper around such code. Each
move from `_unproven` to `_proven` should include realistic Rust tests plus
`./scripts/run_verus.sh` coverage.
