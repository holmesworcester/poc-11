use std::fs;
use std::path::Path;
use std::process::Command;

fn source_text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()))
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn uncommented_source(text: &str) -> String {
    text.lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !trimmed.starts_with("//")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn doc_section(text: &str, heading: &str) -> String {
    let mut out = Vec::new();
    let mut in_section = false;
    for line in text.lines() {
        let trimmed = line.trim_start();
        let Some(rest) = trimmed.strip_prefix("//!") else {
            if in_section {
                break;
            }
            continue;
        };
        let body = rest.trim_start();
        if body == heading {
            in_section = true;
            continue;
        }
        if in_section
            && body.ends_with(':')
            && !body.starts_with("- ")
            && body.chars().next().is_some_and(|c| c.is_ascii_uppercase())
        {
            break;
        }
        if in_section {
            out.push(body.to_string());
        }
    }
    out.join("\n")
}

fn collect_rs_files(dir: &Path, out: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(dir).unwrap_or_else(|err| panic!("read dir {}: {err}", dir.display()))
    {
        let entry =
            entry.unwrap_or_else(|err| panic!("read dir entry in {}: {err}", dir.display()));
        let path = entry.path();
        if path.is_dir() {
            collect_rs_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn verus_proofs_do_not_use_escape_hatches() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs_files(&root.join("src"), &mut files);

    for path in files {
        let text = uncommented_source(&source_text(&path));
        for forbidden in [
            "assume(",
            "assume (",
            "admit();",
            "admit ();",
            "external_body",
            "axiom",
        ] {
            assert!(
                !text.contains(forbidden),
                "{} contains forbidden Verus proof escape hatch {forbidden:?}",
                path.strip_prefix(root).unwrap_or(&path).display()
            );
        }
    }
}

#[test]
fn verus_specs_are_not_public_open_by_default() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut files = Vec::new();
    collect_rs_files(&root.join("src"), &mut files);

    for path in files {
        let text = uncommented_source(&source_text(&path));
        assert!(
            !text.contains("pub open spec fn"),
            "{} contains public open spec function; keep specs private/open or public/closed unless review justifies abstraction leakage",
            path.strip_prefix(root).unwrap_or(&path).display()
        );
    }
}

#[test]
fn in_memory_projection_note_records_extract_project_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let note = source_text(&root.join("docs/research/in-memory-projection-bounded-replay.md"));
    let normalized = normalize_whitespace(&note);

    for required in [
        "persist facts and a syntactic needs/offers index; project the active range in memory; resolve cross-time matches by lookup",
        "fn extract(item: &Item) -> Vec<Edge>",
        "fn project(",
        "st: &Self::State",
        ") -> ProjectOutcome<Self::Update>",
        "fn update_owner(update: &Self::Update) -> FactId",
        "fn apply_update(st: &mut Self::State, update: Self::Update)",
        "`extract` is context-free by signature",
        "The closure rule: addresses must be self-contained",
        "every context address a fact will ever need must be carried in — or derivable from — that fact's own fields",
        "Those copied addresses are asserted routing hints, not authority",
        "`project` must compare them with validated parent/context facts before materializing state or effects",
        "A forged child can dirty the syntactic index with useless edges, but it cannot create validated state",
        "The index is the `Asserted` (dirty) layer",
        "They promote to **`Validated`** only when `project` validates the item in Pass 2",
        "socket, filesystem, and SQLite wrappers feed bytes through verified decode/admission",
        "errors must not create validated state",
        "relentlessly move as much behavior as possible into Verus-proven executable kernels",
        "the runtime turn itself can be proven",
        "`src/facts/` is proof-targeted",
        "Keep the poc-10 family-directory shape",
        "`src/facts/link/` should own family-local modules such as `api`, `project`, `cli`",
        "`src/helpers/` is the explicit trusted boundary",
        "Files without `_unproven` in `core` or `facts` must have their invariants covered by Verus-verified executable code",
    ] {
        assert!(
            normalized.contains(required),
            "in-memory projection note is missing boundary detail {required:?}"
        );
    }
}

#[test]
fn proof_plan_records_unproven_to_unsuffixed_migration_and_link_domain_theorem() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let plan = source_text(&root.join("docs/proof-plan.md"));
    let normalized = normalize_whitespace(&plan);

    for required in [
        "choose code shapes that let behavior move from `_unproven` files into Verus-verified executable kernels",
        "There is no `_proven` suffix",
        "`src/core/*_unproven.rs` contains the current operational core shell",
        "`src/facts/link/project_unproven.rs` contains link codec, extraction, and projection together",
        "Its local stylized link invariants are covered by Verus kernels in the running file; end-to-end core/replay graph composition is still open",
        "`src/core/effects_unproven.rs` and `src/core/turn_unproven.rs` are the current staging surface",
        "`src/core/runtime_unproven.rs` is the current daemon/IO loop",
        "It stays separate from `turn` so the deterministic queue/effect step can be proven without proving OS progress",
        "concrete SQLite lives in `src/helpers/sqlite_unproven.rs`",
        "`src/core/turn.rs`: deterministic `State + Input -> State + Effects` transition",
        "`src/facts/link/project.rs`: target name after the current `src/facts/link/project_unproven.rs` closes its remaining end-to-end proof gaps",
        "Do not split out a parallel proven copy",
        "`src/helpers/*_unproven.rs`: narrow trusted adapters",
        "The `_unproven` naming rule is repository policy, not a semantic Verus theorem",
        "Enforce it with source-tree tests and review gates",
        "Core proofs are about all possible fact families routed through the engine",
        "Current link proof kernels live beside the running implementation in `src/facts/link/project_unproven.rs`",
        "That file has not completed its unsuffixed migration",
        "local projector kernels are proved, while real core/replay graph composition remains open",
        "Verus model proves accepted layout shape",
        "exact proof-facing id-vector construction",
        "Source-file invariant checklists should state user-significant or threat-model-significant properties first",
        "Avoid checklists that are only call traces",
        "Every checklist item must be labeled `Safety:` or `Liveness:`",
        "Use `Safety:` for properties that rule out bad states",
        "Use `Liveness:` only for progress claims",
        "Do not put OS/socket/filesystem progress in a Verus invariant",
        "Each checklist should be followed by",
        "`Imported theorem checklist`: a `[x]` / `[ ]` checklist of external facts this",
        "[x]` entries must name the file plus function/proof",
        "[ ]` entries must name the owner file and the planned",
        "`Proof strategy`: the local argument needed in this file",
        "Each invariant has one proof owner",
        "`core::engine` | In-memory id/body relation, running readiness/promotion rule, validated-context provenance, promotion authority, emitted-fact re-entry, and ongoing queue-step safety.",
        "`core::turn` | Deterministic turn scheduling, effect-result application into the engine, and the future fair-input liveness model.",
        "`facts::link::project_unproven`",
        "In the current root/domain model, a root link (`prev=None, root=None`) is valid as `valid_link(self_id, self_id)`",
        "A child link is valid only when validated context contains `valid_link(parent_id, claimed_root_id)`",
        "Malformed `prev`/`root` combinations emit no edges and cannot validate",
        "The link projector proves any local valid projection statement is for its own fact id and semantic root",
        "The stronger validated-store statement-to-owner theorem remains open until core proves engine/replay provenance over modeled state",
        "parent-author, device, or admin-grant relationships must be explicit link/fact fields before their preservation can be a link theorem",
        "The target composition theorem is",
        "core drain-prefix validated-context provenance",
        "core replay dependency-closure soundness",
        "link's parent/root projection contract",
        "Multiple anchors are allowed; the starter model does not prove global root uniqueness",
        "valid_link(link_id, root_id)",
        "no cross-root splice validates",
        "Instantiate the core transitive-validity theorem with the link projection contract",
        "Fair-input liveness model",
        "model helper/storage results and transport arrivals as explicit fair inputs",
        "scripts/run_verus.sh` must fail rather than claim success when no running-code Verus proof target exists",
        "A file loses `_unproven` only after its invariant-bearing behavior is covered by Verus-verified executable code",
        "statement-to-owner lemma",
        "ancestry chain to its claimed anchor by induction over `prev`",
    ] {
        assert!(
            normalized.contains(required),
            "proof plan is missing migration detail {required:?}"
        );
    }

    for stale in [
        "types_proven.rs",
        "turn_proven.rs",
        "project_proven.rs",
        "author_proven.rs",
        "composition wrappers are proved",
        "That file has completed its unsuffixed migration",
    ] {
        assert!(
            !normalized.contains(stale),
            "proof plan should use unsuffixed proven files instead of stale target {stale:?}"
        );
    }
}

