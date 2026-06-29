# poc-11 Proof Plan

The project direction is proof-first: choose code shapes that let behavior move
from `_unproven` files into Verus-verified executable kernels. `_unproven` is a
temporary or trusted boundary label, not a normal home for domain logic.

There is no `_proven` suffix. In `src/core/` and `src/facts/`, an unsuffixed file
means either its invariant-bearing behavior is covered by executable Verus proof
or it is only a thin wrapper around such code.

## Current Labels

- `verus-core/` is proven executable Rust. It currently verifies the generic
  projection gate used by `src/core/engine_unproven.rs`.
- `src/core/*_unproven.rs` contains the current operational core shell. These
  files expose the old public module names through `src/core/mod.rs`, but their
  filenames make the proof gap visible.
- `src/core/effects_unproven.rs` and `src/core/turn_unproven.rs` are the current
  staging surface for deterministic turn proof. `turn_unproven` orders queued
  work, emits storage effect requests, applies effect results, and delegates
  internal projection steps to the engine.
- `src/facts/link/project_unproven.rs` keeps link codec, extraction, and
  projection together because versioned byte interpretation is part of fact
  meaning. It also owns deterministic typed construction from explicit command
  parameters.
- `src/facts/link/api_unproven.rs` contains storage-backed report helpers.
- `src/facts/link/cli_unproven.rs` contains unproven app admission and formatting.
- `src/helpers/*_unproven.rs` contains narrow trusted boundaries for crypto/hex,
  clocks, SQLite, and TCP framing. Core storage is now only a trait contract;
  concrete SQLite lives in `src/helpers/sqlite_unproven.rs`.
- `src/cli_unproven.rs` is app wiring and string/argument handling.

## Target Shape

Move logic toward these proof-backed unsuffixed modules:

- `src/core/types.rs`: proof-friendly ids, edge addresses, validity, context, and
  validated-offer provenance types.
- `src/core/turn.rs`: deterministic `State + Input -> State + Effects`
  transition for admission, query results, projection, and wakeups, replacing
  `turn_unproven.rs` once the transition invariant is proven.
- `src/facts/link/project.rs`: verified link codec, canonical encode/decode,
  deterministic typed construction from explicit parameters, extraction,
  projection validity, emitted facts, and persistence decision.
- `src/helpers/*_unproven.rs`: narrow trusted adapters for crypto assumptions,
  SQLite, TCP sockets, filesystem, clocks, and similar external APIs.

Compatibility modules may re-export unproven or proven modules while the tree is
in transition. The file that contains invariant-bearing behavior keeps
`_unproven` until that behavior has a Verus proof.

## Proof Boundaries

Core proofs are about all possible fact families routed through the engine:

- A fact becomes valid only through its routed projector.
- Projectors receive only in-memory validated context.
- Every validated offer has a valid owner fact.
- Every validated offer was first asserted by that same owner during extraction.
- Persisted facts and persisted needs/offers are discovery hints, not authority.
- If fact A validates using fact B's offer, then B is valid; that dependency
  relationship is transitively valid over any projected chain.
- Admit, query, project, and wake turns preserve the ongoing engine invariant.
- Route dispatch is sound: decoded family tags select the right family projector,
  and malformed or unknown facts do not become valid.

Link proofs live in `src/facts/link/project.rs` because only the link family
defines what roots, parents, and ancestry mean:

- Link bytes decode canonically into the link semantic shape.
- `link_id(link) == fact_id(encode(link))`.
- Extraction emits exactly the self-offer and, for a child, exactly the parent
  need declared by the link fields.
- A `prev=None` link is an anchor root for its own component. Multiple anchors
  are allowed; the starter model does not prove global root uniqueness.
- A child link is valid only when the validated parent context proves the parent
  is in the same root/domain.
- Link projection emits no new facts unless a later model intentionally adds
  emitted facts.
- Link ancestry is domain preserving: any valid descendant has a valid parent
  chain ending at its claimed anchor root.

## Invariant Checklist Style

Source-file invariant checklists should state user-significant or
threat-model-significant properties first: content addressing, asserted data not
being authority, validated-context provenance, exact fact-family interpretation,
and no validity created by IO/storage/reporting. Avoid checklists that are only
call traces such as "function X calls function Y"; those details belong in Verus
specs, Rust tests, or contract tests under the named invariant.

## Invariant Responsibility

Each invariant has one proof owner. Other files may depend on that theorem or
prove a narrow local preservation rule, but they should not restate the theorem
as if it were their own.

