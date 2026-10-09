#!/usr/bin/env python3
from __future__ import annotations

import argparse
import contextlib
import http.client
import json
import os
import shutil
import socket
import subprocess
import time
from pathlib import Path
from typing import Any, TypedDict, cast
from urllib.parse import urlparse
from urllib.request import Request, urlopen

ROOT = Path(__file__).resolve().parent.parent
DOCKER_BIN = shutil.which("docker")
DEV_COMPOSE_FILE = ROOT / "crates" / "czar-cli" / "docker-compose.dev.yml"
DEFAULT_DEV_FRONTEND_HOST = "127.0.0.1"
DEFAULT_DEV_API_HOST = "127.0.0.1"
DEV_STACK_STARTUP_TIMEOUT_S = 300.0
DEV_STACK_PROXY_TIMEOUT_S = 60.0
DEV_STACK_RUN_TIMEOUT_S = 60.0


class DockerSmokeError(RuntimeError):
    pass


class RunRecord(TypedDict):
    status: str


class CreateRunResponse(TypedDict):
    run_id: str


class RunDetailResponse(TypedDict):
    run: RunRecord


def require_docker() -> str:
    if DOCKER_BIN is None:
        raise DockerSmokeError("docker is required for just docker-smoke")
    return DOCKER_BIN


