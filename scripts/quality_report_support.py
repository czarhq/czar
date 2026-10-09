#!/usr/bin/env python3
from __future__ import annotations

import json
import os
import platform
import re
import shlex
import socket
import subprocess
import time
from collections import Counter
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import TypedDict, cast

from jinja2 import Environment, FileSystemLoader
from rich.console import Console

from quality_report_memory import (
    CARGO_BUILD_JOBS_ENV,
    MemorySnapshot,
    MemoryTracker,
    format_memory_progress,
    read_mem_total_gib,
    resolve_cargo_build_jobs_policy,
)
from quality_report_rendering import status_display

ROOT = Path(__file__).resolve().parent.parent
TEMPLATE_DIR = ROOT / "scripts" / "templates" / "quality_report"
OUTPUT_ROOT = ROOT / ".czar" / "reports" / "quality"
EXAMPLE_TIMEOUT_S = 240
CONSOLE = Console()

@dataclass
class CommandSpec:
    argv: list[str]
    cwd: Path | None = None
    env: dict[str, str] | None = None

    @property
    def display(self) -> str:
        return shlex.join(self.argv)


@dataclass
class CheckSpec:
    key: str
    title: str
    commands: list[CommandSpec]
    critical: bool = True


class CheckResult(TypedDict):
    key: str
    title: str
    status: str
    exit_code: int
    duration_s: int
    critical: bool
    log_path: str
    commands: list[str]
    display_status: str
    memory_start_available: str | None
    memory_min_available: str | None
    memory_end_available: str | None
    memory_start_available_bytes: int | None
    memory_min_available_bytes: int | None
    memory_end_available_bytes: int | None
    memory_source: str


class ProgressRow(TypedDict):
    index: int
    title: str
    command: str
    status: str
    duration: str | float
    memory: str
    log_path: str


class HostSpecs(TypedDict):
    hostname: str
    os: str
    kernel: str
    platform: str
    architecture: str
    cpu_model: str
    logical_cpus: int | None
    physical_cores: int | None
    memory_total: str | None


class EnvironmentInfo(TypedDict):
    generated_at: str
    git_branch: str
    git_commit: str
    git_status: list[str]
    rustc_vv: str
    cargo_version: str
    python_version: str
    uv_version: str
    pnpm_version: str
    just_version: str
    rust_targets: list[str]
    host_specs: HostSpecs
    host_target: str
    rust_target_count: int


class ExampleSummaryRow(TypedDict):
    example: str
    status: str
    exit_code: int
    duration_s: int
    kind: str
    policy: str
    display_status: str


class ExamplesSummary(TypedDict):
    rows: list[ExampleSummaryRow]
    total: int
    status_counts: dict[str, int]
    kind_counts: dict[str, int]
    policy_counts: dict[str, int]
    slowest: list[ExampleSummaryRow]


class FrontendPerfMetrics(TypedDict):
    fcp_ms: int
    lcp_ms: int
    tbt_ms: int
    cls: float
    tti_ms: int
    speed_index_ms: int


class FrontendPerfSummary(TypedDict):
    status: str
    timestamp: str | None
    url: str | None
    scores: dict[str, int]
    metrics: FrontendPerfMetrics | None
    source_path: str


class PerfRow(TypedDict):
    date: str
    commit: str
    config: str
    avg_latency: str
    throughput: str
    rss_kb: str
    cpu_avg: str
    cpu_peak: str
    peak_pending: str
    peak_dispatched: str
    peak_running: str
    status_updates: str
    note: str


class PerfSummary(TypedDict):
    status: str
    rows: list[PerfRow]
    latest_date: str | None
    latest_commit: str | None
    source_path: str


class LicenseSummary(TypedDict, total=False):
    raw: str
    python_projects: str
    python_packages: str
    js_projects: str
    js_packages: str
    js_skipped: str


class CategorySummary(TypedDict):
    title: str
    status: str
    display_status: str
    detail: str


class ReportContext(TypedDict):
    bundle_dir: Path
    output_root: Path
    timestamp: str
    timeout_s: int
    checks: list[CheckResult]
    checks_by_key: dict[str, CheckResult]
    environment: EnvironmentInfo
    examples: ExamplesSummary
    perf: PerfSummary
    frontend_perf: FrontendPerfSummary
    license_summary: LicenseSummary
    overall_status: str
    overall_status_display: str
    categories: list[CategorySummary]
    generated_at: str
    log_tail: dict[str, str]


