"""Lightweight Czar authoring helpers for Python tasks."""

from __future__ import annotations

import json
import os
import re
from dataclasses import dataclass, field
from typing import Any, Callable, TypeVar

from .deferrables import BigQueryJob, CloudRunJob, CustomHttp, DataprocJob, Deferrable
from .progress import czar_progress

F = TypeVar("F", bound=Callable[..., Any])

_PARALLEL_SUFFIX_RE = re.compile(r"\[(\d+)\]$")


def _json_default(value: Any) -> Any:
    if hasattr(value, "model_dump"):
        return value.model_dump()
    if hasattr(value, "dict"):
        return value.dict()
    if hasattr(value, "__dict__"):
        return value.__dict__
    return str(value)


def _env_int(name: str) -> int | None:
    raw = os.environ.get(name)
    if raw is None or raw == "":
        return None
    try:
        return int(raw)
    except ValueError:
        return None


def _parallel_index_from_task_name(task_name: str | None) -> int | None:
    if not task_name:
        return None
    match = _PARALLEL_SUFFIX_RE.search(task_name)
    if not match:
        return None
    return int(match.group(1))


class _ContextLogger:
    __slots__ = ("_context",)

    def __init__(self, context: "Context") -> None:
        self._context = context

    def __call__(self, message: Any) -> None:
        self.info(message)

    def debug(self, message: Any) -> None:
        self._context._emit_log("DEBUG", message)

    def info(self, message: Any) -> None:
        self._context._emit_log("INFO", message)

    def warning(self, message: Any) -> None:
        self._context._emit_log("WARNING", message)

    def error(self, message: Any) -> None:
        self._context._emit_log("ERROR", message)


class _MetricsRecorder:
    __slots__ = ()

    def record(
        self,
        name: str,
        value: Any,
        *,
        labels: dict[str, Any] | None = None,
    ) -> None:
        payload: dict[str, Any] = {"name": str(name), "value": value}
        if labels:
            payload["labels"] = labels
        print(
            f"CZAR_METRIC:{json.dumps(payload, default=_json_default)}",
            flush=True,
        )


def task(func: F | None = None, **options: Any):
    """Mark a function as an Czar task without altering call semantics."""

    def decorator(inner: F) -> F:
        setattr(inner, "__czar_task__", dict(options))
        return inner

    if func is None:
        return decorator
    return decorator(func)


def executor(cls: type | None = None, **options: Any):
    """Mark a class as an Czar custom executor."""

    def decorator(inner: type) -> type:
        setattr(inner, "__czar_executor__", dict(options))
        return inner

    if cls is None:
        return decorator
    return decorator(cls)


@dataclass(slots=True)
class Context:
    """Runtime metadata exposed to Python tasks."""

    run_id: str | None = None
    workflow_name: str | None = None
    task_name: str | None = None
    task_id: str | None = None
    execution_name: str | None = None
    attempt: int | None = None
    parallel_index: int | None = None
    parallel_count: int | None = None
    raw_input: Any = None
    upstream: Any = None
    log: _ContextLogger = field(init=False, repr=False)
    metrics: _MetricsRecorder = field(init=False, repr=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "log", _ContextLogger(self))
        object.__setattr__(self, "metrics", _MetricsRecorder())

    @classmethod
    def from_env(cls, *, raw_input: Any = None, upstream: Any = None) -> "Context":
        task_name = os.environ.get("CZAR_TASK_NAME")

        return cls(
            run_id=os.environ.get("CZAR_RUN_ID"),
            workflow_name=os.environ.get("CZAR_WORKFLOW_NAME"),
            task_name=task_name,
            task_id=os.environ.get("CZAR_TASK_ID"),
            execution_name=os.environ.get("CZAR_EXECUTION_NAME"),
            attempt=_env_int("CZAR_ATTEMPT"),
            parallel_index=_env_int("CZAR_PARALLEL_INDEX")
            or _parallel_index_from_task_name(task_name),
            parallel_count=_env_int("CZAR_PARALLEL_COUNT"),
            raw_input=raw_input,
            upstream=upstream,
        )

    def _emit_log(self, level: str, message: Any) -> None:
        text = str(message)
        if level in {"WARNING", "ERROR", "DEBUG"}:
            text = f"{level}: {text}"
        if text.endswith("\n"):
            print(text, end="", flush=True)
        else:
            print(text, flush=True)

    def progress(self, current: int, total: int, message: str | None = None) -> None:
        czar_progress(current, total, message=message)

    def to_dict(self) -> dict[str, Any]:
        return {
            "run_id": self.run_id,
            "workflow_name": self.workflow_name,
            "task_name": self.task_name,
            "task_id": self.task_id,
            "execution_name": self.execution_name,
            "attempt": self.attempt,
            "parallel_index": self.parallel_index,
            "parallel_count": self.parallel_count,
            "raw_input": self.raw_input,
            "upstream": self.upstream,
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict())


__all__ = [
    "BigQueryJob",
    "CloudRunJob",
    "Context",
    "CustomHttp",
    "DataprocJob",
    "Deferrable",
    "executor",
    "czar_progress",
    "task",
]