def run_checked(argv: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    completed = subprocess.run(
        argv,
        cwd=cwd or ROOT,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.returncode != 0:
        raise DockerSmokeError(
            f"command failed ({completed.returncode}): {' '.join(argv)}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )
    return completed


def run_capture(argv: list[str], *, cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        argv,
        cwd=cwd or ROOT,
        text=True,
        capture_output=True,
        check=False,
    )


def free_tcp_port() -> int:
    with contextlib.closing(socket.socket(socket.AF_INET, socket.SOCK_STREAM)) as sock:
        sock.bind(("127.0.0.1", 0))
        return int(sock.getsockname()[1])


def build_image(tag: str, dockerfile: Path, context_dir: Path) -> None:
    docker = require_docker()
    run_checked(
        [
            docker,
            "build",
            "--file",
            str(dockerfile),
            "--tag",
            tag,
            str(context_dir),
        ]
    )


def smoke_cli_image(tag: str) -> None:
    docker = require_docker()
    completed = run_checked([docker, "run", "--rm", tag, "--help"])
    help_text = f"{completed.stdout}\n{completed.stderr}"
    if "czar" not in help_text.lower():
        raise DockerSmokeError("CLI Docker image ran, but '--help' output did not look like Czar")


def wait_for_http_ready(url: str, *, timeout_s: float) -> str:
    parsed = urlparse(url)
    if parsed.scheme != "http" or not parsed.hostname or parsed.port is None:
        raise DockerSmokeError(f"unsupported smoke URL: {url}")

    deadline = time.monotonic() + timeout_s
    last_error = "no response"
    while time.monotonic() < deadline:
        connection = http.client.HTTPConnection(parsed.hostname, parsed.port, timeout=2.0)
        try:
            connection.request("GET", parsed.path or "/")
            response = connection.getresponse()
            status = response.status
            body = response.read().decode("utf-8", errors="replace")
            if status == 200:
                return body
            last_error = f"http {status}"
        except OSError as err:
            last_error = str(err)
        finally:
            connection.close()
        time.sleep(0.5)
    raise DockerSmokeError(f"timed out waiting for {url}: {last_error}")


def wait_for_http_contains(url: str, *, timeout_s: float, expected_text: str) -> str:
    parsed = urlparse(url)
    if parsed.scheme != "http" or not parsed.hostname or parsed.port is None:
        raise DockerSmokeError(f"unsupported smoke URL: {url}")

    deadline = time.monotonic() + timeout_s
    last_error = f"response from {url} did not contain {expected_text!r}"
    while time.monotonic() < deadline:
        connection = http.client.HTTPConnection(parsed.hostname, parsed.port, timeout=2.0)
        try:
            connection.request("GET", parsed.path or "/")
            response = connection.getresponse()
            status = response.status
            body = response.read().decode("utf-8", errors="replace")
            if status == 200 and expected_text in body:
                return body
            last_error = (
                f"response from {url} had status {status} and did not contain {expected_text!r}"
            )
        except OSError as err:
            last_error = str(err)
        finally:
            connection.close()
        time.sleep(0.5)
    raise DockerSmokeError(f"timed out waiting for {url}: {last_error}")


def post_json(url: str, payload: object) -> dict[str, Any]:
    encoded = json.dumps(payload, separators=(",", ":"), ensure_ascii=True).encode("utf-8")
    request = Request(
        url,
        data=encoded,
        headers={"Content-Type": "application/json"},
        method="POST",
    )
    with urlopen(request, timeout=10.0) as response:  # noqa: S310
        status = response.status
        body = response.read().decode("utf-8", errors="replace")
    if status >= 400:
        raise DockerSmokeError(f"request failed: POST {url} -> {status}\n{body}")
    return cast(dict[str, Any], json.loads(body))


def retry_post_json(url: str, payload: object, *, timeout_s: float) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_s
    last_error = "no response"
    while time.monotonic() < deadline:
        try:
            return post_json(url, payload)
        except OSError as err:
            last_error = str(err)
            time.sleep(0.5)
    raise DockerSmokeError(f"timed out posting to {url}: {last_error}")


def get_json(url: str) -> dict[str, Any]:
    with urlopen(url, timeout=10.0) as response:  # noqa: S310
        status = response.status
        body = response.read().decode("utf-8", errors="replace")
    if status >= 400:
        raise DockerSmokeError(f"request failed: GET {url} -> {status}\n{body}")
    return cast(dict[str, Any], json.loads(body))


def wait_for_run_success(base_url: str, run_id: str, *, timeout_s: float) -> None:
    deadline = time.monotonic() + timeout_s
    last_status = "unknown"
    while time.monotonic() < deadline:
        payload = cast(RunDetailResponse, get_json(f"{base_url}/api/runs/{run_id}"))
        status = payload["run"]["status"]
        last_status = status
        if status == "success":
            return
        if status in {"failed", "cancelled"}:
            raise DockerSmokeError(f"run {run_id} finished with terminal status {status}")
        time.sleep(0.5)
    raise DockerSmokeError(f"timed out waiting for run {run_id} to succeed; last status {last_status}")


def compose_argv(project_name: str, *extra: str) -> list[str]:
    docker = require_docker()
    return [
        docker,
        "compose",
        "-p",
        project_name,
        "-f",
        str(DEV_COMPOSE_FILE),
        *extra,
    ]


def dev_compose_env(frontend_port: int, api_port: int, postgres_port: int) -> dict[str, str]:
    env = os.environ.copy()
    env["HOST_UID"] = str(os.getuid())
    env["HOST_GID"] = str(os.getgid())
    env["CZAR_DEV_FRONTEND_PORT"] = str(frontend_port)
    env["CZAR_DEV_API_PORT"] = str(api_port)
    env["CZAR_DEV_POSTGRES_PORT"] = str(postgres_port)
    env["CZAR_DEV_NO_WATCH"] = "1"
    return env


def run_checked_with_env(argv: list[str], *, env: dict[str, str], cwd: Path | None = None) -> None:
    completed = subprocess.run(
        argv,
        cwd=cwd or ROOT,
        text=True,
        capture_output=True,
        check=False,
        env=env,
    )
    if completed.returncode != 0:
        raise DockerSmokeError(
            f"command failed ({completed.returncode}): {' '.join(argv)}\n"
            f"stdout:\n{completed.stdout}\n"
            f"stderr:\n{completed.stderr}"
        )


def run_capture_with_env(argv: list[str], *, env: dict[str, str], cwd: Path | None = None) -> str:
    completed = subprocess.run(
        argv,
        cwd=cwd or ROOT,
        text=True,
        capture_output=True,
        check=False,
        env=env,
    )
    stdout = completed.stdout.strip()
    stderr = completed.stderr.strip()
    parts = [part for part in [stdout, stderr] if part]
    return "\n".join(parts)


def smoke_web_image(tag: str) -> None:
    docker = require_docker()
    name = f"czar-web-smoke-{int(time.time())}"
    port = free_tcp_port()
    run_checked(
        [
            docker,
            "run",
            "--detach",
            "--name",
            name,
            "--publish",
            f"127.0.0.1:{port}:80",
            tag,
        ]
    )
    try:
        body = wait_for_http_ready(f"http://127.0.0.1:{port}/", timeout_s=20.0)
        if "<!doctype html" not in body.lower() and "<html" not in body.lower():
            raise DockerSmokeError("web Docker image served a response, but it did not look like HTML")
    except Exception:
        logs = run_capture([docker, "logs", name]).stdout
        inspect = run_capture(
            [docker, "inspect", "--format", "{{.State.Status}} {{.State.ExitCode}}", name]
        ).stdout.strip()
        raise DockerSmokeError(
            "web Docker smoke failed\n"
            f"container state: {inspect or 'unknown'}\n"
            f"container logs:\n{logs}"
        ) from None
    finally:
        subprocess.run([docker, "rm", "--force", name], text=True, capture_output=True, check=False)


def smoke_dev_stack() -> None:
    frontend_port = free_tcp_port()
    api_port = free_tcp_port()
    postgres_port = free_tcp_port()
    project_name = f"czar-dev-smoke-{int(time.time())}"
    env = dev_compose_env(frontend_port, api_port, postgres_port)
    frontend_base_url = f"http://{DEFAULT_DEV_FRONTEND_HOST}:{frontend_port}"
    api_base_url = f"http://{DEFAULT_DEV_API_HOST}:{api_port}"

    try:
        run_checked_with_env(compose_argv(project_name, "up", "--build", "--detach"), env=env)
        body = wait_for_http_ready(f"{frontend_base_url}/", timeout_s=60.0)
        if "<!doctype html" not in body.lower() and "<html" not in body.lower():
            raise DockerSmokeError("dev frontend responded, but the body did not look like HTML")

        wait_for_http_contains(
            f"{api_base_url}/api/workflows?limit=20&offset=0",
            timeout_s=DEV_STACK_STARTUP_TIMEOUT_S,
            expected_text="simple_args",
        )
        wait_for_http_contains(
            f"{frontend_base_url}/api/workflows?limit=20&offset=0",
            timeout_s=DEV_STACK_PROXY_TIMEOUT_S,
            expected_text="simple_args",
        )
        created = cast(
            CreateRunResponse,
            retry_post_json(
                f"{frontend_base_url}/api/runs",
                {"workflow": "simple_args"},
                timeout_s=DEV_STACK_PROXY_TIMEOUT_S,
            ),
        )
        wait_for_run_success(api_base_url, created["run_id"], timeout_s=DEV_STACK_RUN_TIMEOUT_S)
    except Exception as exc:
        ps_output = run_capture_with_env(compose_argv(project_name, "ps"), env=env)
        logs_output = run_capture_with_env(
            compose_argv(project_name, "logs", "--no-color", "--tail", "200"),
            env=env,
        )
        raise DockerSmokeError(
            "dev docker compose smoke failed\n"
            f"frontend: {frontend_base_url}\n"
            f"api: {api_base_url}\n"
            f"error: {exc}\n\n"
            f"compose ps:\n{ps_output}\n\n"
            f"compose logs:\n{logs_output}"
        ) from None
    finally:
        subprocess.run(
            compose_argv(project_name, "down", "--volumes", "--remove-orphans"),
            cwd=ROOT,
            text=True,
            capture_output=True,
            check=False,
            env=env,
        )


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Run Docker-based smoke tests.")
    parser.add_argument(
        "target",
        nargs="?",
        choices=["standalone", "dev-stack", "all"],
        default="all",
        help="which Docker smoke path to run",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    cli_tag = "czar-cli-smoke:local"
    web_tag = "czar-web-react-smoke:local"

    if args.target in {"standalone", "all"}:
        print("Building CLI Docker image...")
        build_image(cli_tag, ROOT / "crates" / "czar-cli" / "Dockerfile", ROOT)
        print("Running CLI Docker image...")
        smoke_cli_image(cli_tag)

        print("Building web Docker image...")
        web_root = ROOT / "crates" / "czar-web-react"
        build_image(web_tag, web_root / "Dockerfile", web_root)
        print("Running web Docker image...")
        smoke_web_image(web_tag)
        print("Standalone Docker smoke tests passed")

    if args.target in {"dev-stack", "all"}:
        print("Starting dev docker compose smoke stack...")
        smoke_dev_stack()
        print("Dev docker compose smoke test passed")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
