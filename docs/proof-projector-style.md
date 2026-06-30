# Proof Projector Style Guide

This guide describes how proof-bearing projector files should read in poc-11.
It adapts the structured poc-10 projector style to Verus-backed code: the goal
is not minimal line count, but a file a reviewer can read top to bottom and
understand what the logic proves, what the proof assumes, and which remaining
claims are still owned elsewhere.

Use this guide for fact-family projectors such as
`src/facts/link/project.rs`, and for future `src/facts/*/project.rs`
files once their owned invariants and imported composition theorems are fully
proven.

## Core Rule

Write projector proof files as a story:

1. Bytes establish identity.
2. Shape determines which semantic path applies.
3. Extraction declares exactly what the fact may later claim and need.
4. Validated context supplies authority.
5. Projection promotes only the statement justified by that authority.
6. Projector state records only owned read-model consequences.
7. Composition imports core/replay provenance to lift local steps into honest
   cross-module theorems without deriving facts the imported core model does not
   expose.

The top-level policy, function ordering, proof sectioning, and tests should all
follow that same order.

## Recommended Shape

Each non-trivial proof projector should use this structure:

1. A numbered top-of-file policy.
2. The opening proof checklist, kept near the top of the file.
3. The imported proof checklist, kept with explicit owner file and theorem
   names.
4. The proof strategy section, kept as the local argument for this file.
5. A local theorem checklist when the file has named local proof kernels.
6. A completion plan for open proof gaps, or no completion plan when every
   owned and imported proof item is checked.
7. Runtime types near the top, so readers see the public surface early.
8. Proof vocabulary after runtime types: proof-facing ids, statements,
   decisions, reports, and spec result structs.
9. Numbered sections where each primary runtime function is followed by its
   proof handlers.
10. Branch-specific helpers for root, child, malformed, complete-report, and
    incomplete-report paths when those paths have different proof obligations.
11. Runtime bridge helpers for conversions between proof-facing and runtime
    types.
12. Tests ordered in the same narrative sequence as the file.

The checklist sections are part of the narrative. Do not replace them with
section headings or prose-only policy. The policy tells the reader the story;
the opening proof checklist records the safety/liveness claims; the imported
proof checklist records what this file relies on; the proof strategy explains
how the local code discharges its owned claims.

At minimum, keep these top sections visible before the implementation:

```text
Invariant checklist (Verus):
Owned invariant: ...
- [x] Safety: ...
- [ ] Safety: ...

Imported theorem checklist:
- [x] `owner::module`: theorem meaning. Proven in `path::theorem_name`.
- [ ] `owner::module`: theorem meaning. Owner: `path`, planned theorem `name`.

Local theorem checklist:
- [x] Local statement. Proven below by `proof_or_kernel_name`.

Proof strategy:
- Prove ...

Completion plan for unchecked items:
- Close ...
```

If the file has no unchecked items, omit the completion plan. If the file has no
local theorem checklist yet, say why. Do not omit the opening proof checklist,
imported proof checklist, or proof strategy section from a proof-targeted
projector.

Do not group all Verus specs first and all runtime code last. That makes the
reader assemble the proof manually. Keep each primary behavior close to the spec
and lemmas that justify it.

## Policy Block

Start with a policy that tells the reviewer what the projector admits and why.
For the current link family, the policy should read like this:

```rust
//! POLICY. A link is valid iff:
//!   1. CODEC. Its bytes decode canonically to exactly one `Link`, and its id
//!      is `hash(bytes)`.
//!   2. SHAPE. It is either a root, a child, or malformed.
//!   3. EXTRACT. Roots assert `valid_link(self,self)`; children assert
//!      `valid_link(self,root)` and need `valid_link(parent,root)`; malformed
//!      links assert nothing.
//!   4. CONTEXT. A child may validate only from exact validated parent/root
//!      context.
//!   5. PROJECT. A valid projection promotes only its own statement and emits
//!      no raw facts.
//!   6. STATE. Projection updates only this link id's read-model entry.
//!   7. COMPOSE. The local child step composes with core/replay provenance for
//!      supplied proof-facing same-root chains.
```

The checklist below the policy should remain concrete. Each invariant should
be user-significant or threat-model-significant, not a call trace. For each
open item, name the missing proof owner and the theorem or helper that should
close it.

## Section Outline

Use named phase headers and named subsections. Do not use numeric section
comments as the hierarchy; numbers make ordering visible, but they do not tell
the reader why a block exists. The policy/checklist preface may remain
top-level module docs rather than a code section.

