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
- **One fact**: the current starter is
  `link { prev: Option<FactId>, root: Option<FactId> }` -> a chain
  `link0 <- link1 <- ...`; `prev=None, root=None` is an anchor root for that
  component, not a unique global root. Children carry the root/domain they claim,
  and projection validates them only against a parent statement in that same
  root/domain.

## Layout — poc-10's core/protocol division

| path | role |
|------|------|
| `src/core/` | proof-targeted generic runtime/playback API; `effects_unproven` and `turn_unproven` stage the deterministic turn proof surface |
| `src/facts/link/` | **link fact family**: `project_unproven` holds typed construction, codec, extract, project, and local Verus kernels; it stays `_unproven` until core/admit/replay composition proves the end-to-end validated-store invariants |
| `src/helpers/` | narrow trusted helper/effect boundaries: crypto, hex, clock, SQLite, and TCP framing |
| `src/cli_unproven.rs` | thin app layer wiring the link fact family into the core runtime |

`core` depends on nothing fact-family-specific; `facts` depends on `core`;
`cli_unproven` (the composition root) depends on both. This is the seam Stage 3
generalizes into a manifest.

## Build & run

```sh
cargo build --bin lk
lk --db x.db --at 1 link                 # construct/admit a root
lk --db x.db --at 2 link --prev <id1> --root <root-id>  # extend the chain
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
- `network_roundtrip` — **Test C**: a chain admitted on daemon A is reconstructed
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
./scripts/run_verus.sh
```

There is intentionally no standalone proof-only `gate.rs`. Verus proofs should
land on the running `core` and `facts/link` code. Until a real running-code Verus
target exists, `scripts/run_verus.sh` fails instead of reporting fake proof
coverage.

Proof goals:

- Verify executable core engine and fact-family code, not parallel proof-only
  models.
- Verify fact-family kernels: deterministic typed construction, codec canonicality,
  extraction, projection, and emitted facts.
- Shape deterministic execution as a proof target: move queue/drain logic toward a
  `turn` function that takes state plus inputs and returns new state plus requested
  effects.
- Verify IO/storage interaction contracts around sockets, filesystem, and SQLite:
  accepted network frames go through verified decode/admission, persisted asserted
  edges are exactly verified extraction output, successful lookups satisfy the
  stated storage contract, and errors cannot create validated state. The OS,
  TCP, filesystem, and SQLite implementations remain trusted components unless
  replaced by verified implementations.

Proof-first organization:

- The default direction is to move as much logic as possible into proven code. If
  current Rust shape makes an invariant hard to prove, prefer reshaping the code
  around proof-friendly deterministic kernels instead of leaving the invariant as
  an informal convention.
- Keep the poc-10-style split: `src/core/` owns generic deterministic machinery
  such as turns, queues, contexts, admission, projection gates, and effect
  requests; `src/facts/` owns fact-family logic. Keep fact families in
  poc-10-style directories such as `src/facts/link/`, with `project` owning typed
  construction, codec/extract/project semantics, `api` owning observation, `cli`
  owning app admission/formatting, and tests/proofs as they become real files.
- Use `src/helpers/` for narrow external primitives and effect adapters:
  `crypto_unproven.rs`, `sqlite_unproven.rs`, `tcp_unproven.rs`,
  `fs_unproven.rs`, `clock_unproven.rs`. These files are explicit trusted
  boundaries with limited roles, not places for domain logic.
- Files without `_unproven` in `core` and `facts` should have their invariants
  covered by Verus-verified executable code or by thin wrappers around such code.
  Moving logic out of `_unproven` is expected work, not optional cleanup.
- The concrete migration order is tracked in `docs/proof-plan.md`.

## Review gates

`cargo test`, `cargo clippy --all-targets -- -D warnings`, and
`cargo fmt --check` should pass. `scripts/run_verus.sh` is expected to fail until
real Verus proofs over running code are added. `Cargo.lock` is tracked because
this crate builds a binary.
