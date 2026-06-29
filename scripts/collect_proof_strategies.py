#!/usr/bin/env python3
"""Collect source-level Proof strategy blocks into one minimal Markdown doc."""

from __future__ import annotations

import argparse
from dataclasses import dataclass
from pathlib import Path
import re
import sys


DOC_COMMENT = re.compile(r"^\s*//!\s?(.*)$")
STRATEGY_HEADING = "Proof strategy:"


@dataclass(frozen=True)
class StrategyBlock:
    path: Path
    line: int
    body: tuple[str, ...]


def source_files(root: Path) -> list[Path]:
    return sorted((root / "src").glob("**/*.rs"))


def extract_strategy_blocks(root: Path, paths: list[Path]) -> list[StrategyBlock]:
    blocks: list[StrategyBlock] = []
    for path in paths:
        rel_path = path.relative_to(root)
        lines = path.read_text(encoding="utf-8").splitlines()
        idx = 0
        while idx < len(lines):
            match = DOC_COMMENT.match(lines[idx])
            if match is None or match.group(1).strip() != STRATEGY_HEADING:
                idx += 1
                continue

            start_line = idx + 1
            body: list[str] = []
            idx += 1
            while idx < len(lines):
                next_match = DOC_COMMENT.match(lines[idx])
                if next_match is None:
                    break
                body.append(next_match.group(1).rstrip())
                idx += 1

            while body and body[-1] == "":
                body.pop()
            blocks.append(StrategyBlock(rel_path, start_line, tuple(body)))
            idx += 1
    return blocks


def render_markdown(blocks: list[StrategyBlock]) -> str:
    out = [
        "# Proof Strategy Extract",
        "",
        "This document is generated from source-level `Proof strategy:` blocks.",
        "Only the source location and strategy text are included.",
        "",
    ]
    for block in blocks:
        out.append(f"## {block.path}:{block.line}")
        out.append("")
        if block.body:
            out.extend(block.body)
        else:
            out.append("_No strategy body found._")
        out.append("")
    return "\n".join(out).rstrip() + "\n"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Collect source-level Proof strategy blocks into one Markdown document.",
    )
    parser.add_argument(
        "--root",
        type=Path,
        default=Path(__file__).resolve().parents[1],
        help="repository root; defaults to the parent of this script's directory",
    )
    parser.add_argument(
        "--output",
        type=Path,
        help="optional output file; stdout is used when omitted",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = args.root.resolve()
    blocks = extract_strategy_blocks(root, source_files(root))
    if not blocks:
        print("no Proof strategy blocks found", file=sys.stderr)
        return 1

    doc = render_markdown(blocks)
    if args.output:
        args.output.write_text(doc, encoding="utf-8")
    else:
        sys.stdout.write(doc)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
