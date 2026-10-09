#!/usr/bin/env python3
"""Check that every crate has a doc page under docs/dev/crates/ and vice versa."""

import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CRATES_DIR = ROOT / "crates"
DOCS_DIR = ROOT / "docs" / "dev" / "crates"


def main() -> int:
    crates = {d.name for d in CRATES_DIR.iterdir() if (d / "Cargo.toml").is_file() or (d / "package.json").is_file()}
    docs = {p.stem for p in DOCS_DIR.glob("*.md")}

    errors = [f"Missing docs file for crate {name}: {DOCS_DIR / f'{name}.md'}" for name in sorted(crates - docs)]
    errors += [f"Orphan docs file (no crate {name}): {DOCS_DIR / f'{name}.md'}" for name in sorted(docs - crates)]

    for err in errors:
        print(err)
    if errors:
        return 1
    print(f"Crate docs are in sync ({len(crates)} crates).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
