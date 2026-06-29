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

#[test]
fn in_memory_projection_note_records_extract_project_boundary() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let note = source_text(&root.join("docs/research/in-memory-projection-bounded-replay.md"));
    let normalized = normalize_whitespace(&note);

    for required in [
        "persist facts and a syntactic needs/offers index; project the active range in memory; resolve cross-time matches by lookup",
        "fn extract(item: &Item) -> Vec<Edge>",
        "fn project(item: &Item, ctx: Context<Validated>) -> (State, Effects)",
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
        "`src/facts/link/project_unproven.rs` keeps link codec, extraction, and projection together",
        "`src/core/effects_unproven.rs` and `src/core/turn_unproven.rs` are the current staging surface",
        "`src/core/runtime_unproven.rs` is the current daemon/IO loop",
        "It stays separate from `turn` so the deterministic queue/effect step can be proven without proving OS progress",
        "concrete SQLite lives in `src/helpers/sqlite_unproven.rs`",
        "`src/core/turn.rs`: deterministic `State + Input -> State + Effects` transition",
        "`src/facts/link/project.rs`: verified link codec, canonical encode/decode, deterministic typed construction from explicit parameters, extraction, projection validity",
        "`src/helpers/*_unproven.rs`: narrow trusted adapters",
        "The `_unproven` naming rule is repository policy, not a semantic Verus theorem",
        "Enforce it with source-tree tests and review gates",
        "Core proofs are about all possible fact families routed through the engine",
        "Link proofs live in `src/facts/link/project.rs` because only the link family defines what roots, parents, and ancestry mean",
        "Source-file invariant checklists should state user-significant or threat-model-significant properties first",
        "Avoid checklists that are only call traces",
        "Every checklist item must be labeled `Safety:` or `Liveness:`",
        "Use `Safety:` for properties that rule out bad states",
        "Use `Liveness:` only for progress claims",
        "Do not put OS/socket/filesystem progress in a Verus invariant",
        "Each checklist should be followed by",
        "`Imported theorems`: the external facts this proof depends on",
        "`Proof strategy`: the local argument needed in this file",
        "Each invariant has one proof owner",
        "`core::engine` | In-memory id/body relation, running readiness/promotion rule, validated-context provenance, promotion authority, emitted-fact re-entry, and ongoing queue-step safety.",
        "`core::turn` | Deterministic turn scheduling, effect-result application into the engine, and the future fair-input liveness model.",
        "`facts::link::project` | Link-family implementation of the projector contract and current same-root parent-chain validity theorem.",
        "In the current root/domain model, a root link (`prev=None, root=None`) is valid as `valid_link(self_id, self_id)`",
        "A child link is valid only when validated context contains `valid_link(parent_id, claimed_root_id)`",
        "Malformed `prev`/`root` combinations emit no edges and cannot validate",
        "A validated `valid_link(link_id, root_id)` statement is owned by a valid link fact",
        "parent-author, device, or admin-grant relationships must be explicit link/fact fields before their preservation can be a link theorem",
        "The current composition theorem is",
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
    ] {
        assert!(
            !normalized.contains(stale),
            "proof plan should use unsuffixed proven files instead of stale target {stale:?}"
        );
    }
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
            text.contains("Imported theorems:"),
            "{file} must list imported theorem dependencies"
        );
        assert!(
            text.contains("Proof strategy:"),
            "{file} must describe a local proof strategy"
        );
        for (idx, line) in text.lines().enumerate() {
            let trimmed = line.trim_start();
            if trimmed.starts_with("//! - [ ] ") {
                assert!(
                    trimmed.contains("- [ ] Safety:") || trimmed.contains("- [ ] Liveness:"),
                    "{file}:{} checklist item must be labeled Safety or Liveness",
                    idx + 1
                );
            }
        }
    }

    let engine = normalize_whitespace(&source_text(&root.join("src/core/engine_unproven.rs")));
    for required in [
        "Owned invariant: validated-context provenance and ongoing engine safety",
        "Safety: every in-memory fact is paired with the id derived from its",
        "bytes before the engine hands it to a projector",
        "Safety: a projector receives only validated offers",
        "Safety: every validated offer is owned by a fact already projected valid",
        "Safety: raw bytes returned in `ProjectOutcome.emitted` do not inherit",
        "Imported theorems:",
        "`core::item`: fact ids identify canonical bytes",
        "Proof strategy:",
    ] {
        assert!(
            engine.contains(required),
            "core engine checklist is missing {required:?}"
        );
    }

    let admit = normalize_whitespace(&source_text(&root.join("src/core/admit_unproven.rs")));
    for required in [
        "Owned invariant: new/local fact admission creates only asserted state",
        "Safety: admission creates an `Admitted` token and asserted storage state",
        "creates no validity, validated offer, or validated context",
        "Safety: the admitted token's id/body relation is derived from",
        "`core::item` content addressing",
        "extraction exactness is proved by the fact-family projector",
        "Imported theorems:",
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
        "Safety: project-owned construction",
        "Safety: well-formed parent naming",
        "extraction asserts exactly one need for `valid_link(parent_id, root_id)`",
        "Safety: malformed `prev`/`root` combinations assert no edges",
        "Safety: starter validity rule",
        "valid exactly when validated context contains",
        "`valid_link(parent_id, root_id)` for the child's declared parent and",
        "root/domain ids",
        "Safety: same-root/domain preservation",
        "the child's promoted self-offer carries that same root/domain",
        "Safety: statement-to-owner",
        "was promoted from a valid link fact",
        "Safety: composition with core",
        "using `core::engine` validated-context",
        "provenance, every valid child link has a valid same-root parent chain",
        "valid same-root parent chain to",
        "no theorem here claims anchor uniqueness",
        "Imported theorems:",
        "Proof strategy:",
        "Prove the statement-to-owner lemma",
        "Prove same-root parent-chain transitivity by induction",
    ] {
        assert!(
            link.contains(required),
            "link project checklist is missing {required:?}"
        );
    }

    let api = normalize_whitespace(&source_text(&root.join("src/facts/link/api_unproven.rs")));
    for required in [
        "Owned invariant: link reporting boundary",
        "Safety: chain walking follows only decoded `prev` links from persisted",
        "bytes",
        "a missing fact stops the walk as incomplete",
        "a malformed fact",
        "returns an error before any complete report can be produced",
        "Prove `walk` is read-only and updates its next id only from a successfully",
        "decoded link's `prev` field",
        "malformed bytes",
        "return an error before `chain_report` can return `complete=true`",
    ] {
        assert!(
            api.contains(required),
            "link API checklist is missing {required:?}"
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
    assert!(api.contains("replay::<LinkProjector>"));
    for forbidden in [
        "link_from_params",
        "admit::<",
        "insert_asserted",
        "flush_fact",
        "project_one",
        "LinkState",
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
