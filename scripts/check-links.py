#!/usr/bin/env python3
"""Check markdown files for broken relative links."""

import argparse
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

LINK_RE = re.compile(r"\[(?:[^\]]*)\]\((?P<link>[^)]+)\)")

SEARCH_DIRS = [
    ROOT / "docs",
    ROOT / "README.md",
    ROOT / "SECURITY.md",
    ROOT / "crates",
    ROOT / "examples",
    ROOT / "python-sdk" / "README.md",
    ROOT / "python-cli" / "README.md",
]

SKIP_DIR_NAMES = {
    ".git",
    ".venv",
    "node_modules",
    "__pycache__",
    "dist",
    ".wrangler",
    "target",
}

SKIP_PATTERNS = [
    "http://",
    "https://",
    "mailto:",
]


def is_external(link: str) -> bool:
    return any(link.startswith(p) for p in SKIP_PATTERNS)


def strip_fragment(link: str) -> str:
    return link.split("#")[0]


def check_file(md_file: Path) -> list[str]:
    errors: list[str] = []
    text = md_file.read_text(encoding="utf-8")
    for lineno, line in enumerate(text.splitlines(), start=1):
        for match in LINK_RE.finditer(line):
            link = match.group("link")
            if is_external(link) or link.startswith("#"):
                continue
            target_path = strip_fragment(link)
            if not target_path:
                continue
            # Absolute paths (starting with /) are relative to repo root
            if target_path.startswith("/"):
                resolved = (ROOT / target_path.lstrip("/")).resolve()
            else:
                resolved = (md_file.parent / target_path).resolve()
            if not resolved.exists():
                rel_file = md_file.relative_to(ROOT)
                errors.append(f"{rel_file}:{lineno}: broken link -> {link}")
    return errors


def collect_md_files() -> list[Path]:
    files: list[Path] = []
    for entry in SEARCH_DIRS:
        if entry.is_file():
            files.append(entry)
        elif entry.is_dir():
            for path in entry.rglob("*.md"):
                if any(part in SKIP_DIR_NAMES for part in path.parts):
                    continue
                files.append(path)
    return sorted(files)


def main() -> int:
    parser = argparse.ArgumentParser(description="Check markdown files for broken relative links.")
    parser.add_argument(
        "paths",
        nargs="*",
        help="Specific files or directories to check (default: docs/, crates/, examples/ and top-level READMEs)",
    )
    args = parser.parse_args()

    if args.paths:
        files: list[Path] = []
        for p in args.paths:
            path = Path(p)
            if path.is_file():
                files.append(path.resolve())
            elif path.is_dir():
                for md_path in path.resolve().rglob("*.md"):
                    if any(part in SKIP_DIR_NAMES for part in md_path.parts):
                        continue
                    files.append(md_path)
    else:
        files = collect_md_files()

    all_errors: list[str] = []
    for md_file in files:
        all_errors.extend(check_file(md_file))

    if all_errors:
        for err in all_errors:
            print(err)
        return 1

    print(f"No broken links found ({len(files)} files checked).")
    return 0


if __name__ == "__main__":
    sys.exit(main())
