#!/usr/bin/env python3
"""Check that what a release publishes is what we mean to publish.

Run from the repository root (`just release-check`). It reads, and changes
nothing:

1. Every publishable crate has the metadata crates.io shows (description,
   license, repository, homepage, readme) and the workspace version.
2. scripts/publish-prep.sh lists every publishable crate, so none is left
   out of a publish.
3. The files each crate would upload (`cargo package --list`) and the files
   in the repository hold nothing private: no internal notes, no AI-tool
   settings, no secrets by file name, no personal data.
4. The Python packages build and pass `twine check`.

Exit status 1 when anything fails; every problem is listed.
"""

from __future__ import annotations

import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

# Paths that must never be in the public repository or a package.
FORBIDDEN_PATHS = [
    r"(^|/)TODO\.md$",
    r"^docs/reports/(?!perf/results\.md$)",
    r"^docs/dev/file-review\.md$",
    r"^docs/visual/",
    r"^\.claude/",
    r"(^|/)\.env(\.|$)",
    r"\.(pem|key|p12|pfx)$",
    r"(^|/)id_(rsa|ed25519)",
    r"\.tfstate(\.backup)?$",
    r"(^|/)terraform\.tfvars$",
]
# Text that must not appear in any tracked file.
FORBIDDEN_TEXT = [
    (r"/(home|Users)/(?!(czar|runner|me|nonroot|path)\b)[a-zA-Z0-9_.-]+", "a path from a personal or developer machine"),
    (r"\b[A-Za-z0-9._%+-]+@(gmail|googlemail|yahoo|hotmail|outlook|proton|protonmail|icloud|me)\.[a-z]{2,}\b", "a personal email address"),
    (r"github\.com/(?!czarhq/)[a-zA-Z0-9_.-]+/(czar|ork)\b", "an old or personal repository URL"),
    (r"\b[oO][rR][kK][uU][sS]\b", "the old name"),
    (r"AKIA[0-9A-Z]{16}", "an AWS access key"),
    (r"ghp_[A-Za-z0-9]{30,}", "a GitHub token"),
    (r"pypi-AgEI[A-Za-z0-9_-]{20,}", "a PyPI token"),
    (r"-----BEGIN [A-Z ]*PRIVATE KEY-----(\\n|\n)[A-Za-z0-9+/]{40}", "a private key"),
]
REQUIRED_METADATA = ["description", "license", "repository", "homepage"]


def get_forbidden_text() -> list[tuple[str, str]]:
    patterns = list(FORBIDDEN_TEXT)
    user_home = Path.home().as_posix()
    if Path.home().name not in ("czar", "runner", "me", "nonroot"):
        patterns.append((re.escape(user_home), "a path from the local machine"))
    try:
        git_email = subprocess.run(["git", "config", "user.email"], cwd=ROOT, capture_output=True, text=True).stdout.strip()
        if git_email and not git_email.endswith(("@users.noreply.github.com", "@example.com")):
            patterns.append((re.escape(git_email), "the author's personal email address"))
    except Exception:
        pass
    return patterns


def main() -> int:
    problems: list[str] = []
    meta = json.loads(run(["cargo", "metadata", "--format-version", "1", "--no-deps", "--offline"]))
    workspace_version = None
    publishable = []
    for pkg in meta["packages"]:
        if pkg.get("publish") == []:
            continue
        publishable.append(pkg["name"])
        for field in REQUIRED_METADATA:
            if not pkg.get(field):
                problems.append(f"crate {pkg['name']}: no `{field}`")
        if not pkg.get("readme"):
            problems.append(f"crate {pkg['name']}: no readme (crates.io shows an empty page)")
        workspace_version = workspace_version or pkg["version"]
        if pkg["version"] != workspace_version:
            problems.append(f"crate {pkg['name']}: version {pkg['version']}, the others {workspace_version}")

    listed = set(re.findall(r"^\s+(czar[\w-]*)\s*$", (ROOT / "scripts/publish-prep.sh").read_text(), re.M))
    for name in sorted(set(publishable) - listed):
        problems.append(f"crate {name}: missing from scripts/publish-prep.sh, so it wouldn't be published")
    for name in sorted(listed - set(publishable)):
        problems.append(f"scripts/publish-prep.sh lists {name}, which isn't a publishable crate")

    print(f"{len(publishable)} publishable crates, version {workspace_version}")

    forbidden = get_forbidden_text()

    for name in sorted(publishable):
        files = run(["cargo", "package", "--list", "-p", name, "--allow-dirty", "--offline"]).split()
        crate_dir = next(Path(p["manifest_path"]).parent for p in meta["packages"] if p["name"] == name)
        for f in files:
            for bad in FORBIDDEN_PATHS:
                if re.search(bad, f):
                    problems.append(f"crate {name} would upload {f}")
            path = crate_dir / f
            if path.is_file():
                problems += text_problems(path, f"crate {name}: {f}", forbidden)

    tracked = run(["git", "ls-files"]).splitlines()
    for f in tracked:
        for bad in FORBIDDEN_PATHS:
            if re.search(bad, f):
                problems.append(f"repository holds {f}")
        path = ROOT / f
        if path.is_file() and not path.is_symlink():
            problems += text_problems(path, f, forbidden)
    print(f"{len(tracked)} tracked files scanned")

    with tempfile.TemporaryDirectory() as out:
        # The czar-cli wheel holds the native binary; the release workflow
        # builds it per platform, so only the SDK is built here.
        for project in ["python-sdk"]:
            build = ["uv", "build", project, "--out-dir", out]
            result = subprocess.run(build, cwd=ROOT, capture_output=True, text=True)
            if result.returncode:
                problems.append(f"{project}: build failed: {result.stderr.strip().splitlines()[-1:]}")
        dists = sorted(str(p) for p in Path(out).iterdir() if p.name.endswith((".whl", ".tar.gz")))
        if dists:
            check = subprocess.run(["uvx", "twine", "check", "--strict", *dists], cwd=ROOT, capture_output=True, text=True)
            if check.returncode:
                problems.append("twine check: " + " ".join(check.stdout.split()[-30:]))
        print(f"{len(dists)} Python distributions built and checked")

    if problems:
        print(f"\n{len(problems)} problem(s):")
        for p in problems:
            print(f"  - {p}")
        return 1
    print("\nOK: nothing private, nothing missing.")
    return 0


def text_problems(path: Path, label: str, patterns: list[tuple[str, str]]) -> list[str]:
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError):
        return []
    out = []
    for pattern, what in patterns:
        m = re.search(pattern, text)
        if m:
            line = text.count("\n", 0, m.start()) + 1
            out.append(f"{label}:{line}: {what}")
    return out


def run(cmd: list[str]) -> str:
    result = subprocess.run(cmd, cwd=ROOT, capture_output=True, text=True)
    if result.returncode:
        sys.exit(f"{' '.join(cmd)} failed:\n{result.stderr}")
    return result.stdout


if __name__ == "__main__":
    sys.exit(main())
