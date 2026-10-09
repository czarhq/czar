#!/usr/bin/env python3
from __future__ import annotations

from collections.abc import Mapping, Sequence

from rich import box
from rich.spinner import Spinner
from rich.table import Table

STATUS_EMOJI = {
    "PASS": "✅",
    "FAIL": "❌",
    "INFO": "ℹ️",
    "WARN": "⚠️",
    "TIMEOUT": "⏱️",
    "SKIP": "⏭️",
    "PENDING": "🕒",
}
STATUS_STYLE = {
    "PASS": "green",
    "FAIL": "red",
    "INFO": "cyan",
    "WARN": "yellow",
    "TIMEOUT": "yellow",
    "SKIP": "dim",
    "PENDING": "bright_black",
}


def status_display(status: str) -> str:
    return f"{STATUS_EMOJI.get(status, '•')} {status}"


def status_markup(status: str) -> str:
    label = status_display(status)
    style = STATUS_STYLE.get(status, "white")
    return f"[{style}]{label}[/{style}]"


def render_progress_table(progress_rows: Sequence[Mapping[str, object]]) -> Table:
    table = Table(box=box.SIMPLE_HEAVY, show_header=True)
    table.add_column("#", style="bold cyan", justify="right")
    table.add_column("Check", style="bold")
    table.add_column("Command", overflow="fold")
    table.add_column("State", no_wrap=True)
    table.add_column("Duration", justify="right", no_wrap=True)
    table.add_column("Memory", no_wrap=True)
    table.add_column("Log", overflow="fold")
    for row in progress_rows:
        status = str(row["status"])
        state_renderable: object
        if status == "RUNNING":
            state_renderable = Spinner("dots", text="RUNNING", style="cyan")
        else:
            state_renderable = status_markup(status)
        duration = row["duration"]
        duration_text: str = f"{duration:.1f}s" if isinstance(duration, float) else str(duration)
        table.add_row(
            str(row["index"]),
            str(row["title"]),
            str(row["command"]),
            state_renderable,
            duration_text,
            str(row["memory"]),
            str(row["log_path"]),
        )
    return table