def write_json(destination: Path, payload: object) -> None:
    destination.write_text(
        json.dumps(payload, separators=(",", ":"), ensure_ascii=True),
        encoding="utf-8",
    )


def run_command_group(
    spec: CheckSpec,
    logs_dir: Path,
    on_progress: Callable[[float, MemorySnapshot], None] | None = None,
) -> CheckResult:
    log_path = logs_dir / f"{spec.key}.log"
    started = time.time()
    exit_code = 0
    memory_tracker = MemoryTracker()
    with log_path.open("w", encoding="utf-8") as handle:
        for command in spec.commands:
            env = os.environ.copy()
            if command.env:
                env.update(command.env)
            cargo_jobs_policy = resolve_cargo_build_jobs_policy(env)
            if cargo_jobs_policy.source == "adaptive":
                env[CARGO_BUILD_JOBS_ENV] = cargo_jobs_policy.value
            handle.write(f"# {cargo_jobs_policy.detail}\n")
            handle.write(f"# memory start: {format_memory_progress(memory_tracker.snapshot())}\n")
            handle.write(f"$ {command.display}\n")
            handle.flush()
            process = subprocess.Popen(
                command.argv,
                cwd=command.cwd or ROOT,
                text=True,
                stdout=handle,
                stderr=subprocess.STDOUT,
                env=env,
            )
            while True:
                rc = process.poll()
                if on_progress is not None:
                    on_progress(time.time() - started, memory_tracker.snapshot())
                if rc is not None:
                    exit_code = rc
                    break
                time.sleep(0.1)
            handle.write("\n")
            handle.flush()
            if exit_code != 0:
                break
    duration_s = int(time.time() - started)
    memory_telemetry = memory_tracker.finish()
    with log_path.open("a", encoding="utf-8") as handle:
        handle.write(f"{memory_telemetry.log_line}\n")
    return CheckResult(
        key=spec.key,
        title=spec.title,
        status="PASS" if exit_code == 0 else "FAIL",
        exit_code=exit_code,
        duration_s=duration_s,
        critical=spec.critical,
        log_path=f"logs/{spec.key}.log",
        commands=[command.display for command in spec.commands],
        display_status="",
        memory_start_available=memory_telemetry.start_available,
        memory_min_available=memory_telemetry.min_available,
        memory_end_available=memory_telemetry.end_available,
        memory_start_available_bytes=memory_telemetry.start_available_bytes,
        memory_min_available_bytes=memory_telemetry.min_available_bytes,
        memory_end_available_bytes=memory_telemetry.end_available_bytes,
        memory_source=memory_telemetry.source,
    )


def render_markdown(template_name: str, destination: Path, context: ReportContext) -> None:
    env = Environment(
        loader=FileSystemLoader(str(TEMPLATE_DIR)),
        autoescape=False,
        trim_blocks=True,
        lstrip_blocks=True,
    )
    template = env.get_template(template_name)
    destination.write_text(template.render(**context), encoding="utf-8")


def read_text_output(argv: list[str]) -> str:
    return subprocess.check_output(argv, cwd=ROOT, text=True).strip()


def command_summary(commands: list[CommandSpec]) -> str:
    if not commands:
        return ""
    summary = commands[0].display
    extra = len(commands) - 1
    if extra > 0:
        summary = f"{summary} (+{extra} more)"
    return summary

def read_lscpu_field(name: str) -> str | None:
    try:
        output = subprocess.check_output(["lscpu"], cwd=ROOT, text=True)
    except (FileNotFoundError, subprocess.CalledProcessError):
        return None
    prefix = f"{name}:"
    for line in output.splitlines():
        if line.startswith(prefix):
            return line.split(":", 1)[1].strip()
    return None


def gather_host_specs() -> HostSpecs:
    cpu_model = (
        read_lscpu_field("Model name")
        or read_lscpu_field("Architecture")
        or platform.processor()
        or platform.machine()
    )
    physical_cores = None
    cores_per_socket = read_lscpu_field("Core(s) per socket")
    sockets = read_lscpu_field("Socket(s)")
    if cores_per_socket and sockets and cores_per_socket.isdigit() and sockets.isdigit():
        physical_cores = int(cores_per_socket) * int(sockets)
    return HostSpecs(
        hostname=socket.gethostname(),
        os=platform.system(),
        kernel=platform.release(),
        platform=platform.platform(),
        architecture=platform.machine(),
        cpu_model=cpu_model,
        logical_cpus=os.cpu_count(),
        physical_cores=physical_cores,
        memory_total=read_mem_total_gib(),
    )


