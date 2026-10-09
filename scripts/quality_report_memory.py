#!/usr/bin/env python3
from __future__ import annotations

import ctypes
import os
import re
import subprocess
import sys
import time
from collections.abc import Callable
from dataclasses import dataclass
from pathlib import Path
from typing import cast

CARGO_BUILD_JOBS_ENV = "CARGO_BUILD_JOBS"
MEMINFO_PATH = Path("/proc/meminfo")
BYTES_PER_ADAPTIVE_CARGO_JOB = 3 * 1024 * 1024 * 1024
ADAPTIVE_CARGO_MEMORY_RESERVE = 6 * 1024 * 1024 * 1024
MEMORY_SAMPLE_INTERVAL_S = 1.0


@dataclass(frozen=True)
class CargoBuildJobsPolicy:
    source: str
    value: str
    detail: str


@dataclass(frozen=True)
class MemoryAvailability:
    bytes_available: int | None
    source: str


@dataclass(frozen=True)
class MemorySnapshot:
    current_available_bytes: int | None
    min_available_bytes: int | None
    source: str


@dataclass(frozen=True)
class MemoryTelemetry:
    start_available_bytes: int | None
    min_available_bytes: int | None
    end_available_bytes: int | None
    source: str

    @property
    def start_available(self) -> str | None:
        return format_available_memory(self.start_available_bytes)

    @property
    def min_available(self) -> str | None:
        return format_available_memory(self.min_available_bytes)

    @property
    def end_available(self) -> str | None:
        return format_available_memory(self.end_available_bytes)

    @property
    def log_line(self) -> str:
        return (
            "# memory: "
            f"start_available={self.start_available or 'unavailable'}, "
            f"min_available={self.min_available or 'unavailable'}, "
            f"end_available={self.end_available or 'unavailable'}, "
            f"source={self.source}"
        )


class MemoryTracker:
    def __init__(self, sample_interval_s: float = MEMORY_SAMPLE_INTERVAL_S) -> None:
        self._sample_interval_s = sample_interval_s
        self._started = self._read_memory()
        self._current = self._started
        self._min_available_bytes = self._started.bytes_available
        self._last_sample_at = time.monotonic()

    def snapshot(self) -> MemorySnapshot:
        self.sample()
        return MemorySnapshot(
            current_available_bytes=self._current.bytes_available,
            min_available_bytes=self._min_available_bytes,
            source=self._current.source,
        )

    def finish(self) -> MemoryTelemetry:
        self._record(self._read_memory())
        return MemoryTelemetry(
            start_available_bytes=self._started.bytes_available,
            min_available_bytes=self._min_available_bytes,
            end_available_bytes=self._current.bytes_available,
            source=self._current.source,
        )

    def sample(self) -> None:
        now = time.monotonic()
        if now - self._last_sample_at < self._sample_interval_s:
            return
        self._last_sample_at = now
        self._record(self._read_memory())

    def _record(self, availability: MemoryAvailability) -> None:
        self._current = availability
        if availability.bytes_available is None:
            return
        if self._min_available_bytes is None or availability.bytes_available < self._min_available_bytes:
            self._min_available_bytes = availability.bytes_available

    def _read_memory(self) -> MemoryAvailability:
        return available_memory()


def describe_cargo_build_jobs_policy() -> str:
    policy = resolve_cargo_build_jobs_policy(os.environ.copy())
    if policy.source == "configured":
        return f"Using configured {policy.detail}."
    return f"Adaptive Cargo build jobs enabled; initial {policy.detail}. Set CARGO_BUILD_JOBS to override."


