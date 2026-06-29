# poc-11 Proof Plan

The project direction is proof-first: choose code shapes that let behavior move
from `_unproven` files into Verus-verified executable kernels. `_unproven` is a
temporary or trusted boundary label, not a normal home for domain logic.

There is no `_proven` suffix. In `src/core/` and `src/facts/`, an unsuffixed file
means either its invariant-bearing behavior is covered by executable Verus proof
or it is only a thin wrapper around such code.

## Current Labels

- `src/core/*_unproven.rs` contains the current operational core shell. These
  files expose the old public module names through `src/core/mod.rs`, but their
  filenames make the proof gap visible.
- `src/core/effects_unproven.rs` and `src/core/turn_unproven.rs` are the current
  staging surface for deterministic turn proof. `turn_unproven` orders queued
  work, emits storage effect requests, applies effect results, and delegates
  internal projection steps to the engine.
- `src/core/runtime_unproven.rs` is the current daemon/IO loop: sockets, sleeps,
  peer sends, and stdout readiness. It stays separate from `turn` so the
  deterministic queue/effect step can be proven without proving OS progress.
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

The `_unproven` naming rule is repository policy, not a semantic Verus theorem
about runtime behavior. Enforce it with source-tree tests and review gates; use
Verus for the executable invariants inside core and fact-family code.

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
- Extraction emits exactly the self-offer for `valid_link(self_id, root_id)` and,
  for a child, exactly the parent need for `valid_link(prev, root_id)`.
- Malformed `prev`/`root` combinations emit no edges and cannot validate.
- A `prev=None` link is an anchor root for its own component. Multiple anchors
  are allowed; the starter model does not prove global root uniqueness.
- In the current root/domain model, a root link (`prev=None, root=None`) is valid
  as `valid_link(self_id, self_id)`. A child link is valid only when validated
  context contains `valid_link(parent_id, claimed_root_id)`.
- A validated `valid_link(link_id, root_id)` statement is owned by a valid link
  fact whose id is `link_id` and whose semantic root is `root_id`.
- Link read-model state is updated by `LinkProjector::project` for each projected
  fact; reports observe that state after replay rather than walking persisted
  bytes on demand.
- Link projection emits no new facts unless a later model intentionally adds
  emitted facts.
- Current link ancestry is same-root parent-chain preserving: any valid
  descendant has a valid parent chain ending at an anchor in the same
  root/domain.

## Invariant Checklist Style

Source-file invariant checklists should state user-significant or
threat-model-significant properties first: content addressing, asserted data not
being authority, validated-context provenance, exact fact-family interpretation,
and no validity created by IO/storage/reporting. Avoid checklists that are only
call traces such as "function X calls function Y"; those details belong in Verus
specs, Rust tests, or contract tests under the named invariant.

Every checklist item must be labeled `Safety:` or `Liveness:`. Use `Safety:` for
properties that rule out bad states, invalid authority, bad interpretation,
unsound promotion, or invalid report evidence. Use `Liveness:` only for progress
claims such as eventually scheduling, waking, discovering, draining, or retrying
work. Do not put OS/socket/filesystem progress in a Verus invariant unless that
progress has been modeled as an explicit fair input to a deterministic core turn.

Each checklist should be followed by:

- `Imported theorems`: the external facts this proof depends on, named by owning
  module.
- `Proof strategy`: the local argument needed in this file, without reproving
  imported theorems.

## Invariant Responsibility

Each invariant has one proof owner. Source files use `Owned invariant:` to name
the property the current module owns. Other files may depend on that theorem or
prove a narrow local preservation rule, but they should not restate the theorem
as if it were their own.

