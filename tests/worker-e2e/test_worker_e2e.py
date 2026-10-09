"""
End-to-end test: orchestrator + czar-worker running in Docker Compose.

The orchestrator reads workers.yaml, calls /compile on the worker to discover
the workflow, then dispatches each task back to the worker via /execute.
A passing run proves the full orchestrator -> worker path works.
"""
from __future__ import annotations

import contextlib
import http.client
import json
import os
import socket
import subprocess
import time
from pathlib import Path
from typing import Any

import pytest

ROOT = Path(__file__).resolve().parent.parent.parent
TEST_DIR = Path(__file__).resolve().parent
COMPOSE_FILE = TEST_DIR / "docker-compose.yml"

STARTUP_TIMEOUT_S = 300.0
RUN_TIMEOUT_S = 60.0
REQUEST_TIMEOUT_S = 10.0


# ---------------------------------------------------------------------------
# Fixtures
# ---------------------------------------------------------------------------

@pytest.fixture(scope="session")
def compose_env() -> dict[str, str]:
    return {"ORCHESTRATOR_PORT": str(_free_port())}


@pytest.fixture(scope="session", autouse=True)
def compose_stack(compose_env: dict[str, str]):
    """Start the Docker Compose stack for the session and tear it down after."""
    print(f"\nBuilding images and starting containers (project={_project_name()})...")
    _compose_checked("up", "--build", "--detach", env=compose_env)
    yield
    _compose("down", "--volumes", "--remove-orphans", env=compose_env)


@pytest.fixture(scope="session")
def orchestrator(compose_env: dict[str, str]) -> tuple[str, int]:
    """Wait for the orchestrator to be ready, return (host, port)."""
    host = "127.0.0.1"
    port = int(compose_env["ORCHESTRATOR_PORT"])

    print(f"Waiting for orchestrator on :{port} (up to {int(STARTUP_TIMEOUT_S)}s)...")
    _poll_until(
        lambda: _get(host, port, "/api/runs")[0] == 200,
        timeout_s=STARTUP_TIMEOUT_S,
        error=f"orchestrator did not become ready within {STARTUP_TIMEOUT_S}s",
    )

    print("Orchestrator ready. Waiting for worker_e2e workflow to sync...")
    _poll_until(
        lambda: _get(host, port, "/api/workflows/worker_e2e")[0] == 200,
        timeout_s=STARTUP_TIMEOUT_S,
        error="workflow 'worker_e2e' was not synced from worker within timeout",
    )
    print("Workflow synced.")
    return host, port


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------

def test_worker_e2e_workflow_succeeds(orchestrator: tuple[str, int]) -> None:
    """Submit the worker_e2e workflow and assert it completes successfully."""
    host, port = orchestrator
    run_id = _submit_run(host, port, workflow="worker_e2e")
    print(f"Run submitted: {run_id}")
    _wait_for_success(host, port, run_id)
    print("Run completed successfully.")


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------

def _submit_run(host: str, port: int, workflow: str) -> str:
    status, body = _post(host, port, "/api/runs", {"workflow": workflow})
    assert status in (200, 201), f"POST /api/runs returned {status}: {body}"
    run_id = body.get("run_id")
    assert run_id, f"no run_id in response: {body}"
    return run_id


def _wait_for_success(host: str, port: int, run_id: str) -> None:
    print(f"Polling run {run_id} (up to {int(RUN_TIMEOUT_S)}s)...")

    def _check() -> bool:
        status, body = _get(host, port, f"/api/runs/{run_id}")
        if status != 200:
            return False
        run_status = body.get("run", body).get("status", "")
        if run_status == "success":
            return True
        assert run_status not in ("failed", "error"), \
            f"run {run_id} ended with status={run_status}: {body}"
        return False

    _poll_until(_check, timeout_s=RUN_TIMEOUT_S, error=f"run {run_id} did not succeed within {RUN_TIMEOUT_S}s")


def _poll_until(condition: Any, *, timeout_s: float, error: str) -> None:
    deadline = time.monotonic() + timeout_s
    while time.monotonic() < deadline:
        try:
            if condition():
                return
        except AssertionError:
            raise
        except Exception:
            pass
        time.sleep(0.5)
    pytest.fail(error)


# ---------------------------------------------------------------------------
# HTTP
# ---------------------------------------------------------------------------

def _get(host: str, port: int, path: str) -> tuple[int, dict[str, Any]]:
    return _request("GET", host, port, path)


def _post(host: str, port: int, path: str, payload: object = None) -> tuple[int, dict[str, Any]]:
    return _request("POST", host, port, path, payload)


def _request(method: str, host: str, port: int, path: str, payload: object = None) -> tuple[int, dict[str, Any]]:
    body = json.dumps(payload, separators=(",", ":")).encode() if payload is not None else None
    headers = {"Content-Type": "application/json"} if body else {}
    conn = http.client.HTTPConnection(host, port, timeout=REQUEST_TIMEOUT_S)
    try:
        conn.request(method, path, body=body, headers=headers)
        resp = conn.getresponse()
        text = resp.read().decode("utf-8", errors="replace")
        return resp.status, json.loads(text) if text else {}
    finally:
        conn.close()


# ---------------------------------------------------------------------------
# Docker Compose
# ---------------------------------------------------------------------------

def _project_name() -> str:
    return f"czar-worker-e2e-{int(time.time())}"


def _compose(*args: str, env: dict[str, str] | None = None) -> subprocess.CompletedProcess[str]:
    cmd = ["docker", "compose", "-f", str(COMPOSE_FILE), *args]
    return subprocess.run(
        cmd, cwd=ROOT, text=True, capture_output=True, check=False,
        env={**os.environ, **(env or {})},
    )


def _compose_checked(*args: str, env: dict[str, str] | None = None) -> None:
    result = _compose(*args, env=env)
    if result.returncode != 0:
        pytest.fail(
            f"docker compose {' '.join(args)} failed ({result.returncode})\n"
            f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}"
        )


# ---------------------------------------------------------------------------
# Port allocation
# ---------------------------------------------------------------------------

def _free_port() -> int:
    with contextlib.closing(socket.socket(socket.AF_INET, socket.SOCK_STREAM)) as s:
        s.bind(("127.0.0.1", 0))
        return int(s.getsockname()[1])
