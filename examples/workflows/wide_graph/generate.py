#!/usr/bin/env python3
"""Generate a wide graph workflow YAML file."""

from __future__ import annotations

import argparse
from pathlib import Path
from typing import TypedDict

import yaml

LAYER_COUNT = 10
NODES_PER_LAYER = 100
WORKFLOW_NAME = "wide_graph"


class TaskSpec(TypedDict, total=False):
    executor: str
    command: str
    depends_on: list[str]


class WorkflowSpec(TypedDict, total=False):
    name: str
    tasks: dict[str, TaskSpec]
    task_groups: dict[str, list[str]]


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", required=True)
    parser.add_argument("--workflow-name", default=WORKFLOW_NAME)
    parser.add_argument("--layers", type=int, default=LAYER_COUNT)
    parser.add_argument("--nodes-per-layer", type=int, default=NODES_PER_LAYER)
    args = parser.parse_args()

    workflow = build_workflow(
        workflow_name=args.workflow_name,
        layers=args.layers,
        nodes_per_layer=args.nodes_per_layer,
    )
    output_path = Path(args.output)
    output_path.write_text(yaml.safe_dump(workflow, sort_keys=False), encoding="utf-8")


def build_workflow(workflow_name: str, layers: int, nodes_per_layer: int) -> WorkflowSpec:
    tasks: dict[str, TaskSpec] = {}
    task_groups: dict[str, list[str]] = {}

    for layer in range(layers):
        layer_tasks: list[str] = []
        for idx in range(nodes_per_layer):
            name = task_name(layer, idx)
            task: TaskSpec = {
                "executor": "process",
                "command": f"echo {name}",
            }
            if layer > 0:
                task["depends_on"] = [task_name(layer - 1, idx)]
            tasks[name] = task
            layer_tasks.append(name)
        task_groups[f"layer_{layer}"] = layer_tasks

    return {
        "name": workflow_name,
        "tasks": tasks,
        "task_groups": task_groups,
    }


def task_name(layer: int, idx: int) -> str:
    return f"t_{layer}_{idx:02d}"


if __name__ == "__main__":
    main()
