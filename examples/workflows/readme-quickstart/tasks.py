# tasks.py
import json
from pathlib import Path
from typing import TypedDict

from czar import Context, task
import requests

class User(TypedDict):
    id: int
    status: str

class ExtractResult(TypedDict):
    raw_path: str
    user_count: int

class TransformResult(TypedDict):
    active_path: str
    active_count: int

class LoadResult(TypedDict):
    output_path: str


def load_users(api_url: str) -> list[User]:
    if api_url.startswith("file://"):
        relative_path = api_url.removeprefix("file://")
        source_path = Path(relative_path)
        if not source_path.is_absolute():
            source_path = Path(__file__).parent / source_path
        return json.loads(source_path.read_text(encoding="utf-8"))

    response = requests.get(api_url, timeout=30)
    response.raise_for_status()
    return response.json()


@task
def extract(api_url: str, raw_path: str, ctx: Context) -> ExtractResult:
    ctx.log.info(f"Extracting users from {api_url}")
    users = load_users(api_url)
    raw_file = Path(raw_path)
    raw_file.parent.mkdir(parents=True, exist_ok=True)
    raw_file.write_text(json.dumps(users), encoding="utf-8")
    return {"raw_path": raw_path, "user_count": len(users)}

@task
def transform(raw_path: str, active_path: str, ctx: Context) -> TransformResult:
    ctx.log.info(f"Transforming {raw_path} into {active_path}")
    users: list[User] = json.loads(Path(raw_path).read_text(encoding="utf-8"))
    active_users = [user for user in users if user["status"] == "active"]
    active_file = Path(active_path)
    active_file.parent.mkdir(parents=True, exist_ok=True)
    active_file.write_text(json.dumps(active_users), encoding="utf-8")
    return {"active_path": active_path, "active_count": len(active_users)}

@task
def load(active_path: str, output_path: str, ctx: Context) -> LoadResult:
    ctx.log.info(f"Writing final artifact to {output_path}")
    output_file = Path(output_path)
    output_file.parent.mkdir(parents=True, exist_ok=True)
    output_file.write_text(
        Path(active_path).read_text(encoding="utf-8"),
        encoding="utf-8",
    )
    return {"output_path": output_path}