#[test]
fn proof_projector_style_guide_records_narrative_structure() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let guide = source_text(&root.join("docs/proof-projector-style.md"));
    let normalized = normalize_whitespace(&guide);

    for required in [
        "Write projector proof files as a story",
        "Bytes establish identity",
        "Shape determines which semantic path applies",
        "Extraction declares exactly what the fact may later claim and need",
        "Validated context supplies authority",
        "Projection promotes only the statement justified by that authority",
        "Projector state records only owned read-model consequences",
        "Composition imports core/replay provenance",
        "A numbered top-of-file policy",
        "The opening proof checklist, kept near the top of the file",
        "The imported proof checklist, kept with explicit owner file and theorem names",
        "The proof strategy section, kept as the local argument for this file",
        "The checklist sections are part of the narrative",
        "Do not replace them with section headings or prose-only policy",
        "Invariant checklist (Verus):",
        "Imported theorem checklist:",
        "Local theorem checklist:",
        "Proof strategy:",
        "Completion plan for unchecked items:",
        "Do not omit the opening proof checklist, imported proof checklist, or proof strategy section",
        "Runtime types near the top",
        "Proof vocabulary after runtime types",
        "each primary runtime function is followed by its proof handlers",
        "Do not group all Verus specs first and all runtime code last",
        "POLICY. A link is valid iff",
        "CODEC. Its bytes decode canonically",
        "SHAPE. It is either a root, a child, or malformed",
        "EXTRACT. Roots assert `valid_link(self,self)`",
        "CONTEXT. A child may validate only from exact validated parent/root",
        "PROJECT. A valid projection promotes only its own statement and emits",
        "no raw facts",
        "STATE. Projection updates only this link id's read-model entry",
        "COMPOSE. The local child step composes with core/replay provenance",
        "Primary Functions And Handlers",
        "Avoid this shape",
        "All specs All executable kernels All proof lemmas All runtime functions",
        "Branch Paths",
        "Root path",
        "Child path",
        "Malformed path",
        "Complete report path",
        "Incomplete report path",
        "A link fact is authority for at most one statement",
        "The codec binds self_id to canonical bytes",
        "Core/replay proofs are responsible",
        "fact declares a domain id",
        "projector requires validated context for that dependency",
        "projector emits validated statements only inside that same domain",
        "New or reorganized proof projector work should include realistic tests",
        "Canonical codec round-trips accepted bytes and ids",
        "Updates are insert/ignore by owner id",
        "Complete reports derive only from complete same-root parent reports",
        "Commit the completed work on that same worktree branch before handoff or review",
    ] {
        assert!(
            normalized.contains(required),
            "proof projector style guide is missing required narrative detail {required:?}"
        );
    }

    let plan = source_text(&root.join("docs/proof-plan.md"));
    assert!(
        plan.contains("docs/proof-projector-style.md"),
        "proof plan should point fact-family projector authors at the style guide"
    );
}