def resolve_cargo_build_jobs_policy(env: dict[str, str]) -> CargoBuildJobsPolicy:
    configured_value = env.get(CARGO_BUILD_JOBS_ENV, "").strip()
    if configured_value:
        return CargoBuildJobsPolicy(
            source="configured",
            value=configured_value,
            detail=f"{CARGO_BUILD_JOBS_ENV}={configured_value} from environment",
        )

    memory = available_memory()
    mem_available_bytes = memory.bytes_available
    logical_cpus = os.cpu_count()
    adaptive_jobs = _adaptive_cargo_build_jobs(mem_available_bytes, logical_cpus)
    if mem_available_bytes is None:
        detail = (
            f"{CARGO_BUILD_JOBS_ENV}={adaptive_jobs} adaptive fallback; "
            "available memory unavailable, override by setting CARGO_BUILD_JOBS"
        )
    else:
        detail = (
            f"{CARGO_BUILD_JOBS_ENV}={adaptive_jobs} adaptive; "
            f"available_memory={format_gib(mem_available_bytes)} from {memory.source}, "
            f"logical_cpus={logical_cpus or 1}, "
            f"reserve={format_gib(ADAPTIVE_CARGO_MEMORY_RESERVE)}, "
            f"per_job={format_gib(BYTES_PER_ADAPTIVE_CARGO_JOB)}"
        )
    return CargoBuildJobsPolicy(source="adaptive", value=str(adaptive_jobs), detail=detail)


def format_memory_progress(snapshot: MemorySnapshot | None) -> str:
    if snapshot is None:
        return "-"
    current = format_available_memory(snapshot.current_available_bytes) or "unavailable"
    minimum = format_available_memory(snapshot.min_available_bytes) or "unavailable"
    return f"now {current} / min {minimum}"


def format_memory_minimum(snapshot: MemoryTelemetry | MemorySnapshot) -> str:
    min_available_bytes = snapshot.min_available_bytes
    return f"min {format_available_memory(min_available_bytes) or 'unavailable'}"


def format_available_memory(bytes_available: int | None) -> str | None:
    if bytes_available is None:
        return None
    return format_gib(bytes_available)


def format_gib(byte_count: int) -> str:
    return f"{byte_count / (1024 * 1024 * 1024):.1f} GiB"


def available_memory() -> MemoryAvailability:
    probes: list[Callable[[], MemoryAvailability]]
    if sys.platform.startswith("linux"):
        probes = [_available_memory_from_meminfo, _available_memory_from_sysconf]
    elif sys.platform == "darwin":
        probes = [_available_memory_from_vm_stat, _available_memory_from_sysconf]
    elif sys.platform == "win32":
        probes = [_available_memory_from_windows]
    else:
        probes = [_available_memory_from_sysconf]

    for probe in probes:
        availability = probe()
        if availability.bytes_available is not None:
            return availability
    return MemoryAvailability(bytes_available=None, source="unavailable")


def read_mem_total_gib() -> str | None:
    meminfo = Path("/proc/meminfo")
    if meminfo.exists():
        for line in meminfo.read_text(encoding="utf-8").splitlines():
            if line.startswith("MemTotal:"):
                parts = line.split()
                if len(parts) >= 2 and parts[1].isdigit():
                    gib = int(parts[1]) / 1024 / 1024
                    return f"{gib:.1f} GiB"
    if sys.platform == "darwin":
        return _read_darwin_mem_total_gib()
    return None


