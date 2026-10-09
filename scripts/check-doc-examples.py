#!/usr/bin/env python3
from __future__ import annotations

import hashlib
import re
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TARGETS = [
    Path("README.md"),
    *sorted(Path("docs/guide").glob("*.md")),
    *sorted(Path("docs/design").glob("*.md")),
    *sorted(Path("docs/deployment").rglob("*.md")),
]

SNIPPET_RE = re.compile(
    r"^<!-- doc-snippet: path=(?P<path>\S+) sha256=(?P<sha>[0-9a-f]{64}) lines=(?P<start>\d+)-(?P<end>\d+) -->$"
)
MARKDOWN_LINK_RE = re.compile(r"\[[^]]+\]\(([^)]+)\)")
STATUS_RE = re.compile(r"^Status:\s+.+$")


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def resolve_doc_link(doc_path: Path, raw_link: str) -> Path:
    return (doc_path.parent / raw_link).resolve()


def validate_examples_section(doc_path: Path, lines: list[str], errors: list[str]) -> None:
    try:
        section_index = lines.index("## Examples Covered")
    except ValueError:
        errors.append(f"{doc_path}: missing '## Examples Covered' section")
        return

    numbered: list[str] = []
    for line in lines[section_index + 1 :]:
        if line.startswith("## "):
            break
        if line.strip().startswith(tuple(f"{i}." for i in range(1, 10))):
            numbered.append(line.strip())

    if not numbered:
        errors.append(f"{doc_path}: Examples Covered section has no numbered example links")
        return

    for line in numbered:
        match = MARKDOWN_LINK_RE.search(line)
        if not match:
            errors.append(f"{doc_path}: invalid Examples Covered entry: {line}")
            continue
        target = resolve_doc_link(doc_path, match.group(1))
        if not target.exists():
            errors.append(f"{doc_path}: example link does not exist: {match.group(1)}")
            continue
        if not target.is_dir():
            errors.append(f"{doc_path}: example link must point to a directory: {match.group(1)}")
            continue
        try:
            target.relative_to(REPO_ROOT / "examples")
        except ValueError:
            errors.append(f"{doc_path}: example link must stay under examples/: {match.group(1)}")


def validate_status_markers(doc_path: Path, lines: list[str], errors: list[str]) -> None:
    in_code = False
    for idx, line in enumerate(lines, start=1):
        if line.startswith("```"):
            in_code = not in_code
            continue
        if not in_code and STATUS_RE.match(line):
            errors.append(f"{doc_path}:{idx}: visible Status marker should be hidden in a comment")


def validate_snippets(doc_path: Path, lines: list[str], errors: list[str]) -> None:
    i = 0
    while i < len(lines):
        line: str = lines[i]
        if not line.startswith("```"):
            i += 1
            continue

        lang = line[3:].strip().split()[0] if line[3:].strip() else ""
        is_bash = lang in {"bash", "sh"}
        snippet_match = SNIPPET_RE.match(lines[i - 1]) if i > 0 else None

        code_lines: list[str] = []
        i += 1
        while i < len(lines) and not lines[i].startswith("```"):
            code_lines.append(lines[i])
            i += 1
        if i >= len(lines):
            errors.append(f"{doc_path}: unterminated code fence")
            return

        if not is_bash:
            if snippet_match is None:
                errors.append(f"{doc_path}:{i + 1}: non-bash code fence is missing a doc-snippet comment")
            else:
                source_path = (REPO_ROOT / snippet_match.group("path")).resolve()
                if not source_path.exists():
                    errors.append(
                        f"{doc_path}:{i + 1}: referenced snippet source does not exist: {snippet_match.group('path')}"
                    )
                else:
                    try:
                        source_path.relative_to(REPO_ROOT / "examples")
                    except ValueError:
                        errors.append(
                            f"{doc_path}:{i + 1}: snippet source must stay under examples/: "
                            f"{snippet_match.group('path')}"
                        )
                    if sha256(source_path) != snippet_match.group("sha"):
                        errors.append(
                            f"{doc_path}:{i + 1}: snippet source hash mismatch for {snippet_match.group('path')}"
                        )
                    start = int(snippet_match.group("start"))
                    end = int(snippet_match.group("end"))
                    excerpt = source_path.read_text().splitlines()[start - 1 : end]
                    if excerpt != code_lines:
                        errors.append(
                            f"{doc_path}:{i + 1}: snippet differs from {snippet_match.group('path')}:{start}-{end}"
                        )
        i += 1


def main() -> int:
    errors: list[str] = []
    for doc_path in TARGETS:
        lines = doc_path.read_text(encoding="utf-8").splitlines()
        validate_examples_section(doc_path, lines, errors)
        validate_status_markers(doc_path, lines, errors)
        validate_snippets(doc_path, lines, errors)

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