#[test]
fn link_project_file_follows_proof_projector_narrative_sections() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = source_text(&root.join("src/facts/link/project_unproven.rs"));
    let normalized = normalize_whitespace(&project);

    for required in [
        "POLICY. A link is valid iff",
        "1. CODEC. Its bytes decode canonically to exactly one `Link`",
        "2. SHAPE. It is either a root, a child, or malformed",
        "3. EXTRACT. Roots assert `valid_link(self,self)`",
        "4. CONTEXT. A child may validate only from exact validated parent/root",
        "5. PROJECT. A valid projection promotes only its own statement and emits no",
        "raw facts",
        "6. STATE. Projection updates only this link id's read-model entry",
        "7. COMPOSE. The local child step composes with core/replay provenance",
        "Invariant checklist (Verus):",
        "Imported theorem checklist:",
        "Local theorem checklist:",
        "Proof strategy:",
        "Completion plan for unchecked items:",
        "2. Runtime Surface.",
        "3. Proof Vocabulary.",
        "3a. Shape Predicates And Statement Helpers.",
        "4. Construction.",
        "4. Construction Proof Model.",
        "4. Construction Kernel.",
        "4a. Construction Lemma.",
        "5. Canonical Codec.",
        "5. Canonical Codec Model.",
        "5. Codec Kernels.",
        "5a. Codec Lemmas.",
        "6. Extraction.",
        "6. Extraction Model.",
        "6. Extraction Kernel.",
        "6a. Extraction Lemmas.",
        "7. Projection Validity.",
        "7. Projection Validity Model.",
        "7. Projection Validity Kernel.",
        "7a. Projection Lemmas.",
        "8. Output And Read Model.",
        "8c. Projected Report Model.",
        "8g. Projected Report Kernel.",
        "8l. Projected Report Lemmas.",
        "9. Composition Model.",
        "9. Composition Kernel.",
        "9a. Composition Lemma.",
        "10. Runtime Bridge Helpers.",
        "Projector trait wiring.",
        "Each method delegates to the",
        "sectioned helpers above",
    ] {
        assert!(
            normalized.contains(required),
            "link project file is missing proof-projector narrative section {required:?}"
        );
    }

    let runtime_surface = project
        .find("// 2. Runtime Surface.")
        .expect("runtime surface section");
    let proof_vocabulary = project
        .find("// 3. Proof Vocabulary.")
        .expect("proof vocabulary section");
    let construction = project
        .find("// 4. Construction.")
        .expect("construction section");
    let bridge = project
        .find("// 10. Runtime Bridge Helpers.")
        .expect("runtime bridge section");
    assert!(
        runtime_surface < proof_vocabulary
            && proof_vocabulary < construction
            && construction < bridge,
        "link project sections should read policy, runtime surface, proof vocabulary, behavior, bridge"
    );
}

