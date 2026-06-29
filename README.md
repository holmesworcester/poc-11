# poc-11 — link toy (Stage 0)

A minimal, functional **proof-of-model** for the in-memory projection /
bounded-replay design (`docs/research/in-memory-projection-bounded-replay.md`).
Full staged build plan: `~/.claude/plans/imperative-hugging-tome.md`.

## The model in one screen

- **Durable** (SQLite, behind the `Index` trait): a content-addressed fact log plus
  a *syntactic* needs/offers index of `Offer<Asserted>`. Edges come from the
  context-free `extract` (Pass 1 = admission). The index is **never windowed** and
  serves both match directions via one reverse key.
- **In memory**: validated read-model state from `project` (Pass 2), rebuilt on
  demand by `play()`/`replay()` as an explicit queue/worklist: admit seed facts
  into memory, query stored offerers for unmet needs, project admitted facts, and
  wake needers when validated offers appear.
- **Queue engine model**: `EngineState` makes the live split explicit with
  `to_admit` (load/decode/index facts in memory), `to_project` (validate admitted
  facts), need queries (pull stored offerers), and offer queries (wake stored/local
  needers). Validated offers carry their owner so the core invariant is concrete:
  every context offer came from a fact already projected valid.
- **Both directions are engine operations**: `replay(seeds)` does the backward
  **need→offer** pull (a bounded seed drags in its dependency closure); `wake(arrived)`
  does the forward **offer→need** cascade (a newly-available fact re-derives everyone
  who needs it, one hop per level — the §3 wavefront).
- **Typestate**: one `Offer<V>`. `extract` emits `Offer<Asserted>` (persisted);
  `Offer::validate` — the only bridge — promotes it to `Offer<Validated>` when
  `project` validates the item; `Context` carries only validated offers.
  `Admitted<I>`'s only constructor is `admit`, so extract-before-project is a compile
  error. **Projectors get no storage/IO handle.**
- **One fact**: `link { prev: Option<FactId> }` → a chain `link0 <- link1 <- ...`;
  a link is valid iff its parent is valid (a root is valid by itself). The chain's
  transitive validity is the Stage-1 Verus target.

## Layout — poc-10's core/protocol division

| path | role |
|------|------|
| `src/core/` | **protocol-agnostic runtime + playback**: `item` (content addressing), `offer` (`Offer<V>`), `typestate` (`Asserted`/`Validated`/`Context`), `projector` (the trait), `admit` (Pass 1), `index` (`Index` trait + `SqliteIndex`), `engine` (typed in-memory queues), `play` (`play`/`replay`/`wake`, Pass 2), `net`, `runtime` (the generic daemon `serve<P>`) |
| `src/protocol/` | **item families + projectors**: `link` (the one fact family) |
| `src/cli.rs` | the thin **app layer** wiring a protocol family into the core runtime |
| `src/proof.rs` | Verus source, verified standalone (see `scripts/run_verus.sh`) |

`core` depends on nothing protocol-specific; `protocol` depends on `core`; `cli` (the
composition root) depends on both. This is the seam Stage 3 generalizes into a manifest.

## Build & run

```sh
cargo build --bin lk
lk --db x.db --at 1 link                 # author a root
lk --db x.db --at 2 link --prev <id1>    # extend the chain
lk --db x.db replay --window 1           # seed 1, pull the whole chain via the index
lk --db x.db chain <head-id>             # validated chain (complete/length/root)
lk --db x.db count
lk --db x.db start --listen 127.0.0.1 9000 --peer 127.0.0.1 9001   # daemon
```

The daemon's ingress admits received frames (Pass 1, persist); validated state is
recomputed on read (`chain`/`replay`), so there is no resident state to keep in sync.

## Test (serial — see `.cargo/config.toml`)

```sh
cargo test
```

- `bounded_replay` — **Test A**: window=10 over a 25-link chain projects all 25
  (`pulled_in_count: 15`); **Test B**: 25 independents → only the window projects
  (`pulled_in_count: 0`). Same totals, so B is a real control for A.
- `network_roundtrip` — **Test C**: a chain authored on daemon A is reconstructed
  and validated on daemon B over a real TCP socket.
- `reverse_key` — **Test D**: a child admitted before its parent is Invalid; once the
  parent arrives, the engine's `wake` (offer→need) re-derives and validates it. A
  read-only fake index also proves replay loads stored facts into memory without
  writing their bytes or asserted edges back to persistence.
- `engine_queues` — the proof-facing queue split against the SQLite-backed storage
  contract: demanding only a head pulls the stored parent chain into memory, and a
  later in-memory parent admission wakes a stored child. These tests also assert
  validated-offer provenance and that requeued valid facts do not duplicate
  promoted offers.

## Verify (Verus)

```sh
./scripts/run_verus.sh    # real link-projector and transitive-chain proofs
```

`src/proof.rs` abstracts crypto and durable storage behind typed contracts, then
proves a generic positive projection calculus: admitted facts have typed needs and
offers, ready facts promote only owner-bearing validated offers, context soundness
(`validated_offers` only come from valid owners) and offer provenance are preserved,
and any dependency-respecting projection schedule validates all scheduled facts.
The link projector is then an instance of that calculus, proving root/child
behavior and transitive validity for any root-to-head chain projected through valid
edges.

## Review gates

`cargo test`, `cargo clippy --all-targets -- -D warnings`, `cargo fmt --check`, and
`./scripts/run_verus.sh` all pass. `Cargo.lock` is tracked (this crate builds a binary).