```text
Opening Policy And Reader Map
   - Top-level policy.
   - Fact-family contract.
   - Opening proof checklist.
   - Imported theorem checklist.
   - Local theorem checklist.
   - Proof strategy.
   - Completion plan.

=== Vocabulary ===
Role: define the nouns before making claims.
Subsections:
   - Runtime Surface
   - Proof Vocabulary
   - Shape Predicates And Statement Helpers

=== Spec Models ===
Role: define intended meaning.
Subsections:
   - Projection Validity Model
   - Extraction Model
   - Report Fallback Model
   - Construction Proof Model
   - Update Application Model
   - Canonical Codec Model
   - Composition Model
   - Projected Report Model

=== Executable Kernels ===
Role: implement the spec models as verified running proof-facing functions.
Subsections:
   - Report Helper Kernel
   - Construction Kernel
   - Extraction Kernel
   - Update Application Kernel
   - Codec Kernels
   - Projected Id Vector Kernels
   - Projected Report Kernel
   - Projection Validity Kernel
   - Emitted-Fact Kernel

=== Lemmas ===
Role: package reusable theorem facts over the kernels.
Subsections:
   - Projection Lemmas
   - Output Ownership Lemmas
   - Construction Lemma
   - Update Application Lemma
   - Codec Lemmas
   - Projected Id Vector Lemmas
   - Composition Lemmas
   - Extraction Lemmas
   - Projection Statement Lemmas
   - Malformed Shape Lemmas
   - Projected Report Lemmas

=== Runtime Implementation ===
Role: route real Rust behavior through verified kernels.
Subsections:
   - Runtime Construction
   - Runtime Canonical Codec
   - Runtime Extraction
   - Runtime Projection Validity
   - Runtime Output And Read Model

=== Wiring And Boundary ===
Role: connect the fact-family implementation to the generic Projector trait.
Subsections:
   - Projector Trait Wiring
   - Runtime Bridge Helpers

Tests
    - Codec
    - Construction
    - Extraction
    - Projection
    - Update ownership
    - Report shape
    - Composition assumptions
```

## Primary Functions And Handlers

A section should lead with the runtime behavior or public semantic function.
Put proof-facing handlers immediately below it.

Good shape:

```text
Extraction
  link_edges
  link_semantic_root
  valid_link_key
  extraction_spec
  extract_link_core
  child_extraction_offer_and_need_same_root
  malformed_extraction_is_empty
```

Avoid this shape:

```text
All specs
All executable kernels
All proof lemmas
All runtime functions
```

The second shape is mechanically tidy but narratively expensive. It forces the
reader to jump across the file to understand one behavior.

## Branch Paths

Use named paths for proof branches, matching the poc-10 authority-path style.
For link, the natural paths are:

- Root path: no parent, no claimed root, valid as `valid_link(self,self)`.
- Child path: parent and claimed root present, valid only from exact
  `valid_link(parent,root)` context.
- Malformed path: exactly one of `prev` or `root` present, no edges and invalid.
- Complete report path: parent report exists, is complete, and has the same root.
- Incomplete report path: missing, incomplete, wrong-root, invalid, or malformed
  input produces singleton incomplete read-model state.

If a branch has different authority, state, or report consequences, give it a
small helper or a visible section. A single large match can be correct and still
make the proof hard to audit.

## Proof Narrative

The link proof should tell this story:

```text
A link fact is authority for at most one statement:
valid_link(self_id, root_id).

The codec binds self_id to canonical bytes. Extraction names the only statement
the fact may later promote, plus the exact parent/root dependency a child
needs. Projection can validate a root directly, but can validate a child only
from validated context for that exact parent/root statement. A valid projection
emits no new raw facts and updates only this link's read-model entry. Report
state records the same chain shape. Core/replay proofs are responsible for
turning this local same-root step into replay-wide validity.
```

This same shape should generalize to later fact families:

```text
fact declares a domain id
fact declares a dependency or authority id
projector requires validated context for that dependency
projector checks dependency.domain == fact.domain
projector emits validated statements only inside that same domain
projector updates only owner-scoped state
core/replay proves validated-context provenance and closure
```

## Tests

New or reorganized proof projector work should include realistic tests. The
tests should exercise the runtime behavior that the proof-facing helpers claim
to model.

For link, keep tests ordered most-central first:

1. Canonical codec round-trips accepted bytes and ids.
2. Deterministic construction preserves only explicit link parameters.
3. Root extraction emits exactly the self/root offer.
4. Child extraction emits same-root offer and parent/root need.
5. Child without same-root parent context is invalid.
6. Malformed shape emits no edges and is invalid.
7. Valid root and child projection preserve claimed root.
8. Projection emits no raw facts.
9. Updates are insert/ignore by owner id.
10. Complete reports derive only from complete same-root parent reports.

Tests may be Rust tests, documentation tests, or Verus checks, depending on the
change. Placeholder assertions are not enough.

## Review Checklist

Before handoff or review:

1. The top policy tells the same story as the code order.
2. Each primary runtime function has its proof handlers nearby.
3. Root, child, malformed, complete-report, and incomplete-report paths are
   visible.
4. Imported theorem checklist items name their owning file and theorem.
5. Local theorem checklist items name the proof function or executable kernel.
6. Open proof gaps state what remains and where it is owned.
7. Tests cover the runtime behavior being described or changed.
8. Run the relevant checks for the worktree.
9. Commit the completed work on that same worktree branch before handoff or
   review.