#[test]
fn proof_target_files_have_verus_invariant_checklists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        "src/core/admit_unproven.rs",
        "src/core/effects_unproven.rs",
        "src/core/engine_unproven.rs",
        "src/core/index_unproven.rs",
        "src/core/item_unproven.rs",
        "src/core/mod.rs",
        "src/core/offer_unproven.rs",
        "src/core/play_unproven.rs",
        "src/core/projector_unproven.rs",
        "src/core/runtime_unproven.rs",
        "src/core/turn_unproven.rs",
        "src/core/typestate_unproven.rs",
        "src/facts/link/api_unproven.rs",
        "src/facts/link/cli_unproven.rs",
        "src/facts/link/mod.rs",
        "src/facts/link/project_unproven.rs",
    ];

    for file in files {
        let text = source_text(&root.join(file));
        assert!(
            text.contains("Invariant checklist (Verus):"),
            "{file} is missing its Verus invariant checklist"
        );
        assert!(
            text.contains("Owned invariant:"),
            "{file} must name the invariant owned by that file"
        );
        assert!(
            text.contains("Imported theorem checklist:"),
            "{file} must list imported theorem dependencies as a proven-status checklist"
        );
        assert!(
            text.contains("Proof strategy:"),
            "{file} must describe a local proof strategy"
        );
        let mut in_invariant_checklist = false;
        let mut in_imported_checklist = false;
        let mut imported_items = 0usize;
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.contains("Invariant checklist (Verus):") {
                in_invariant_checklist = true;
                in_imported_checklist = false;
                continue;
            }
            if trimmed.contains("Imported theorem checklist:") {
                in_invariant_checklist = false;
                in_imported_checklist = true;
                continue;
            }
            if trimmed.contains("Proof strategy:") {
                in_invariant_checklist = false;
                in_imported_checklist = false;
                continue;
            }
            if in_invariant_checklist && trimmed.starts_with("//! - [ ] ") {
                assert!(
                    trimmed.contains("- [ ] Safety:") || trimmed.contains("- [ ] Liveness:"),
                    "{file}:{} checklist item must be labeled Safety or Liveness",
                    idx + 1
                );
            } else if in_invariant_checklist && trimmed.starts_with("//! - [x] ") {
                assert!(
                    trimmed.contains("- [x] Safety:") || trimmed.contains("- [x] Liveness:"),
                    "{file}:{} checklist item must be labeled Safety or Liveness",
                    idx + 1
                );
            } else if in_imported_checklist && trimmed.starts_with("//! - ") {
                imported_items += 1;
                assert!(
                    trimmed.starts_with("//! - [ ] ") || trimmed.starts_with("//! - [x] "),
                    "{file}:{} imported theorem item must be marked [ ] or [x]",
                    idx + 1
                );
            }
        }
        assert!(
            imported_items > 0,
            "{file} must have at least one imported theorem checklist item"
        );
    }

    let engine = normalize_whitespace(&source_text(&root.join("src/core/engine_unproven.rs")));
    for required in [
        "Owned invariant: validated-context provenance and ongoing engine safety",
        "Safety: every in-memory fact is paired with the id derived from its",
        "bytes before the engine hands it to a projector",
        "Safety: a projector is called only after every asserted need has a",
        "matching validated offer",
        "it receives only validated offers",
        "Safety: every validated offer in the running engine state is owned by a",
        "Safety: every admit/query/project/wake step preserves these invariants",
        "Safety: raw bytes returned in `ProjectOutcome.emitted` do not inherit",
        "reject any update whose owner is not the",
        "projected fact",
        "Imported theorem checklist:",
        "`core::item`: fact ids identify canonical bytes",
        "`src/core/item_unproven.rs::fact_id_content_address`",
        "`core::offer`: asserted-to-validated promotion preserves edge address",
        "`src/core/offer_unproven.rs::validate_preserves_offer_address`",
        "`engine_transition_preserves_validated_context_provenance`",
        "Proof strategy:",
    ] {
        assert!(
            engine.contains(required),
            "core engine checklist is missing {required:?}"
        );
    }

    let offer = normalize_whitespace(&source_text(&root.join("src/core/offer_unproven.rs")));
    for required in [
        "Owned invariant: edge representation and promotion shape",
        "Safety: matching depends only on `(role, scope, key)`",
        "Verified below in this file",
        "Safety: promotion preserves the asserted edge's address and metadata",
        "src/core/offer_unproven.rs::asserted_edge_address_shape",
        "src/core/offer_unproven.rs::validate_preserves_offer_address",
        "src/core/offer_unproven.rs::validated_offer_typestate_only",
    ] {
        assert!(
            offer.contains(required),
            "core offer checklist is missing {required:?}"
        );
    }

    let typestate =
        normalize_whitespace(&source_text(&root.join("src/core/typestate_unproven.rs")));
    for required in [
        "Owned invariant: validated context representation",
        "Safety: `Context` can contain only `Offer<Validated>`",
        "Safety: `has_offer` answers only whether an exact validated match",
        "src/core/typestate_unproven.rs::context_validated_only",
        "src/core/typestate_unproven.rs::context_lookup_exact",
    ] {
        assert!(
            typestate.contains(required),
            "core typestate checklist is missing {required:?}"
        );
    }

    let admit = normalize_whitespace(&source_text(&root.join("src/core/admit_unproven.rs")));
    for required in [
        "Owned invariant: new/local fact admission creates only asserted state",
        "Safety: admission creates an `Admitted` token and asserted storage state",
        "creates no validity, validated offer, or validated context",
        "Safety: the admitted token's id/body relation is derived from",
        "`core::item` content addressing",
        "`src/core/item_unproven.rs::fact_id_content_address`",
        "extraction exactness is proved by the fact-family projector",
        "Imported theorem checklist:",
        "Proof strategy:",
    ] {
        assert!(
            admit.contains(required),
            "core admission checklist is missing model invariant {required:?}"
        );
    }

    let link = normalize_whitespace(&source_text(
        &root.join("src/facts/link/project_unproven.rs"),
    ));
    for required in [
        "Owned invariant: link-family semantics and its `Projector` implementation",
        "Safety: canonical link identity",
        "canonical_link_identity",
        "Safety: project-owned construction",
        "Verified below in this file",
        "Safety: well-formed parent naming",
        "extraction asserts exactly one need for `valid_link(parent_id, root_id)`",
        "Safety: malformed `prev`/`root` combinations assert no edges",
        "Safety: starter validity rule",
        "valid exactly when validated context contains",
        "`valid_link(parent_id, root_id)` for the child's declared parent and",
        "root/domain ids",
        "Safety: same-root/domain preservation",
        "the child's promoted self-offer carries that same root/domain",
        "- [ ] Safety: end-to-end statement-to-owner",
        "validated-store provenance theorem is still owned by core/replay",
        "valid_projection_statement_to_owner_and_root",
        "Safety: projection output update ownership",
        "Safety: update application scope",
        "`apply_update` is insert/ignore by `link_id`",
        "link_from_params_constructs_only_link_fields",
        "apply_update_is_insert_ignore_by_link_id",
        "Safety: projected chain entry shape",
        "each projection may create only the",
        "current fact's `ProjectedLink`",
        "`ProjectedLink` is read-model state, not validity",
        "evidence",
        "Safety: no emitted-fact authority leak",
        "Prove `update_owner` returns the update's owner id exactly",
        "- [ ] Safety: end-to-end composition with core",
        "The local link theorem is a conditional induction step",
        "not the whole",
        "replay/graph invariant",
        "valid_link_composes_with_parent_chain",
        "Imported theorem checklist:",
        "`core::item`: fact ids are content addresses for canonical bytes",
        "`src/core/item_unproven.rs::fact_id_content_address`",
        "`core::offer`: asserted edge constructors and match addresses have fixed",
        "`src/core/offer_unproven.rs::asserted_edge_address_shape`",
        "`core::typestate`: `Context::has_offer` is exact validated-offer lookup",
        "`src/core/typestate_unproven.rs::context_lookup_exact`",
        "`core::engine`: abstract context/promotion gates relate context offers",
        "`engine_transition_preserves_validated_context_provenance`",
        "planned theorem `engine_drain_prefix_sound`",
        "planned theorem `replay_reports_engine_validity`",
        "admit_establishes_id_body",
        "`engine_transition_preserves_validated_context_provenance`",
        "Proof strategy:",
        "Prove the local statement-to-owner lemma",
        "Prove the local same-root parent-chain step by induction",
        "without a caller-provided boolean",
    ] {
        assert!(
            link.contains(required),
            "link project checklist is missing {required:?}"
        );
    }

    let api = normalize_whitespace(&source_text(&root.join("src/facts/link/api_unproven.rs")));
    for required in [
        "Owned invariant: link reporting boundary",
        "Safety: report fields are read from projector-maintained `LinkState`",
        "after replay; this module does not compute them by walking persisted",
        "Safety: missing requested facts return `present=false`",
        "malformed facts",
        "return a replay/decode error before any report can be produced",
        "Safety: `complete` means replay projected the requested head valid",
        "Prove `chain_report` calls replay first",
        "report from `LinkState.projected`",
    ] {
        assert!(
            api.contains(required),
            "link API checklist is missing {required:?}"
        );
    }
}

