#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import IO, NotRequired, Required, TextIO, TypedDict, cast

REPO_ROOT = Path(__file__).resolve().parent.parent
MANIFEST_PATH = REPO_ROOT / "examples" / "manifest.json"


class ValidationConfig(TypedDict):
    kind: str
    files: list[str]


class RunConfig(TypedDict):
    enabled: Required[bool]
    reason: NotRequired[str]


class ManifestEntry(TypedDict):
    name: str
    dir: str
    kind: str
    validation: ValidationConfig
    run: RunConfig


class ManifestFile(TypedDict):
    examples: list[ManifestEntry]


class ExampleResult(TypedDict):
    example: str
    status: str
    exit_code: int
    duration_s: int
    kind: str
    policy: str


STATUS_LABEL_WIDTH = len("TIMEOUT")
ANSI_RESET = "\033[0m"
ANSI_STATUS_COLORS = {
    "PASS": "\033[32m",
    "SKIP": "\033[36m",
    "TIMEOUT": "\033[33m",
    "FAIL": "\033[31m",
}


def load_manifest() -> list[ManifestEntry]:
    data = cast(ManifestFile, json.loads(MANIFEST_PATH.read_text(encoding="utf-8")))
    return data["examples"]


def get_entry(name: str) -> ManifestEntry:
    for entry in load_manifest():
        if entry["name"] == name:
            return entry
    raise SystemExit(f"Unknown example '{name}'. See `just examples-list-all`.")


def runnable_entries() -> list[ManifestEntry]:
    return [entry for entry in load_manifest() if entry["run"]["enabled"]]


def write_results(rows: list[ExampleResult], results_path: Path) -> None:
    results_path.parent.mkdir(parents=True, exist_ok=True)
    results_path.write_text(
        json.dumps(rows, separators=(",", ":"), ensure_ascii=True),
        encoding="utf-8",
    )


def run_command(
    cmd: list[str],
    *,
    stdout: int | IO[str] | None = None,
    stderr: int | IO[str] | None = None,
    timeout: int | None = None,
) -> int:
    try:
        completed: subprocess.CompletedProcess[bytes] = subprocess.run(
            cmd,
            cwd=REPO_ROOT,
            stdout=stdout,
            stderr=stderr,
            check=False,
            timeout=timeout,
        )
        return completed.returncode
    except subprocess.TimeoutExpired:
        return 124


def validate_docs_once(skip_docs_validation: bool) -> bool:
    if skip_docs_validation:
        return True
    code = run_command(["./scripts/check-doc-examples.py"])
    return code == 0


def should_use_color() -> bool:
    if not sys.stdout.isatty():
        return False
    if os.environ.get("NO_COLOR"):
        return False
    return os.environ.get("TERM", "").lower() != "dumb"


def format_status_label(status: str, use_color: bool) -> str:
    label = f"{status:<{STATUS_LABEL_WIDTH}}"
    if not use_color:
        return label
    color = ANSI_STATUS_COLORS.get(status)
    if color is None:
        return label
    return f"{color}{label}{ANSI_RESET}"


def validate_entry(entry: ManifestEntry, docs_ok: bool, log_handle: TextIO) -> tuple[str, int]:
    validation = entry["validation"]
    kind = validation["kind"]
    if kind == "docs":
        return ("PASS" if docs_ok else "FAIL", 0 if docs_ok else 1)
    if kind == "workflow":
        for relative in validation["files"]:
            code = run_command(
                [
                    "cargo",
                    "run",
                    "-q",
                    "-p",
                    "czar-cli",
                    "--bin",
                    "czar",
                    "--",
                    "validate-workflow",
                    str(REPO_ROOT / entry["dir"] / relative),
                ],
                stdout=log_handle,
                stderr=subprocess.STDOUT,
            )
            if code != 0:
                return ("FAIL", code)
        return ("PASS", 0)
    if kind == "yaml":
        files = [str(REPO_ROOT / entry["dir"] / relative) for relative in validation["files"]]
        code = run_command(
            ["cargo", "run", "-q", "-p", "czar-cli", "--bin", "yaml-validate", "--", *files],
            stdout=log_handle,
            stderr=subprocess.STDOUT,
        )
        return ("PASS" if code == 0 else "FAIL", code)
    raise SystemExit(f"Unsupported validation kind: {kind}")


