#!/usr/bin/env python3
"""One-time snapshot copy of the engine's public README (frozen evidence).

The page's architecture narrative quotes the engine's own public description;
this freezes the exact bytes the claims were read from. Refuses overwrite.

Usage:
    python3 tools/freeze_docs.py --engine /path/to/diagram-render-rs [--tree .]
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from common import GateError, guard_engine, resolve_tree, sha256_bytes  # noqa: E402


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tree", default=None)
    parser.add_argument("--engine", default=None)
    args = parser.parse_args()

    tree = resolve_tree(args.tree)
    engine = guard_engine(args.engine)
    target = tree / "data" / "frozen" / "engine-readme.txt"
    if target.exists():
        raise GateError("engine-readme.txt already exists; frozen is never rewritten")

    readme = (engine / "README.md").read_bytes()
    header = (
        "# Byte snapshot of the engine's public README.md at the frozen HEAD.\n"
        f"# sha256: {sha256_bytes(readme)}\n"
        "# (copied once as the anchor for the page's architecture narrative)\n\n"
    )
    target.write_bytes(header.encode() + readme)
    print(f"wrote {target.relative_to(tree)}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except GateError as error:
        print(f"FREEZE-DOCS FAILED: {error}", file=sys.stderr)
        raise SystemExit(1)