#[test]
fn item_verified_kernel_is_running_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let item = source_text(&root.join("src/core/item_unproven.rs"));

    for required in [
        "verus!",
        "pub fn fact_id_contract_core",
        "fact_id_content_address",
        "fact_id_crypto_assumption",
    ] {
        assert!(
            item.contains(required),
            "item file is missing verified-kernel detail {required:?}"
        );
    }

    for required in ["fact_id_contract_core()", "crypto_fact_id(bytes)"] {
        assert!(
            item.contains(required),
            "runtime item code does not delegate to verified-kernel detail {required:?}"
        );
    }
}

#[test]
fn typestate_verified_kernel_is_running_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let typestate = source_text(&root.join("src/core/typestate_unproven.rs"));

    for required in [
        "verus!",
        "pub fn context_shape_core",
        "pub fn context_lookup_core",
        "context_validated_only",
        "context_lookup_exact",
    ] {
        assert!(
            typestate.contains(required),
            "typestate file is missing verified-kernel detail {required:?}"
        );
    }

    for required in [
        "context_shape_core()",
        "context_lookup_core(o.role == role, &o.key == key).matched",
    ] {
        assert!(
            typestate.contains(required),
            "runtime typestate code does not delegate to verified-kernel detail {required:?}"
        );
    }
}

