#!/usr/bin/env python3
from __future__ import annotations

import argparse
import fcntl
import os
import pty
import struct
import subprocess
import sys
import termios
import time
from pathlib import Path

from smoke_support import (
    DEFAULT_WORKFLOW_FILE,
    DEFAULT_WORKFLOW_NAME,
    ROOT,
    SmokeStackConfig,
    remove_sqlite_artifacts,
    run_checked,
    smoke_env,
    terminate_process,
    wait_for_http_ready,
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run local smoke tests.")
    subparsers = parser.add_subparsers(dest="command", required=True)

    subparsers.add_parser("cli", help="run the CLI smoke test")

    tui_parser = subparsers.add_parser("tui", help="run the TUI smoke test")
    tui_parser.add_argument("--addr", default="127.0.0.1:4011")
    tui_parser.add_argument("--db-dir", default=".czar/smoke/tui")
    tui_parser.add_argument("--dump-path", default=".czar/smoke/tui/render.txt")

    return parser.parse_args()


def cargo_czar_base_argv() -> list[str]:
    return [
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
    ]


def run_cli_smoke() -> int:
    config = SmokeStackConfig(addr="127.0.0.1:0", db_dir=(ROOT / ".czar" / "smoke" / "cli").resolve())
    config.db_dir.mkdir(parents=True, exist_ok=True)
    remove_sqlite_artifacts(config.database_path)
    env = smoke_env(config)

    run_checked([*cargo_czar_base_argv(), "init"], env=env)
    run_checked([*cargo_czar_base_argv(), "run", str(DEFAULT_WORKFLOW_FILE)], env=env)
    print(f"CLI smoke test passed for workflow {DEFAULT_WORKFLOW_NAME}")
    return 0


def wait_for_render_text(path: Path, expected_text: str, *, timeout_s: float) -> str:
    deadline = time.monotonic() + timeout_s
    last_text = ""
    while time.monotonic() < deadline:
        if path.exists():
            last_text = path.read_text(encoding="utf-8")
            if expected_text in last_text:
                return last_text
        time.sleep(0.1)
    raise TimeoutError(f"timed out waiting for render text {expected_text!r} in {path}\n\n{last_text}")


def wait_for_run_success(path: Path, *, timeout_s: float) -> str:
    deadline = time.monotonic() + timeout_s
    last_text = ""
    while time.monotonic() < deadline:
        if path.exists():
            last_text = path.read_text(encoding="utf-8")
            if " Run: " in last_text and "[success]" in last_text:
                return last_text
        time.sleep(0.2)
    raise TimeoutError(f"timed out waiting for successful run render in {path}\n\n{last_text}")


def set_pty_size(fd: int, *, rows: int, cols: int) -> None:
    winsize = struct.pack("HHHH", rows, cols, 0, 0)
    fcntl.ioctl(fd, termios.TIOCSWINSZ, winsize)


def run_tui_smoke(addr: str, db_dir: str, dump_path: str) -> int:
    config = SmokeStackConfig(
        addr=addr,
        db_dir=(ROOT / db_dir).resolve(),
    )
    dump_file = (ROOT / dump_path).resolve()
    dump_file.parent.mkdir(parents=True, exist_ok=True)
    dump_file.unlink(missing_ok=True)

    api_process = subprocess.Popen(
        [
            sys.executable,
            str(ROOT / "scripts" / "smoke_stack.py"),
            "api-server",
            "--addr",
            config.addr,
            "--db-dir",
            str(config.db_dir.relative_to(ROOT)),
            "--workflows-dir",
            str(config.workflows_dir.relative_to(ROOT)),
        ],
        cwd=ROOT,
        stdout=subprocess.DEVNULL,
        stderr=subprocess.PIPE,
    )
    try:
        wait_for_http_ready(
            f"{config.api_base_url}/api/workflows?limit=5&offset=0",
            timeout_s=30.0,
            contains_text=DEFAULT_WORKFLOW_NAME,
        )

        master_fd, slave_fd = pty.openpty()
        set_pty_size(slave_fd, rows=40, cols=140)
        env = os.environ.copy()
        env["CZAR_TUI_RENDER_DUMP_PATH"] = str(dump_file)
        tui_process = subprocess.Popen(
            ["cargo", "run", "-q", "-p", "czar-tui", "--", "--base-url", config.api_base_url],
            cwd=ROOT,
            env=env,
            stdin=slave_fd,
            stdout=slave_fd,
            stderr=subprocess.PIPE,
            close_fds=True,
        )
        os.close(slave_fd)
        try:
            wait_for_render_text(dump_file, DEFAULT_WORKFLOW_NAME, timeout_s=20.0)
            os.write(master_fd, b"t")
            wait_for_run_success(dump_file, timeout_s=40.0)
            os.write(master_fd, b"q")
            tui_process.wait(timeout=10.0)
            if tui_process.returncode != 0:
                raise RuntimeError(f"TUI exited with code {tui_process.returncode}")
            stderr_output = (
                tui_process.stderr.read().decode("utf-8", errors="replace")
                if tui_process.stderr is not None
                else ""
            )
            if "Error:" in stderr_output or "panic" in stderr_output.lower():
                raise RuntimeError(f"TUI reported an error:\n{stderr_output}")
            print(f"TUI smoke test passed against {config.api_base_url}")
            return 0
        finally:
            os.close(master_fd)
            terminate_process(tui_process, name="czar-tui")
    finally:
        terminate_process(api_process, name="smoke API server")


def main() -> int:
    args = parse_args()
    if args.command == "cli":
        return run_cli_smoke()
    if args.command == "tui":
        return run_tui_smoke(args.addr, args.db_dir, args.dump_path)
    raise ValueError(f"unsupported command: {args.command}")


if __name__ == "__main__":
    raise SystemExit(main())