def _adaptive_cargo_build_jobs(mem_available_bytes: int | None, logical_cpus: int | None) -> int:
    cpu_jobs = max(1, logical_cpus or 1)
    if mem_available_bytes is None:
        return min(cpu_jobs, 4)

    memory_budget = mem_available_bytes - ADAPTIVE_CARGO_MEMORY_RESERVE
    memory_jobs = 1 if memory_budget <= 0 else max(1, memory_budget // BYTES_PER_ADAPTIVE_CARGO_JOB)
    return min(cpu_jobs, int(memory_jobs))


def _available_memory_from_meminfo() -> MemoryAvailability:
    mem_available_kib = _read_meminfo_kib().get("MemAvailable")
    if mem_available_kib is None:
        return MemoryAvailability(bytes_available=None, source="unavailable")
    return MemoryAvailability(bytes_available=mem_available_kib * 1024, source="MemAvailable")


def _available_memory_from_sysconf() -> MemoryAvailability:
    page_size = _sysconf_value("SC_PAGE_SIZE")
    available_pages = _sysconf_value("SC_AVPHYS_PAGES")
    if page_size is None or available_pages is None:
        return MemoryAvailability(bytes_available=None, source="unavailable")
    return MemoryAvailability(bytes_available=page_size * available_pages, source="sysconf SC_AVPHYS_PAGES")


def _available_memory_from_vm_stat() -> MemoryAvailability:
    try:
        output = subprocess.check_output(["vm_stat"], text=True, timeout=2)
    except (OSError, subprocess.SubprocessError):
        return MemoryAvailability(bytes_available=None, source="unavailable")

    page_size_match = re.search(r"page size of (\d+) bytes", output)
    if page_size_match is None:
        return MemoryAvailability(bytes_available=None, source="unavailable")
    page_size = int(page_size_match.group(1))
    reusable_pages = 0
    for line in output.splitlines():
        match = re.match(r"Pages (free|inactive|speculative):\s+([0-9]+)\.", line.strip())
        if match:
            reusable_pages += int(match.group(2))
    if reusable_pages <= 0:
        return MemoryAvailability(bytes_available=None, source="unavailable")
    return MemoryAvailability(bytes_available=page_size * reusable_pages, source="vm_stat reusable pages")


class _MemoryStatusEx(ctypes.Structure):
    _fields_ = [
        ("dwLength", ctypes.c_ulong),
        ("dwMemoryLoad", ctypes.c_ulong),
        ("ullTotalPhys", ctypes.c_ulonglong),
        ("ullAvailPhys", ctypes.c_ulonglong),
        ("ullTotalPageFile", ctypes.c_ulonglong),
        ("ullAvailPageFile", ctypes.c_ulonglong),
        ("ullTotalVirtual", ctypes.c_ulonglong),
        ("ullAvailVirtual", ctypes.c_ulonglong),
        ("ullAvailExtendedVirtual", ctypes.c_ulonglong),
    ]


def _available_memory_from_windows() -> MemoryAvailability:
    if sys.platform != "win32":
        return MemoryAvailability(bytes_available=None, source="unavailable")

    status = _MemoryStatusEx()
    status.dwLength = ctypes.sizeof(_MemoryStatusEx)
    try:
        global_memory_status_ex = ctypes.WinDLL("kernel32", use_last_error=True).GlobalMemoryStatusEx  # type: ignore[attr-defined]
        global_memory_status_ex.argtypes = [ctypes.POINTER(_MemoryStatusEx)]
        global_memory_status_ex.restype = ctypes.c_int
        ok = global_memory_status_ex(ctypes.byref(status))
    except (AttributeError, OSError):
        return MemoryAvailability(bytes_available=None, source="unavailable")
    if not ok:
        return MemoryAvailability(bytes_available=None, source="unavailable")
    return MemoryAvailability(bytes_available=int(status.ullAvailPhys), source="GlobalMemoryStatusEx ullAvailPhys")


def _read_darwin_mem_total_gib() -> str | None:
    try:
        output = subprocess.check_output(["sysctl", "-n", "hw.memsize"], text=True, timeout=2).strip()
    except (OSError, subprocess.SubprocessError):
        return None
    if not output.isdigit():
        return None
    return format_gib(int(output))


def _read_meminfo_kib(meminfo_path: Path = MEMINFO_PATH) -> dict[str, int]:
    try:
        lines = meminfo_path.read_text(encoding="utf-8").splitlines()
    except OSError:
        return {}

    values: dict[str, int] = {}
    for line in lines:
        key, separator, raw_value = line.partition(":")
        if not separator:
            continue
        parts = raw_value.strip().split()
        if not parts:
            continue
        try:
            values[key] = int(parts[0])
        except ValueError:
            continue
    return values


def _sysconf_value(name: str) -> int | None:
    sysconf = cast(Callable[[str], int] | None, getattr(os, "sysconf", None))
    if sysconf is None:
        return None
    try:
        value = sysconf(name)
    except (OSError, ValueError):
        return None
    if value <= 0:
        return None
    return value
