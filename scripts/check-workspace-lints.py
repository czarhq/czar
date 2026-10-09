#!/usr/bin/env python3
from __future__ import annotations

import sys
import tomllib
from pathlib import Path
from typing import Any, TypedDict, cast

REPO_ROOT = Path(__file__).resolve().parent.parent
ROOT_CARGO = REPO_ROOT / "Cargo.toml"
CRATES_DIR = REPO_ROOT / "crates"


class WorkspaceConfig(TypedDict, total=False):
    members: list[str]


class RootManifest(TypedDict, total=False):
    workspace: WorkspaceConfig


class LintsConfig(TypedDict, total=False):
    workspace: bool


class CrateManifest(TypedDict, total=False):
    lints: LintsConfig


def load_toml(path: Path) -> dict[str, Any]:
    return tomllib.loads(path.read_text(encoding="utf-8"))


def expand_workspace_members(root_manifest: RootManifest) -> set[Path]:
    workspace = root_manifest.get("workspace", {})
    members = workspace.get("members", [])
    expanded: set[Path] = set()
    for entry in members:
        if any(ch in entry for ch in "*?[]"):
            for match in REPO_ROOT.glob(entry):
                expanded.add(match.resolve())
        else:
            expanded.add((REPO_ROOT / entry).resolve())
    return expanded


def main() -> int:
    root_manifest = cast(RootManifest, load_toml(ROOT_CARGO))
    workspace_members = expand_workspace_members(root_manifest)

    crate_dirs = {
        path.parent.resolve() for path in CRATES_DIR.glob("*/Cargo.toml") if path.is_file()
    }

    errors: list[str] = []

    missing_from_workspace = sorted(crate_dirs - workspace_members)
    for crate_dir in missing_from_workspace:
        errors.append(
            f"{crate_dir.relative_to(REPO_ROOT)}/Cargo.toml: crate under crates/ must be listed in [workspace].members"
        )

    for crate_dir in sorted(crate_dirs & workspace_members):
        manifest_path = crate_dir / "Cargo.toml"
        manifest = cast(CrateManifest, load_toml(manifest_path))
        lints = manifest.get("lints")
        if not isinstance(lints, dict) or lints.get("workspace") is not True:
            errors.append(
                f"{manifest_path.relative_to(REPO_ROOT)}: missing `[lints] workspace = true`"
            )

    if errors:
        for error in errors:
            print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