#[test]
fn offer_verified_kernel_is_running_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let offer = source_text(&root.join("src/core/offer_unproven.rs"));

    for required in [
        "verus!",
        "pub fn asserted_edge_shape_core",
        "pub fn asserted_edge_core",
        "pub fn validate_shape_core",
        "pub fn validate_edge_core",
        "asserted_edge_address_shape",
        "validate_preserves_offer_address",
        "validated_offer_typestate_only",
    ] {
        assert!(
            offer.contains(required),
            "offer file is missing verified-kernel detail {required:?}"
        );
    }

    for required in [
        "asserted_edge_core(edge_kind_to_core(kind))",
        "validate_edge_core(TypedEdgeCore",
    ] {
        assert!(
            offer.contains(required),
            "runtime offer code does not delegate to verified-kernel detail {required:?}"
        );
    }
}

#[test]
fn index_effects_admit_verified_kernels_are_running_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let index = source_text(&root.join("src/core/index_unproven.rs"));
    for required in [
        "verus!",
        "pub fn index_contract_core",
        "index_asserted_only",
        "index_lookup_discovery_only",
        "index_lookup_contract",
    ] {
        assert!(
            index.contains(required),
            "index file is missing verified-kernel detail {required:?}"
        );
    }

    let effects = source_text(&root.join("src/core/effects_unproven.rs"));
    for required in [
        "verus!",
        "pub fn effect_payload_core",
        "effect_payloads_carry_no_validated_state",
    ] {
        assert!(
            effects.contains(required),
            "effects file is missing verified-kernel detail {required:?}"
        );
    }

    let admit = source_text(&root.join("src/core/admit_unproven.rs"));
    for required in [
        "verus!",
        "pub fn admission_core",
        "admit_establishes_id_body",
        "admission_core(durable)",
        "admission.writes_fact_bytes",
    ] {
        assert!(
            admit.contains(required),
            "admission file is missing verified-kernel/runtime detail {required:?}"
        );
    }
}

#[test]
fn engine_turn_play_proof_status_is_honest() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));

    let engine = source_text(&root.join("src/core/engine_unproven.rs"));
    for required in [
        "Introduce a proof model and state predicate",
        "fact_id(&bytes) != id",
        "P::encode(&item) != bytes",
        "if P::update_owner(update) != id",
        "self.promoted_offers.insert((id, addr))",
        "P::decode(&emitted.bytes)",
    ] {
        assert!(
            engine.contains(required),
            "engine file is missing verified-kernel/runtime detail {required:?}"
        );
    }
    for forbidden in [
        "pub fn engine_step_core",
        "pub fn engine_drain_prefix_core",
        "pub fn engine_admission_core",
        "pub fn engine_lookup_core",
        "pub fn engine_project_gate_core",
        "pub fn engine_promotion_uniqueness_core",
        "pub fn engine_emitted_fact_core",
        "pub proof fn engine_step_preserves_invariant",
        "pub proof fn engine_drain_prefix_sound",
        "pub proof fn engine_admit_local_establishes_id_body",
        "pub proof fn engine_admit_loaded_establishes_id_body",
        "pub proof fn engine_lookup_is_discovery_only",
        "pub proof fn engine_promotes_only_valid_owner_offers",
        "pub proof fn engine_context_offers_have_valid_owners",
        "EngineStepCore",
        "EngineDrainPrefixCore",
        "EngineAdmissionCore",
        "EngineLookupCore",
        "EngineProjectGateCore",
        "EnginePromotionUniquenessCore",
        "EngineEmittedFactCore",
    ] {
        assert!(
            !engine.contains(forbidden),
            "engine file must not expose boolean-summary proof wrapper {forbidden:?}"
        );
    }

    let turn = source_text(&root.join("src/core/turn_unproven.rs"));
    for required in [
        "verus!",
        "pub fn turn_core",
        "turn_surface_contract",
        "effect_payload_core(",
        "index_contract_core()",
    ] {
        assert!(
            turn.contains(required),
            "turn file is missing verified-kernel/runtime detail {required:?}"
        );
    }

    let play = source_text(&root.join("src/core/play_unproven.rs"));
    for required in [
        "verus!",
        "pub fn replay_report_core",
        "replay_report_surface_contract",
        "replay_report_core(true, true, false)",
        "replay_report_core(true, true, true)",
    ] {
        assert!(
            play.contains(required),
            "play file is missing verified-kernel/runtime detail {required:?}"
        );
    }
}

