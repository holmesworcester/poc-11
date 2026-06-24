# poc-11 Feedback

## Findings

- **High:** offer-to-need wakeups are not actually driven by replay. `src/play.rs` only resolves needs to offerers, then promotes offers and stops. The reverse-key test manually calls `needs_for_key` and replays those needers, so it proves the lookup exists, not that the engine or daemon handles the cascade promised in `README.md`. If Stage 0 is meant to model both directions from the poc-10 note, this is the main gap.

- **Medium:** `SqliteIndex::flush_fact` accepts arbitrary `id`/`bytes` pairs without enforcing `id == fact_id(bytes)`. That weakens the content-addressed fact log invariant and lets replay re-admit a body under a different computed id. The cycle test depends on this raw injection, so consider moving that test to a fake `Index` instead of weakening the real SQLite path.

- **Medium:** `src/offer.rs` is redundant, uncompiled duplicate scaffolding. It is not in `src/lib.rs`, duplicates `edge.rs`/`typestate.rs`, and can be deleted for an immediate 118-line cut with no functional loss.

- **Low:** `cargo fmt --check` fails. `cargo test`, Clippy, and Verus pass, but formatting is not review-ready.

- **Low:** `.gitignore` ignores `Cargo.lock` even though this repo builds a CLI binary. Track the lockfile for reproducible tests and daemon behavior.

## Assessment

The design makes sense as a Stage 0 proof-of-model. The persisted syntactic index, typestate boundary, demand replay, CLI black-box tests, and TCP round trip are coherent. The tests are realistic and cover the headline need-to-offer bounded replay behavior.

It is not optimal yet. The no-risk reduction is deleting `src/offer.rs`. The bigger simplification depends on scope: for a pure link-chain demo, `EmittedFact`, `durable`, `Origin`, and some future-looking edge metadata could be trimmed, but those are carrying the intended Stage 1/Stage 2 shape. Keep them only if that forward shape is the point of `poc-11`.

## Verification

- `cargo test`: pass
- `cargo clippy --all-targets -- -D warnings`: pass
- `./scripts/run_verus.sh`: pass
- `cargo fmt --check`: fail
