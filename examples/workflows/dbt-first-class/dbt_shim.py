import argparse
import csv
import json
import re
import sys
import time
from pathlib import Path

import duckdb


REF_RE = re.compile(r"\{\{\s*ref\(['\"]([^'\"]+)['\"]\)\s*\}\}")


def _read_project_name(project_dir: Path) -> str:
    project_file = project_dir / "dbt_project.yml"
    for line in project_file.read_text().splitlines():
        stripped = line.strip()
        if stripped.startswith("name:"):
            return stripped.split(":", 1)[1].strip()
    raise RuntimeError(f"Could not find project name in {project_file}")


def _read_duckdb_path(profiles_dir: Path) -> Path:
    profiles_file = profiles_dir / "profiles.yml"
    for line in profiles_file.read_text().splitlines():
        stripped = line.strip()
        if stripped.startswith("path:"):
            raw_path = stripped.split(":", 1)[1].strip()
            return (profiles_dir / raw_path).resolve()
    raise RuntimeError(f"Could not find DuckDB path in {profiles_file}")


def _materialized_for_model(relative_parts: tuple[str, ...]) -> str:
    if relative_parts and relative_parts[0] == "staging":
        return "view"
    return "table"


def _parse_refs(sql: str, name_to_unique_id: dict[str, str]) -> list[str]:
    refs = []
    for match in REF_RE.finditer(sql):
        ref_name = match.group(1)
        unique_id = name_to_unique_id.get(ref_name)
        if unique_id:
            refs.append(unique_id)
    return refs


def _relative_posix(path: Path, base: Path) -> str:
    return path.relative_to(base).as_posix()


def _build_manifest(project_dir: Path) -> dict:
    project_name = _read_project_name(project_dir)
    nodes: dict[str, dict] = {}
    name_to_unique_id: dict[str, str] = {}

    seeds_dir = project_dir / "seeds"
    for seed_file in sorted(seeds_dir.rglob("*.csv")):
        name = seed_file.stem
        unique_id = f"seed.{project_name}.{name}"
        relative = seed_file.relative_to(seeds_dir)
        node = {
            "unique_id": unique_id,
            "name": name,
            "resource_type": "seed",
            "path": relative.as_posix(),
            "original_file_path": _relative_posix(seed_file, project_dir),
            "fqn": [project_name, name],
            "depends_on": {"macros": [], "nodes": []},
            "database": "warehouse",
            "schema": "main",
            "alias": name,
            "config": {},
        }
        nodes[unique_id] = node
        name_to_unique_id[name] = unique_id

    models_dir = project_dir / "models"
    model_files = sorted(models_dir.rglob("*.sql"))
    for model_file in model_files:
        name = model_file.stem
        relative = model_file.relative_to(models_dir)
        path_parts = relative.parts[:-1]
        unique_id = f"model.{project_name}.{name}"
        node = {
            "unique_id": unique_id,
            "name": name,
            "resource_type": "model",
            "path": relative.as_posix(),
            "original_file_path": _relative_posix(model_file, project_dir),
            "fqn": [project_name, *path_parts, name],
            "depends_on": {"macros": [], "nodes": []},
            "database": "warehouse",
            "schema": "main",
            "alias": name,
            "config": {"materialized": _materialized_for_model(path_parts)},
        }
        nodes[unique_id] = node
        name_to_unique_id[name] = unique_id

    for model_file in model_files:
        name = model_file.stem
        unique_id = name_to_unique_id[name]
        sql = model_file.read_text()
        nodes[unique_id]["depends_on"]["nodes"] = _parse_refs(sql, name_to_unique_id)

    tests_dir = project_dir / "tests"
    for test_file in sorted(tests_dir.rglob("*.sql")):
        name = test_file.stem
        unique_id = f"test.{project_name}.{name}"
        sql = test_file.read_text()
        node = {
            "unique_id": unique_id,
            "name": name,
            "resource_type": "test",
            "path": test_file.name,
            "original_file_path": _relative_posix(test_file, project_dir),
            "fqn": [project_name, name],
            "depends_on": {"macros": [], "nodes": _parse_refs(sql, name_to_unique_id)},
            "database": "warehouse",
            "schema": "main_dbt_test__audit",
            "alias": name,
            "config": {},
        }
        nodes[unique_id] = node
        name_to_unique_id[name] = unique_id

    return {
        "metadata": {"project_name": project_name},
        "nodes": nodes,
    }