#[test]
fn link_project_verified_kernel_is_running_code() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = source_text(&root.join("src/facts/link/project_unproven.rs"));
    let manifest = source_text(&root.join("Cargo.toml"));

    for required in [
        "verus!",
        "canonical_link_identity",
        "link_codec_layout_core",
        "codec_layout_rejects_bad_tag",
        "codec_layout_rejects_bad_flags",
        "codec_layout_rejects_truncation",
        "pub fn project_link_core",
        "pub fn extract_link_core",
        "child_extraction_offer_and_need_same_root",
        "valid_projection_statement_equals_extracted_offer",
        "valid_child_requires_validated_same_root_parent",
        "valid_link_composes_with_parent_chain",
        "projection_update_owner_is_self",
        "valid_projection_statement_owned_by_projected_link",
        "valid_projection_statement_to_owner_and_root",
        "link_from_params_constructs_only_link_fields",
        "apply_update_is_insert_ignore_by_link_id",
        "pub fn projected_report_core",
        "complete_child_report_requires_complete_same_root_parent",
        "singleton_projected_ids_core",
        "child_projected_ids_core",
        "singleton_projected_ids_are_exact",
        "child_projected_ids_are_parent_plus_self",
        "pub fn link_emitted_fact_count_core",
        "valid_child_preserves_claimed_root",
        "malformed_projection_is_invalid",
    ] {
        assert!(
            project.contains(required),
            "link project file is missing verified-kernel detail {required:?}"
        );
    }

    for required in [
        "project_link_core(",
        "extract_link_core(",
        "link_from_params_core(",
        "link_update_apply_core(",
        "link_codec_identity_core(",
        "link_codec_layout_core(",
        "singleton_projected_ids_core(",
        "child_projected_ids_core(",
        "link_chain_composition_core(",
        "link_core_for(id, l.prev, l.root)",
        "validity_from_core(projection.validity)",
        "Verified below in this file",
    ] {
        assert!(
            project.contains(required),
            "runtime link projector does not delegate to local verified-kernel detail {required:?}"
        );
    }

    assert!(
        root.join("src/facts/link/project_unproven.rs").exists(),
        "project_unproven.rs must remain while core graph composition is open"
    );
    assert!(
        !root.join("src/facts/link/project.rs").exists(),
        "project.rs must not exist until every link project invariant is proven"
    );
    for forbidden in [
        "end_to_end_validated_link_offer_statement_to_owner",
        "end_to_end_valid_link_has_same_root_chain",
    ] {
        assert!(
            !project.contains(forbidden),
            "link project must not expose overclaiming wrapper {forbidden:?}"
        );
    }

    assert!(
        manifest.contains("[package.metadata.verus]") && manifest.contains("verify = true"),
        "Cargo manifest must keep cargo-verus verification enabled"
    );
}

#[test]
fn link_project_status_records_review_findings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = source_text(&root.join("src/facts/link/project_unproven.rs"));
    let normalized = normalize_whitespace(&project);

    for required in [
        "- [x] Safety: canonical link identity",
        "accepted link bytes have the canonical",
        "`tag | has_prev | prev[32]? | has_root | root[32]? | content` layout",
        "malformed tags/flags/truncation are rejected",
        "link_codec_layout_core",
        "codec_layout_rejects_truncation",
        "- [ ] Safety: end-to-end statement-to-owner",
        "validated-store provenance theorem is still owned by core/replay",
        "- [x] Safety: projected chain entry shape",
        "each projection may create only the",
        "current fact's `ProjectedLink`",
        "`ProjectedLink` is read-model state, not validity",
        "evidence",
        "singleton_projected_ids_core",
        "child_projected_ids_core",
        "child_projected_ids_are_parent_plus_self",
        "- [ ] Safety: end-to-end composition with core",
        "The local link theorem is a conditional induction step, not the whole",
        "replay/graph invariant",
        "Completion plan for unchecked items:",
        "Replace the caller-supplied `parent_chain_to_anchor: bool`",
        "engine_drain_prefix_sound",
        "replay_reports_engine_validity",
        "admit_establishes_id_body",
        "Rename this file to `project.rs` only after those end-to-end invariants are",
    ] {
        assert!(
            normalized.contains(required),
            "link project proof status should record audit finding detail {required:?}"
        );
    }
}

