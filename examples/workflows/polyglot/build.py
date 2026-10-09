#!/usr/bin/env python3
"""Build native and Wasm Rust artifacts for the polyglot example."""

from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--profile",
        choices=("debug", "release"),
        default="debug",
        help="Cargo profile to build",
    )
    parser.add_argument(
        "--check",
        action="store_true",
        help="Only verify that the expected artifacts exist",
    )
    return parser.parse_args()


def host_library_extension() -> str:
    if sys.platform == "darwin":
        return "dylib"
    if sys.platform.startswith("win"):
        return "dll"
    return "so"


def host_binary_suffix() -> str:
    if sys.platform.startswith("win"):
        return ".exe"
    return ""


def expected_artifacts(profile: str) -> list[Path]:
    example_dir = Path(__file__).resolve().parent
    return [
        example_dir
        / "rust_tasks"
        / "target"
        / profile
        / f"libpolyglot_rust_lib.{host_library_extension()}",
        example_dir
        / "rust_tasks"
        / "target"
        / profile
        / f"process_raw{host_binary_suffix()}",
        example_dir
        / "rust_tasks"
        / "target"
        / profile
        / f"process_sdk{host_binary_suffix()}",
        example_dir
        / "rust_tasks"
        / "target"
        / profile
        / f"process_macro{host_binary_suffix()}",
        example_dir
        / "rust_tasks"
        / "target"
        / "wasm32-unknown-unknown"
        / profile
        / "polyglot_rust_lib.wasm",
        example_dir
        / "rust_tasks"
        / "target"
        / "wasm32-wasip1"
        / profile
        / "wasm_cli.wasm",
    ]


def check_artifacts(profile: str) -> None:
    missing = [path for path in expected_artifacts(profile) if not path.exists()]
    if missing:
        missing_list = "\n".join(f"  - {path}" for path in missing)
        raise SystemExit(
            "Missing prebuilt polyglot artifacts for profile "
            f"{profile}.\nRun: just example-build polyglot {profile}\n{missing_list}"
        )


def ensure_rust_targets() -> None:
    installed = subprocess.run(
        ["rustup", "target", "list", "--installed"],
        check=True,
        capture_output=True,
        text=True,
    ).stdout.splitlines()
    for target in ("wasm32-unknown-unknown", "wasm32-wasip1"):
        if target not in installed:
            raise SystemExit(
                f"Missing Rust target {target}. Install it with: rustup target add {target}"
            )


def build(profile: str) -> None:
    example_dir = Path(__file__).resolve().parent
    manifest_path = example_dir / "rust_tasks" / "Cargo.toml"
    cargo_args: list[str] = ["--release"] if profile == "release" else []

    builds = [
        [
            "cargo",
            "build",
            "--manifest-path",
            str(manifest_path),
            "--lib",
            "--bin",
            "process_raw",
            "--bin",
            "process_sdk",
            "--bin",
            "process_macro",
            *cargo_args,
        ],
        [
            "cargo",
            "build",
            "--manifest-path",
            str(manifest_path),
            "--target",
            "wasm32-unknown-unknown",
            "--lib",
            *cargo_args,
        ],
        [
            "cargo",
            "build",
            "--manifest-path",
            str(manifest_path),
            "--target",
            "wasm32-wasip1",
            "--bin",
            "wasm_cli",
            *cargo_args,
        ],
    ]

    for cmd in builds:
        subprocess.run(cmd, check=True, cwd=example_dir)


def main() -> None:
    args = parse_args()
    if args.check:
        check_artifacts(args.profile)
        return
    ensure_rust_targets()
    build(args.profile)
    check_artifacts(args.profile)


if __name__ == "__main__":
    main()