| Owner | Responsibility |
| --- | --- |
| `core::item` | Fact-id meaning and crypto assumptions for content-addressed canonical bytes. |
| `core::projector` | Generic fact-family interface contract: canonical codec, content-pure extraction/durability, confined projection. |
| `facts::link::project` | Link-family implementation of the projector contract, projector-owned read-model state, and current same-root parent-chain validity theorem. |
| `core::offer` | Edge representation and the asserted-to-validated promotion shape. |
| `core::typestate` | `Context` representation and exact validated-offer lookup shape. |
| `core::admit` | New/local fact admission creates only asserted state; admission never creates validity. |
| `core::index` | Durable storage lookup contract for persisted facts and asserted edges. |
| `core::engine` | In-memory id/body relation, running readiness/promotion rule, validated-context provenance, promotion authority, emitted-fact re-entry, and ongoing queue-step safety. |
| `core::effects` | Helper boundary data shape; helper effects carry no validated state. |
| `core::turn` | Deterministic turn scheduling, effect-result application into the engine, and the future fair-input liveness model. |
| `core::play` | Replay/wake API semantics over the turn/engine invariants. |
| `core::runtime` | IO adapter isolation; network, clock, and send outcomes do not create validity. |
| `facts::link::api` | Reporting boundary; commands run replay and observe projector-owned state, but reports are not proof evidence. |
| `facts::link::cli` | CLI adapter boundary; user input chooses constructor parameters only. |

The current composition theorem is:

```text
core validated-context provenance
+ link's parent/root projection contract
=> every valid child link is backed by a valid parent link, transitively to an
   anchor in the same root/domain
```

## Stylized Link Model

The current runnable toy uses only:

```text
Link { prev: Option<FactId>, root: Option<FactId>, content: Vec<u8> }
```

That is enough to prove same-root parent transitivity, with `prev=None` and
`root=None` as an anchor. Stronger protocol-shaped invariants still require the
relationship fields being proved: parent-author, device, or admin-grant
relationships must be explicit link/fact fields before their preservation can be
a link theorem.

```text
Current root:
  prev = None
  root = None
  semantic_root_id = self fact id

Current child:
  prev = Some(parent_id)
  root = Some(anchor_id)
  semantic_root_id = anchor_id
```

The validated link context should expose a statement like:

```text
valid_link(link_id, root_id)
```

Link projection checks:

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
2. **Link semantic shape.** Keep the child-carried root/domain id in the runnable
   link fact shape. Preserve `prev=None, root=None` anchors and explicitly allow
   multiple anchors.
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
   only the validated link statement for this fact. Prove the link-specific
   statement-to-owner lemma: any validated `valid_link(x,r)` came from a valid
   link fact whose own id is `x` and whose semantic root is `r`. Prove projector
   state confinement: projection of fact `x` may update only link-owned state
   entries for `x`, and complete report entries are built incrementally from
   already-projected same-root parent entries.
6. **Core turn proof.** Prove `State + Input -> State + Effects` by induction over
   every turn: admission, need-query result, projection, offer-query result, and
   idle all preserve validated-offer provenance and context safety.
7. **Fair-input liveness model.** Before proving liveness, model helper/storage
   results and transport arrivals as explicit fair inputs to the deterministic
   turn. Prove progress only over that model; do not smuggle OS/socket/filesystem
   fairness into safety invariants.
8. **Storage/effect contract.** Keep SQLite, sockets, filesystem, and clocks in
   helpers. Prove that successful effect results are interpreted only through the
   verified decode/admission/extraction path, and that errors cannot create
   validated state.
9. **Composition proof.** Instantiate the core transitive-validity theorem with
   the link projection contract. Prove every valid link has a domain-preserving
   ancestry chain to its claimed anchor by induction over `prev`, while making
   no uniqueness claim about anchors.
10. **Rename only when complete.** A file loses `_unproven` only after its
   invariant-bearing behavior is covered by Verus-verified executable code and
   realistic Rust tests. Until then, keep the `_unproven` label.

## Done Criteria

A file can lose `_unproven` only when its invariant-bearing behavior is covered by
Verus-verified executable code or is only a thin wrapper around such code. Each
move out of `_unproven` should include realistic Rust tests plus running-code
Verus coverage. `scripts/run_verus.sh` must fail rather than claim success when
no running-code Verus proof target exists.
