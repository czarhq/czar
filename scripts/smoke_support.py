#!/usr/bin/env python3
from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
import urllib.error
import urllib.request
from dataclasses import dataclass
from pathlib import Path
from typing import Final

ROOT: Final[Path] = Path(__file__).resolve().parent.parent
DEFAULT_WORKFLOWS_DIR: Final[Path] = ROOT / "examples" / "workflows" / "simple_args"
DEFAULT_WORKFLOW_FILE: Final[Path] = DEFAULT_WORKFLOWS_DIR / "simple_args.yaml"
DEFAULT_WORKFLOW_NAME: Final[str] = "simple_args"


@dataclass(frozen=True)
class SmokeStackConfig:
    addr: str
    db_dir: Path
    workflows_dir: Path = DEFAULT_WORKFLOWS_DIR

    @property
    def api_base_url(self) -> str:
        return f"http://{self.addr}"

    @property
    def database_path(self) -> Path:
        return self.db_dir / "czar.db"

    @property
    def database_url(self) -> str:
        return f"sqlite://{self.database_path}?mode=rwc"


def with_pythonpath(root: Path, current_pythonpath: str | None) -> str:
    sdk_path = str(root / "python-sdk")
    if current_pythonpath:
        return f"{sdk_path}:{current_pythonpath}"
    return sdk_path


def smoke_env(config: SmokeStackConfig) -> dict[str, str]:
    env = os.environ.copy()
    env["DATABASE_URL"] = config.database_url
    env["CZAR_BOOTSTRAP_WORKFLOW_DEFINITIONS"] = "1"
    env["CZAR_WORKFLOW_DEFINITIONS_DIR"] = str(config.workflows_dir)
    env["PYTHONPATH"] = with_pythonpath(ROOT, env.get("PYTHONPATH"))
    return env


def remove_sqlite_artifacts(database_path: Path) -> None:
    for candidate in (
        database_path,
        database_path.with_name(f"{database_path.name}-shm"),
        database_path.with_name(f"{database_path.name}-wal"),
    ):
        candidate.unlink(missing_ok=True)


def run_checked(argv: list[str], *, env: dict[str, str], cwd: Path = ROOT) -> None:
    subprocess.run(argv, cwd=cwd, env=env, check=True)


def wait_for_http_ready(url: str, *, timeout_s: float, contains_text: str | None = None) -> str:
    deadline = time.monotonic() + timeout_s
    last_error: str | None = None
    while time.monotonic() < deadline:
        try:
            with urllib.request.urlopen(url, timeout=2.0) as response:
                body = response.read().decode("utf-8")
                if contains_text is None or contains_text in body:
                    return body
                last_error = f"response from {url} did not contain {contains_text!r}"
        except (TimeoutError, ConnectionError, OSError, urllib.error.URLError) as exc:
            last_error = str(exc)
        time.sleep(0.25)
    if last_error is None:
        last_error = "unknown readiness failure"
    raise TimeoutError(f"timed out waiting for {url}: {last_error}")


def terminate_process(process: subprocess.Popen[bytes], *, name: str, timeout_s: float = 10.0) -> None:
    if process.poll() is not None:
        return
    process.send_signal(signal.SIGTERM)
    try:
        process.wait(timeout=timeout_s)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=timeout_s)
    if process.returncode not in (0, -signal.SIGTERM):
        print(f"{name} exited with code {process.returncode}", file=sys.stderr)