#[test]
fn link_project_keeps_local_theorems_out_of_imported_checklist() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let project = source_text(&root.join("src/facts/link/project_unproven.rs"));

    let imported = doc_section(&project, "Imported theorem checklist:");
    for forbidden in [
        "Local link same-root extraction/projection kernel",
        "Local link conditional composition step",
        "Local link output/read-model kernel",
    ] {
        assert!(
            !imported.contains(forbidden),
            "local theorem {forbidden:?} must not be listed as an imported theorem"
        );
    }

    let local = doc_section(&project, "Local theorem checklist:");
    for required in [
        "Local link same-root extraction/projection kernel",
        "Local link conditional composition step",
        "Local link output/read-model kernel",
    ] {
        assert!(
            local.contains(required),
            "local theorem checklist is missing {required:?}"
        );
    }
}

#[test]
fn link_fact_family_contracts_are_strict_and_role_local() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let files = [
        "src/facts/link/mod.rs",
        "src/facts/link/api_unproven.rs",
        "src/facts/link/cli_unproven.rs",
        "src/facts/link/project_unproven.rs",
    ];

    for file in files {
        let text = source_text(&root.join(file));
        assert!(
            text.contains("Fact-family contract (do not weaken):"),
            "{file} is missing the strict fact-family contract"
        );
        assert!(
            text.contains("do not weaken") || text.contains("Do not weaken"),
            "{file} must make the contract's stability explicit"
        );
    }

    let family_mod = uncommented_source(&source_text(&root.join("src/facts/link/mod.rs")));
    assert!(
        !root.join("src/facts/link/author_unproven.rs").exists(),
        "link author module must not be reintroduced; deterministic construction belongs in project"
    );
    assert!(
        !family_mod.contains("author_unproven"),
        "link/mod.rs must not export an author module"
    );
    for forbidden in ["pub fn ", "pub struct ", "pub enum "] {
        assert!(
            !family_mod.contains(forbidden),
            "link/mod.rs must remain re-export-only; found {forbidden:?}"
        );
    }

    let api = uncommented_source(&source_text(&root.join("src/facts/link/api_unproven.rs")));
    assert!(api.contains("pub fn chain_report("));
    assert!(api.contains("Replay::<LinkProjector>::new"));
    for forbidden in [
        "link_from_params",
        "admit::<",
        "insert_asserted",
        "flush_fact",
        "project_one",
        "Context",
        "Offer<Validated>",
    ] {
        assert!(
            !api.contains(forbidden),
            "reporting must not contain construction/projection concern {forbidden:?}"
        );
    }

    let cli = uncommented_source(&source_text(&root.join("src/facts/link/cli_unproven.rs")));
    assert!(cli.contains("link_from_params(at, prev, root, label)"));
    assert!(
        !cli.contains("chain_report(idx, parent)"),
        "CLI construction must not derive child root from reports"
    );
    assert!(cli.contains("admit::<LinkProjector>"));
    assert!(cli.contains("let r = chain_report(idx, id)?;"));
    for forbidden in [
        "Link {",
        "insert_asserted",
        "flush_fact",
        "LinkState",
        "Validity",
        "Context",
        "Offer<Validated>",
        ".project(",
    ] {
        assert!(
            !cli.contains(forbidden),
            "CLI must not contain fact semantics/proof concern {forbidden:?}"
        );
    }

    let project = uncommented_source(&source_text(
        &root.join("src/facts/link/project_unproven.rs"),
    ));
    for required in [
        "pub struct Link",
        "pub struct LinkState",
        "pub struct LinkProjector",
        "pub fn link_from_params(",
        "impl Projector for LinkProjector",
        "fn encode(",
        "fn decode(",
        "fn extract(",
        "fn project(",
    ] {
        assert!(
            project.contains(required),
            "project module is missing semantic owner item {required:?}"
        );
    }
    for forbidden in [
        "Index",
        "SqliteIndex",
        "admit::<",
        "chain_report",
        "author(",
        "replay::<",
        "to_hex",
        "from_hex",
    ] {
        assert!(
            !project.contains(forbidden),
            "project module must not contain storage/UI/app-admission concern {forbidden:?}"
        );
    }
}

#[test]
fn proof_strategy_collector_extracts_source_blocks() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let out = Command::new("python3")
        .arg(root.join("scripts/collect_proof_strategies.py"))
        .current_dir(root)
        .output()
        .expect("run proof strategy collector");

    assert!(
        out.status.success(),
        "collector failed\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("# Proof Strategy Extract"));
    assert!(stdout.contains("src/core/engine_unproven.rs"));
    assert!(stdout.contains("src/facts/link/project_unproven.rs"));
    assert!(stdout.contains("statement-to-owner lemma"));
}