def check_examples(
    timeout_s: int,
    results_path: Path,
    skip_docs_validation: bool,
    *,
    run_runnable: bool,
) -> int:
    entries = load_manifest()
    example_name_width = max(len(entry["name"]) for entry in entries)
    use_color = should_use_color()
    skip_names = {
        name.strip()
        for name in os.environ.get("CZAR_SKIP_EXAMPLES", "").split(",")
        if name.strip()
    }
    docs_ok = validate_docs_once(skip_docs_validation)

    failed = False
    rows: list[ExampleResult] = []
    for entry in entries:
        log_path = REPO_ROOT / ".czar" / "reports" / f"example-{entry['name']}.log"
        log_path.parent.mkdir(parents=True, exist_ok=True)
        start = time.time()
        with log_path.open("w", encoding="utf-8") as log_handle:
            status, exit_code = validate_entry(entry, docs_ok, log_handle)
            if status == "PASS" and entry["run"]["enabled"] and run_runnable:
                if entry["name"] in skip_names:
                    status = "SKIP"
                    exit_code = 0
                else:
                    build_code = run_command(
                        ["just", "example-build", entry["name"]],
                        stdout=log_handle,
                        stderr=subprocess.STDOUT,
                    )
                    if build_code != 0:
                        status = "FAIL"
                        exit_code = build_code
                    else:
                        run_code = run_command(
                            ["just", "example", entry["name"]],
                            stdout=log_handle,
                            stderr=subprocess.STDOUT,
                            timeout=timeout_s,
                        )
                        if run_code == 0:
                            status = "PASS"
                            exit_code = 0
                        elif run_code == 124:
                            status = "TIMEOUT"
                            exit_code = 124
                        else:
                            status = "FAIL"
                            exit_code = run_code

        duration = int(time.time() - start)
        policy = "run" if entry["run"]["enabled"] else "validate_only"
        rows.append(
            ExampleResult(
                example=entry["name"],
                status=status,
                exit_code=exit_code,
                duration_s=duration,
                kind=entry["kind"],
                policy=policy,
            )
        )

        print(
            f"{format_status_label(status, use_color)}  "
            f"{entry['name']:<{example_name_width}}  "
            f"exit={exit_code}  duration={duration}s"
        )
        if status not in {"PASS", "SKIP"}:
            failed = True
            log_content = log_path.read_text(encoding="utf-8").strip()
            if log_content:
                print(f"  --- {log_path.name} ---", file=sys.stderr)
                for line in log_content.splitlines():
                    print(f"  {line}", file=sys.stderr)
                print(f"  --- end {log_path.name} ---", file=sys.stderr)

    write_results(rows, results_path)
    print(f"Wrote {results_path.relative_to(REPO_ROOT)}")
    return 1 if failed else 0


def validate_only(results_path: Path, skip_docs_validation: bool) -> int:
    return check_examples(
        timeout_s=0,
        results_path=results_path,
        skip_docs_validation=skip_docs_validation,
        run_runnable=False,
    )


def cmd_list(show_all: bool, show_details: bool) -> int:
    entries = load_manifest() if show_all else runnable_entries()
    for entry in entries:
        if show_details:
            policy = "run" if entry["run"]["enabled"] else "validate_only"
            print(f"{entry['name']}\t{entry['kind']}\t{policy}")
        else:
            print(entry["name"])
    return 0


def cmd_assert_runnable(name: str) -> int:
    entry = get_entry(name)
    if entry["run"]["enabled"]:
        return 0
    reason = entry["run"].get("reason", "This example is validate-only.")
    print(f"Example '{name}' is not runnable via `just example`: {reason}", file=sys.stderr)
    return 1


def resolve_dir(name: str) -> Path:
    entry = get_entry(name)
    return REPO_ROOT / entry["dir"]


def resolve_workflow_file(name: str) -> Path:
    """Return the absolute path to the workflow YAML this example runs.

    Resolution order:
      1. If `validation.kind == "workflow"` and exactly one file is listed,
         that file is the workflow.
      2. `<dir>/<name>.yaml` if it exists.
      3. `<dir>/workflow.yaml` if it exists.
      4. The only `*.yaml` in `<dir>` excluding `czar.yaml`.
    """
    entry = get_entry(name)
    entry_dir = REPO_ROOT / entry["dir"]
    validation = entry["validation"]
    if validation["kind"] == "workflow" and len(validation["files"]) == 1:
        return entry_dir / validation["files"][0]
    candidate = entry_dir / f"{name}.yaml"
    if candidate.is_file():
        return candidate
    candidate = entry_dir / "workflow.yaml"
    if candidate.is_file():
        return candidate
    yamls = [p for p in entry_dir.glob("*.yaml") if p.name != "czar.yaml"]
    if len(yamls) == 1:
        return yamls[0]
    if len(yamls) == 0:
        raise SystemExit(f"No workflow yaml found for example '{name}' in {entry_dir}")
    raise SystemExit(
        f"Multiple workflow yamls in {entry_dir}; set `validation.files` to the one to run."
    )


def main() -> int:
    parser = argparse.ArgumentParser(description="Example manifest utilities")
    subparsers = parser.add_subparsers(dest="command", required=True)

    list_parser = subparsers.add_parser("list")
    list_parser.add_argument("--all", action="store_true")
    list_parser.add_argument("--details", action="store_true")

    assert_parser = subparsers.add_parser("assert-runnable")
    assert_parser.add_argument("name")

    resolve_dir_parser = subparsers.add_parser("resolve-dir")
    resolve_dir_parser.add_argument("name")

    resolve_workflow_parser = subparsers.add_parser("resolve-workflow")
    resolve_workflow_parser.add_argument("name")

    validate_parser = subparsers.add_parser("validate")
    validate_parser.add_argument(
        "--results",
        default=".czar/reports/examples-validate.json",
    )
    validate_parser.add_argument("--skip-docs-validation", action="store_true")

    check_parser = subparsers.add_parser("check")
    check_parser.add_argument("--timeout", type=int, default=240)
    check_parser.add_argument(
        "--results",
        default=".czar/reports/examples-check.json",
    )
    check_parser.add_argument("--skip-docs-validation", action="store_true")

    args = parser.parse_args()
    if args.command == "list":
        return cmd_list(args.all, args.details)
    if args.command == "assert-runnable":
        return cmd_assert_runnable(args.name)
    if args.command == "resolve-dir":
        print(resolve_dir(args.name))
        return 0
    if args.command == "resolve-workflow":
        print(resolve_workflow_file(args.name))
        return 0
    if args.command == "validate":
        results_path = REPO_ROOT / args.results
        return validate_only(results_path, args.skip_docs_validation)
    if args.command == "check":
        results_path = REPO_ROOT / args.results
        return check_examples(
            args.timeout,
            results_path,
            args.skip_docs_validation,
            run_runnable=True,
        )
    raise AssertionError("unreachable")


if __name__ == "__main__":
    raise SystemExit(main())
