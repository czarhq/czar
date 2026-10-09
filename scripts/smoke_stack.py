#!/usr/bin/env python3
from __future__ import annotations

import argparse
import os

from smoke_support import ROOT, SmokeStackConfig, remove_sqlite_artifacts, run_checked, smoke_env


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Start the local smoke-test API stack.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    api_server = subparsers.add_parser("api-server", help="reset the SQLite smoke DB and run czar serve")
    api_server.add_argument("--addr", default="127.0.0.1:4010")
    api_server.add_argument("--db-dir", default=".czar/smoke/api")
    api_server.add_argument("--workflows-dir", default="examples/workflows/simple_args")

    return parser.parse_args()


def run_api_server(addr: str, db_dir: str, workflows_dir: str) -> int:
    config = SmokeStackConfig(
        addr=addr,
        db_dir=(ROOT / db_dir).resolve(),
        workflows_dir=(ROOT / workflows_dir).resolve(),
    )
    config.db_dir.mkdir(parents=True, exist_ok=True)
    remove_sqlite_artifacts(config.database_path)
    env = smoke_env(config)

    init_argv = [
        "cargo",
        "run",
        "-q",
        "-p",
        "czar-cli",
        "--no-default-features",
        "--features",
        "sqlite,local-executors",
        "--bin",
        "czar",
        "--",
        "init",
    ]
    run_checked(init_argv, env=env)

    serve_argv = [
        "cargo",
        "run",
        "-q",
        "-p",
        "czar-cli",
        "--no-default-features",
        "--features",
        "sqlite,local-executors",
        "--bin",
        "czar",
        "--",
        "serve",
        "--addr",
        addr,
        "--workflows-dir",
        str(config.workflows_dir),
    ]
    os.execvpe("cargo", serve_argv, env)


def main() -> int:
    args = parse_args()
    if args.command == "api-server":
        return run_api_server(args.addr, args.db_dir, args.workflows_dir)
    raise ValueError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