def _load_manifest(project_dir: Path, target_path: Path) -> dict:
    for candidate in [target_path / "manifest.json", project_dir / "target" / "manifest.json"]:
        if candidate.exists():
            return json.loads(candidate.read_text())
    manifest = _build_manifest(project_dir)
    _write_manifest(manifest, target_path)
    if target_path != project_dir / "target":
        _write_manifest(manifest, project_dir / "target")
    return manifest


def _write_manifest(manifest: dict, target_path: Path) -> None:
    target_path.mkdir(parents=True, exist_ok=True)
    (target_path / "manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True))


def _selector_matches(node: dict, selector: str) -> bool:
    if node["unique_id"] == selector or node["name"] == selector:
        return True
    if node["resource_type"] == "test":
        return node["name"] == selector
    fqn = node.get("fqn", [])
    if len(fqn) > 1 and ".".join(fqn[1:]) == selector:
        return True
    path = node.get("path", "")
    stem = Path(path).with_suffix("").as_posix().replace("/", ".")
    return stem == selector


def _select_node(manifest: dict, selector: str) -> dict:
    for node in manifest.get("nodes", {}).values():
        if node.get("resource_type") not in {"seed", "model", "test"}:
            continue
        if _selector_matches(node, selector):
            return node
    raise RuntimeError(f"Could not find dbt node for selector '{selector}'")


def _render_sql(sql: str, manifest: dict) -> str:
    alias_by_name = {
        node["name"]: node.get("alias") or node["name"]
        for node in manifest.get("nodes", {}).values()
        if node.get("resource_type") in {"seed", "model"}
    }

    def replace(match: re.Match[str]) -> str:
        ref_name = match.group(1)
        return alias_by_name.get(ref_name, ref_name)

    return REF_RE.sub(replace, sql)


def _result_payload(
    *,
    unique_id: str,
    status: str,
    started_at: float,
    adapter_response: dict | None = None,
    message: str | None = None,
    failures: int | None = None,
) -> dict:
    return {
        "results": [
            {
                "unique_id": unique_id,
                "status": status,
                "execution_time": round(time.time() - started_at, 6),
                "adapter_response": adapter_response or {},
                "message": message,
                "failures": failures,
            }
        ]
    }


def _write_run_results(target_path: Path, payload: dict) -> None:
    target_path.mkdir(parents=True, exist_ok=True)
    (target_path / "run_results.json").write_text(json.dumps(payload, indent=2, sort_keys=True))


def _run_seed(node: dict, project_dir: Path, warehouse_path: Path) -> dict:
    seed_file = project_dir / node["original_file_path"]
    alias = node.get("alias") or node["name"]
    with duckdb.connect(str(warehouse_path)) as conn:
        conn.execute(
            f"create or replace table {alias} as select * from read_csv_auto(?, header=true)",
            [str(seed_file)],
        )
        row_count = conn.execute(f"select count(*) from {alias}").fetchone()[0]
    return {"rows_affected": int(row_count)}


def _run_model(node: dict, project_dir: Path, warehouse_path: Path, manifest: dict) -> dict:
    sql_file = project_dir / node["original_file_path"]
    rendered_sql = _render_sql(sql_file.read_text(), manifest)
    alias = node.get("alias") or node["name"]
    materialized = node.get("config", {}).get("materialized") or "table"
    relation_kind = "view" if materialized == "view" else "table"
    with duckdb.connect(str(warehouse_path)) as conn:
        conn.execute(f"create or replace {relation_kind} {alias} as {rendered_sql}")
    return {"relation_name": alias}


def _run_test(node: dict, project_dir: Path, warehouse_path: Path, manifest: dict) -> tuple[dict, int]:
    sql_file = project_dir / node["original_file_path"]
    rendered_sql = _render_sql(sql_file.read_text(), manifest)
    with duckdb.connect(str(warehouse_path), read_only=True) as conn:
        failures = conn.execute(f"select count(*) from ({rendered_sql}) as __czar_dbt_test").fetchone()[0]
    return {"failures": int(failures)}, int(failures)


def _command_parse(args: argparse.Namespace) -> int:
    manifest = _build_manifest(args.project_dir)
    _write_manifest(manifest, args.target_path)
    return 0


def _command_execute(args: argparse.Namespace) -> int:
    started_at = time.time()
    manifest = _load_manifest(args.project_dir, args.target_path)
    node = _select_node(manifest, args.select)
    warehouse_path = _read_duckdb_path(args.profiles_dir)
    warehouse_path.parent.mkdir(parents=True, exist_ok=True)

    try:
        if node["resource_type"] == "seed":
            adapter_response = _run_seed(node, args.project_dir, warehouse_path)
            payload = _result_payload(
                unique_id=node["unique_id"],
                status="success",
                started_at=started_at,
                adapter_response=adapter_response,
                failures=0,
            )
            _write_run_results(args.target_path, payload)
            return 0

        if node["resource_type"] == "model":
            adapter_response = _run_model(node, args.project_dir, warehouse_path, manifest)
            payload = _result_payload(
                unique_id=node["unique_id"],
                status="success",
                started_at=started_at,
                adapter_response=adapter_response,
                failures=0,
            )
            _write_run_results(args.target_path, payload)
            return 0

        if node["resource_type"] == "test":
            adapter_response, failures = _run_test(node, args.project_dir, warehouse_path, manifest)
            status = "pass" if failures == 0 else "fail"
            payload = _result_payload(
                unique_id=node["unique_id"],
                status=status,
                started_at=started_at,
                adapter_response=adapter_response,
                failures=failures,
                message="singular test failed" if failures else None,
            )
            _write_run_results(args.target_path, payload)
            return 0 if failures == 0 else 1
    except Exception as err:
        payload = _result_payload(
            unique_id=node["unique_id"],
            status="error",
            started_at=started_at,
            message=str(err),
        )
        _write_run_results(args.target_path, payload)
        print(str(err), file=sys.stderr)
        return 1

    raise RuntimeError(f"Unsupported resource type: {node['resource_type']}")


def _build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="dbt")
    subparsers = parser.add_subparsers(dest="command", required=True)

    parse_parser = subparsers.add_parser("parse")
    parse_parser.add_argument("--project-dir", required=True, type=Path)
    parse_parser.add_argument("--profiles-dir", type=Path)
    parse_parser.add_argument("--profile")
    parse_parser.add_argument("--target")
    parse_parser.add_argument("--target-path", type=Path, default=None)

    for command in ("seed", "run", "test"):
        command_parser = subparsers.add_parser(command)
        command_parser.add_argument("--project-dir", required=True, type=Path)
        command_parser.add_argument("--profiles-dir", required=True, type=Path)
        command_parser.add_argument("--target-path", required=True, type=Path)
        command_parser.add_argument("--select", required=True)
        command_parser.add_argument("--profile")
        command_parser.add_argument("--target")
        command_parser.add_argument("--threads")
        command_parser.add_argument("--no-use-colors", action="store_true")

    return parser


def main(argv: list[str] | None = None) -> int:
    parser = _build_parser()
    args = parser.parse_args(argv)
    args.project_dir = args.project_dir.resolve()
    args.target_path = (
        args.target_path.resolve() if args.target_path else (args.project_dir / "target").resolve()
    )
    if getattr(args, "profiles_dir", None) is not None:
        args.profiles_dir = args.profiles_dir.resolve()

    if args.command == "parse":
        return _command_parse(args)
    return _command_execute(args)


if __name__ == "__main__":
    raise SystemExit(main())