def load_examples_summary(results_path: Path) -> ExamplesSummary:
    rows = (
        cast(list[ExampleSummaryRow], json.loads(results_path.read_text(encoding="utf-8")))
        if results_path.exists()
        else []
    )
    for row in rows:
        row["display_status"] = status_display(row["status"])
    status_counts = Counter(row["status"] for row in rows)
    kind_counts = Counter(row["kind"] for row in rows)
    policy_counts = Counter(row["policy"] for row in rows)
    slowest = sorted(rows, key=lambda row: row["duration_s"], reverse=True)[:5]
    return ExamplesSummary(
        rows=rows,
        total=len(rows),
        status_counts=dict(status_counts),
        kind_counts=dict(kind_counts),
        policy_counts=dict(policy_counts),
        slowest=slowest,
    )


def load_perf_summary() -> PerfSummary:
    perf_file = ROOT / "docs" / "reports" / "perf" / "results.md"
    if not perf_file.exists():
        return PerfSummary(
            status="missing",
            rows=[],
            latest_date=None,
            latest_commit=None,
            source_path="docs/reports/perf/results.md",
        )
    rows: list[PerfRow] = []
    for line in perf_file.read_text(encoding="utf-8").splitlines():
        if not line.startswith("| ") or "Date" in line or "---" in line:
            continue
        cols = [part.strip() for part in line.strip("|").split("|")]
        if len(cols) < 13:
            continue
        rows.append(
            PerfRow(
                date=cols[0],
                commit=cols[1],
                config=cols[2],
                avg_latency=cols[3],
                throughput=cols[4],
                rss_kb=cols[5],
                cpu_avg=cols[6],
                cpu_peak=cols[7],
                peak_pending=cols[8],
                peak_dispatched=cols[9],
                peak_running=cols[10],
                status_updates=cols[11],
                note=cols[12],
            )
        )
    return PerfSummary(
        status="available" if rows else "empty",
        latest_date=rows[0]["date"] if rows else None,
        latest_commit=rows[0]["commit"] if rows else None,
        rows=rows[:5],
        source_path="docs/reports/perf/results.md",
    )


def load_frontend_perf_summary() -> FrontendPerfSummary:
    source = ROOT / ".czar" / "reports" / "frontend-perf" / "latest.json"
    if not source.exists():
        return FrontendPerfSummary(
            status="missing",
            timestamp=None,
            url=None,
            scores={},
            metrics=None,
            source_path=str(source.relative_to(ROOT)),
        )
    try:
        data = json.loads(source.read_text(encoding="utf-8"))
        return FrontendPerfSummary(
            status="available",
            timestamp=data.get("timestamp"),
            url=data.get("url"),
            scores=data.get("scores", {}),
            metrics=cast(FrontendPerfMetrics, data.get("metrics")),
            source_path=str(source.relative_to(ROOT)),
        )
    except Exception:
        return FrontendPerfSummary(
            status="error",
            timestamp=None,
            url=None,
            scores={},
            metrics=None,
            source_path=str(source.relative_to(ROOT)),
        )


def parse_license_summary(summary_line: str) -> LicenseSummary:
    pattern = re.compile(
        r"licenses ok: rust workspace; python (?P<python_projects>\d+) project\(s\), "
        r"(?P<python_packages>\d+) package\(s\); js (?P<js_projects>\d+) production project\(s\), "
        r"(?P<js_packages>\d+) package\(s\), (?P<js_skipped>\d+) skipped"
    )
    match = pattern.search(summary_line.strip())
    if not match:
        return LicenseSummary(raw=summary_line.strip())
    data = LicenseSummary(**match.groupdict())
    data["raw"] = summary_line.strip()
    return data


def tail_text(path: Path, lines: int = 60) -> str:
    if not path.exists():
        return ""
    content = path.read_text(encoding="utf-8").splitlines()
    return "\n".join(content[-lines:])