| Owner | Responsibility |
| --- | --- |
| `core::item` | Fact-id meaning and crypto assumptions for content-addressed canonical bytes. |
| `core::projector` | Generic fact-family interface contract: canonical codec, content-pure extraction/durability, confined projection. |
| `facts::link::project` | Link-family implementation of the projector contract and link-specific validity/root/domain theorems. |
| `core::offer` | Edge representation and the asserted-to-validated promotion shape. |
| `core::typestate` | `Context` representation and exact validated-offer lookup shape. |
| `core::admit` | Asserted-only ingress for new/local facts; admission never creates validity. |
| `core::index` | Durable storage lookup contract for persisted facts and asserted edges. |
| `core::engine` | Validated-context provenance, promotion authority, emitted-fact re-entry, and ongoing queue-step safety. |
| `core::effects` | Helper boundary data shape; helper effects carry no validated state. |
| `core::turn` | Deterministic turn scheduling and effect-result application into the engine. |
| `core::play` | Replay/wake API semantics over the turn/engine invariants. |
| `core::runtime` | IO adapter isolation; network, clock, and send outcomes do not create validity. |
| `facts::link::api` | Reporting boundary; reports are observations, not proof evidence. |
| `facts::link::cli` | CLI adapter boundary; user input chooses constructor parameters only. |

The composition theorem is:

```text
core validated-context provenance
+ link's parent/root projection contract
=> every valid child link is backed by a valid parent link, transitively to an
   anchor in the same root/domain
```

## Stylized Link Model

The current runnable toy uses only:

```text
Link { prev: Option<FactId>, content: Vec<u8> }
```

That is enough to start proving parent transitivity, with `prev=None` as an
anchor. Before claiming stronger protocol-shaped invariants, migrate the link
semantic shape to carry a child root/domain id:

```text
Root:
  prev = None
  encoded root_id = None
  semantic_root_id = self fact id

Child:
  prev = Some(parent_id)
  encoded root_id = Some(anchor_id)
  semantic_root_id = anchor_id
```

The validated link context should expose a statement like:

```text
valid_link(link_id, root_id)
```

Then link projection checks:

- root: valid without parent context and emits `valid_link(self_id, self_id)`;
- child: valid only if context contains `valid_link(parent_id, claimed_root_id)`;
- child: emits `valid_link(self_id, claimed_root_id)` after validation;
- malformed links, roots that encode a foreign root id, and children whose parent
  has a different root/domain are invalid.

This is intentionally isomorphic to later fact families:

```text
fact declares domain id
fact declares dependency/authority id
projector requires validated context for that dependency
projector checks dependency.domain == fact.domain
projector emits validated statements only inside that same domain
```

For later fact families, `root_id` corresponds to `workspace_id` or another
authority domain. The link toy should prove the domain-preserving authority
pattern before we translate the heavier poc-10 user, device-link, and admin-grant
fact families.

## Full Proof Plan

1. **Proof-friendly core types.** Move ids, edge addresses, validity, validated
   offers, validated fields, and route tags toward shared executable types that
   Verus can reason about directly. Keep maps/scans simple first; optimize after
   the spec is stable.
2. **Link semantic shape.** Add the child-carried root/domain id to the runnable
   link fact shape. Preserve `prev=None` anchors and explicitly allow multiple
   anchors.
3. **Link codec proof.** Prove canonical encode/decode for the full link shape:
   accepted bytes decode uniquely, malformed tag/flags/lengths are rejected, and
   `decode(encode(link)) == link`.
4. **Link extraction proof.** Prove extraction is context-free and exact:
   self-offer only for the link id, parent need only for `prev`, and any root or
   domain statement needed by projection is derivable from encoded link fields or
   the fact id.
5. **Link projection proof.** Prove the family contract: anchors emit
   `valid_link(self,self)`, children require validated parent context with the
   same root id, no cross-root splice validates, and emitted offers/fields carry
   only the validated link statement for this fact.
6. **Core turn proof.** Prove `State + Input -> State + Effects` by induction over
   every turn: admission, need-query result, projection, offer-query result, and
   idle all preserve validated-offer provenance and context safety.
7. **Storage/effect contract.** Keep SQLite, sockets, filesystem, and clocks in
   helpers. Prove that successful effect results are interpreted only through the
   verified decode/admission/extraction path, and that errors cannot create
   validated state.
8. **Composition proof.** Instantiate the core transitive-validity theorem with
   the link projection contract. Prove every valid link has a domain-preserving
   ancestry chain to its claimed anchor, while making no uniqueness claim about
   anchors.
9. **Rename only when complete.** A file loses `_unproven` only after its
   invariant-bearing behavior is covered by Verus-verified executable code and
   realistic Rust tests. Until then, keep the `_unproven` label.

## Done Criteria

A file can lose `_unproven` only when its invariant-bearing behavior is covered by
Verus-verified executable code or is only a thin wrapper around such code. Each
move out of `_unproven` should include realistic Rust tests plus
`./scripts/run_verus.sh` coverage.
