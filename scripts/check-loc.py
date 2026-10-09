#!/usr/bin/env python3
"""
Check that no source files exceed 500 lines of code.

Exit codes:
  0: All files are within the limit
  1: One or more files exceed the limit
"""
import argparse
import os
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_LOC = 500

EXCLUDE_DIRS = {
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".venv",
    "dist",
    "build",
    ".wrangler",
    "wrangler-harness",
    "third_party",
}
EXCLUDE_FILES = {
    ".DS_Store",
    "pnpm-lock.yaml",
}
EXCLUDE_SUFFIXES = {
    ".generated.yaml",
}

# File extensions to check
SOURCE_EXTENSIONS = {".rs", ".py", ".js", ".ts", ".jsx", ".tsx", ".yaml", ".yml"}

def main() -> int:
    args = parse_args()
    violations: list[tuple[Path, int]] = []
    all_files: list[tuple[Path, int]] = []

    source_files = iter_source_files(ROOT)

    for file_path in sorted(source_files):
        is_ok, line_count = check_file_loc(file_path, args.max_loc)
        rel_path = file_path.relative_to(ROOT)

        all_files.append((rel_path, line_count))

        if not is_ok:
            violations.append((rel_path, line_count))

    if args.verbose:
        print(f"\n{'File':<60} {'Lines':>6}")
        print("-" * 67)
        for path, lines in all_files:
            marker = "❌" if lines > args.max_loc else "✓"
            print(f"{marker} {str(path):<58} {lines:>6}")
        print(f"\nTotal files checked: {len(all_files)}")

    if violations:
        print(f"\n❌ Found {len(violations)} file(s) exceeding {args.max_loc} lines:\n")
        print(f"{'File':<60} {'Lines':>6} {'Over':>6}")
        print("-" * 73)
        for path, lines in violations:
            over = lines - args.max_loc
            print(f"{str(path):<60} {lines:>6} (+{over})")
        print(f"\nPlease refactor these files to be under {args.max_loc} lines.")
        return 1

    print(f"✓ All {len(all_files)} source files are within {args.max_loc} line limit.")
    return 0


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=f"Check that no source files exceed {MAX_LOC} lines of code."
    )
    parser.add_argument(
        "--max-loc",
        type=int,
        default=MAX_LOC,
        help=f"Maximum lines of code per file (default: {MAX_LOC})",
    )
    parser.add_argument(
        "--verbose",
        "-v",
        action="store_true",
        help="Show all files checked, not just violations",
    )
    return parser.parse_args()


def iter_source_files(root_dir: Path) -> list[Path]:
    """Find all source files in the repository."""
    files: list[Path] = []
    for root, dirs, filenames in os.walk(root_dir):
        dirs[:] = [d for d in dirs if d not in EXCLUDE_DIRS]
        for name in filenames:
            if name in EXCLUDE_FILES:
                continue
            if any(name.endswith(s) for s in EXCLUDE_SUFFIXES):
                continue
            path = Path(root) / name
            if path.suffix in SOURCE_EXTENSIONS:
                files.append(path)
    return files


def check_file_loc(file_path: Path, max_loc: int = MAX_LOC) -> tuple[bool, int]:
    """
    Check if a file exceeds the max LOC limit.

    Returns:
        (is_ok, line_count) where is_ok is True if file is within limit
    """
    line_count = count_lines(file_path)
    return line_count <= max_loc, line_count


def count_lines(file_path: Path) -> int:
    """Count LOC, skipping empty lines and comments."""
    try:
        count = 0
        in_block_comment = False
        suffix = file_path.suffix
        with open(file_path, encoding="utf-8", errors="ignore") as f:
            for raw_line in f:
                line = raw_line.strip()
                if not line:
                    continue

                if suffix in {".py", ".yaml", ".yml"}:
                    if line.startswith("#"):
                        continue
                    count += 1
                    continue

                if suffix in {".rs", ".js", ".ts", ".jsx", ".tsx"}:
                    if in_block_comment:
                        if "*/" in line:
                            in_block_comment = False
                        continue
                    if line.startswith("/*"):
                        if "*/" not in line:
                            in_block_comment = True
                        continue
                    if line.startswith("//"):
                        continue
                    count += 1
                    continue

                count += 1
        return count
    except (OSError, UnicodeDecodeError):
        return 0


if __name__ == "__main__":
    sys.exit(main())
