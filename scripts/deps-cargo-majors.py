#!/usr/bin/env python3
"""Report Rust dependencies whose newest aged release is outside the manifest requirement.

`cargo generate-lockfile --publish-time` only moves within the declared semver
ranges. This lists the direct dependencies that need a manual manifest bump,
using the newest stable, non-yanked crates.io release published before the
cutoff (the same release-age window the other ecosystems use).
"""

import argparse
import json
import re
import sys
import tomllib
import urllib.request
from datetime import datetime
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

MANIFESTS = [
    ROOT / "Cargo.toml",
    *sorted((ROOT / "crates").glob("*/Cargo.toml")),
    ROOT / "examples/workflows/polyglot/rust_tasks/Cargo.toml",
]

DEP_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")

USER_AGENT = "czar-deps (https://github.com/czarhq/czar)"

VERSION_RE = re.compile(r"^(\d+)(?:\.(\d+))?(?:\.(\d+))?")


def parse_version(text: str) -> tuple[int, int, int]:
    match = VERSION_RE.match(text.lstrip("=^~ "))
    if not match:
        raise ValueError(f"unsupported version requirement: {text}")
    major, minor, patch = (int(part) if part else 0 for part in match.groups())
    return major, minor, patch


def compatible(requirement: str, version: str) -> bool:
    """Return True if `version` satisfies a plain caret/exact requirement."""
    req = requirement.strip()
    have = parse_version(version)
    want = parse_version(req)
    if req.startswith("="):
        return have == want
    if have < want:
        return False
    if want[0] > 0:
        return have[0] == want[0]
    if want[1] > 0:
        return have[:2] == want[:2]
    return have == want


def registry_deps(table: dict[str, object]) -> dict[str, str]:
    deps: dict[str, str] = {}
    for key, spec in table.items():
        if isinstance(spec, str):
            deps[key] = spec
            continue
        if not isinstance(spec, dict):
            continue
        if "path" in spec or "git" in spec or spec.get("workspace"):
            continue
        version = spec.get("version")
        if isinstance(version, str):
            package = spec.get("package")
            deps[package if isinstance(package, str) else key] = version
    return deps


def collect_requirements() -> dict[str, dict[str, set[str]]]:
    """Map crate name -> requirement -> manifests declaring it."""
    found: dict[str, dict[str, set[str]]] = {}
    for manifest in MANIFESTS:
        data = tomllib.loads(manifest.read_text(encoding="utf-8"))
        tables: list[dict[str, object]] = []
        workspace = data.get("workspace", {})
        if isinstance(workspace, dict) and isinstance(workspace.get("dependencies"), dict):
            tables.append(workspace["dependencies"])
        for name in DEP_TABLES:
            if isinstance(data.get(name), dict):
                tables.append(data[name])
        for target in data.get("target", {}).values():
            for name in DEP_TABLES:
                if isinstance(target.get(name), dict):
                    tables.append(target[name])
        rel = str(manifest.relative_to(ROOT))
        for table in tables:
            for crate, requirement in registry_deps(table).items():
                found.setdefault(crate, {}).setdefault(requirement, set()).add(rel)
    return found


def newest_before(crate: str, cutoff: datetime) -> tuple[str, str] | None:
    url = f"https://crates.io/api/v1/crates/{crate}/versions?per_page=100&sort=semver"
    request = urllib.request.Request(url, headers={"User-Agent": USER_AGENT})
    with urllib.request.urlopen(request, timeout=30) as response:
        versions = json.load(response)["versions"]
    for entry in versions:
        if entry["yanked"] or "-" in entry["num"]:
            continue
        published = datetime.fromisoformat(entry["created_at"].replace("Z", "+00:00"))
        if published <= cutoff:
            return entry["num"], published.date().isoformat()
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description="Report Rust dependencies needing a manifest bump.")
    parser.add_argument("--cutoff", required=True, help="RFC 3339 timestamp, e.g. 2026-09-21T00:00:00Z")
    args = parser.parse_args()
    cutoff = datetime.fromisoformat(args.cutoff.replace("Z", "+00:00"))

    rows: list[tuple[str, str, str, str, str]] = []
    for crate, requirements in sorted(collect_requirements().items()):
        newest = newest_before(crate, cutoff)
        if newest is None:
            print(f"warning: no release of {crate} older than {args.cutoff}", file=sys.stderr)
            continue
        version, published = newest
        for requirement, manifests in sorted(requirements.items()):
            if not compatible(requirement, version):
                rows.append((crate, requirement, version, published, ", ".join(sorted(manifests))))

    if not rows:
        print(f"All Rust requirements admit the newest release published before {args.cutoff}.")
        return 0

    print(f"Rust dependencies needing a manifest bump (newest release published before {args.cutoff}):")
    width = max(len(row[0]) for row in rows)
    for crate, requirement, version, published, manifests in rows:
        print(f"  {crate:<{width}}  {requirement:>12} -> {version:<10} ({published})  {manifests}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
