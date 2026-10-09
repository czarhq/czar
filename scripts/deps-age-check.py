#!/usr/bin/env python3
"""Fail if a crate that changed in Cargo.lock is younger than the release-age rule.

`just deps-update` only picks releases at least CZAR_MIN_RELEASE_AGE_DAYS
(default 7) old, but a hand-run `cargo update` or `cargo add` doesn't know the
rule: cargo enforces `global-min-publish-age` itself only from 1.100. This
compares every Cargo.lock in the repository with a base revision and asks
crates.io when each new or changed crate version was published. Versions
listed in deps-age-exceptions.toml (with a reason, until a date) are allowed.

  deps-age-check.py [BASE]   BASE defaults to origin/main (or HEAD if absent)
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tomllib
from fnmatch import fnmatchcase
import urllib.request
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
REGISTRY = "registry+https://github.com/rust-lang/crates.io-index"


def git(*args: str) -> str | None:
    result = subprocess.run(["git", *args], cwd=ROOT, capture_output=True, text=True)
    return result.stdout if result.returncode == 0 else None


def crates(text: str | None) -> set[tuple[str, str]]:
    if not text:
        return set()
    return {(p["name"], p["version"]) for p in tomllib.loads(text).get("package", []) if p.get("source") == REGISTRY}


def exceptions() -> list[dict]:
    """The exceptions that still apply: their `until` date hasn't passed."""
    path = ROOT / "deps-age-exceptions.toml"
    if not path.exists():
        return []
    today = datetime.now(timezone.utc).date()
    return [e for e in tomllib.loads(path.read_text()).get("exception", []) if datetime.fromisoformat(str(e["until"])).date() >= today]


def published(name: str, version: str) -> datetime:
    request = urllib.request.Request(
        f"https://crates.io/api/v1/crates/{name}/{version}",
        headers={"User-Agent": "czar-deps-age-check (https://github.com/czarhq/czar)"},
    )
    with urllib.request.urlopen(request, timeout=30) as response:
        created = json.load(response)["version"]["created_at"]
    return datetime.fromisoformat(created.replace("Z", "+00:00"))


def main() -> int:
    base = sys.argv[1] if len(sys.argv) > 1 else ("origin/main" if git("rev-parse", "--verify", "-q", "origin/main") else "HEAD")
    days = int(os.environ.get("CZAR_MIN_RELEASE_AGE_DAYS", "7"))
    cutoff = datetime.now(timezone.utc) - timedelta(days=days)
    locks = [Path(p) for p in (git("ls-files", "*Cargo.lock") or "").split()]
    allowed = exceptions()
    young = []
    excused = []
    checked = 0
    for lock in locks:
        new = crates((ROOT / lock).read_text()) - crates(git("show", f"{base}:{lock}"))
        for name, version in sorted(new):
            checked += 1
            when = published(name, version)
            if when > cutoff:
                line = f"{lock}: {name} {version} was published {when:%Y-%m-%d %H:%M} UTC, less than {days} days ago"
                reason = next((e["reason"] for e in allowed if any(fnmatchcase(name, c) for c in e["crates"])), None)
                (excused if reason else young).append(f"{line}; allowed: {reason}" if reason else line)
    for line in excused:
        print(line)
    for line in young:
        print(line)
    if young:
        print(f"Use `just deps-update-crate <crate>` to pick an older release, or wait until it is {days} days old.")
        return 1
    note = f" ({len(excused)} allowed by deps-age-exceptions.toml)" if excused else ""
    print(f"Crate ages ok: {checked} new or changed crate versions since {base}, all at least {days} days old or allowed{note}.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
