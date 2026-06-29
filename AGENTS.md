# AGENTS Instructions

## Proof Strategy Extraction

When reviewing proof strategy paragraphs, use the extractor script instead of
manually collecting scattered source comments:

```bash
./scripts/collect_proof_strategies.py
```

The script writes a minimal Markdown document to stdout containing only source
locations and `Proof strategy:` blocks. Redirect it to a temporary path outside
the worktree unless the task explicitly asks for a checked-in document:

```bash
./scripts/collect_proof_strategies.py > /tmp/proof-strategies.md
```

If a persistent document is requested, pass `--output` with the requested path.

For any worktree task that creates or updates instructions, the final step is:
commit the completed work on that same worktree branch before handoff or review.
