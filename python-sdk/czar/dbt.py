import json
import os
import shutil
import subprocess
from pathlib import Path
from typing import Any

from . import Context


def _command_for_resource(resource_type: str) -> str:
    if resource_type == "model":
        return "run"
    if resource_type == "test":
        return "test"
    if resource_type == "seed":
        return "seed"
    if resource_type == "snapshot":
        return "snapshot"
    raise ValueError(f"Unsupported dbt resource_type: {resource_type}")


def _emit_lines(raw: str, *, prefix: str) -> None:
    for line in raw.splitlines():
        line = line.rstrip()
        if line:
            print(f"{prefix}{line}", flush=True)


def _load_result(path: Path, unique_id: str) -> dict[str, Any]:
    if not path.exists():
        return {}
    payload = json.loads(path.read_text())
    results = payload.get("results", [])
    for result in results:
        if result.get("unique_id") == unique_id:
            return result
    return {}


def run_node(task_input: dict[str, Any], ctx=None) -> dict[str, Any]:
    ctx = ctx or Context.from_env(raw_input=task_input)

    project_dir = Path(task_input["project_dir"]).resolve()
    profiles_dir = Path(task_input.get("profiles_dir") or project_dir).resolve()
    target_path = Path(task_input["target_path"]).resolve()
    target_path.mkdir(parents=True, exist_ok=True)

    unique_id = str(task_input["unique_id"])
    selector = str(task_input.get("selector") or unique_id)
    resource_type = str(task_input["resource_type"])
    node_name = str(task_input.get("node_name") or unique_id)
    target = task_input.get("target")
    profile = task_input.get("profile")
    threads = task_input.get("threads")

    dbt_executable = shutil.which("dbt")
    if not dbt_executable:
        raise RuntimeError("dbt executable was not found in PATH")

    command = [
        dbt_executable,
        _command_for_resource(resource_type),
        "--project-dir",
        str(project_dir),
        "--profiles-dir",
        str(profiles_dir),
        "--target-path",
        str(target_path),
        "--select",
        selector,
        "--no-use-colors",
    ]
    if profile:
        command.extend(["--profile", str(profile)])
    if target:
        command.extend(["--target", str(target)])
    if threads:
        command.extend(["--threads", str(threads)])

    env = os.environ.copy()
    env.setdefault("PYTHONUNBUFFERED", "1")
    ctx.log.info(f"dbt {resource_type} {node_name}")
    completed = subprocess.run(
        command,
        cwd=project_dir,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )
    if completed.stdout:
        _emit_lines(completed.stdout, prefix="")
    if completed.stderr:
        _emit_lines(completed.stderr, prefix="STDERR: ")
    if completed.returncode != 0:
        raise RuntimeError(
            f"dbt {_command_for_resource(resource_type)} failed for {unique_id} with exit code {completed.returncode}"
        )

    run_results_path = target_path / "run_results.json"
    result = _load_result(run_results_path, unique_id)
    adapter_response = result.get("adapter_response")
    output = {
        "unique_id": unique_id,
        "resource_type": resource_type,
        "name": node_name,
        "status": result.get("status", "success"),
        "execution_time": result.get("execution_time"),
        "adapter_response": adapter_response,
        "message": result.get("message"),
        "failures": result.get("failures"),
        "target_path": str(target_path),
    }
    return output
