#!/usr/bin/env python3
from __future__ import annotations

from pathlib import Path
from typing import TypedDict

ROOT = Path(__file__).resolve().parent.parent
REPORT = ROOT / "docs" / "reports" / "perf" / "results.md"


class BenchmarkRow(TypedDict):
    date: str
    commit: str
    config: str
    avg_latency: str
    throughput: str


def parse_rows() -> list[BenchmarkRow]:
    if not REPORT.exists():
        return []
    rows: list[BenchmarkRow] = []
    for line in REPORT.read_text(encoding="utf-8").splitlines():
        if not line.startswith("| ") or "Date" in line or "---" in line:
            continue
        cols = [part.strip() for part in line.strip("|").split("|")]
        if len(cols) < 13:
            continue
        rows.append(
            {
                "date": cols[0],
                "commit": cols[1],
                "config": cols[2],
                "avg_latency": cols[3],
                "throughput": cols[4],
            }
        )
    return rows


def main() -> int:
    rows = parse_rows()
    if not rows:
        print(f"no benchmark baseline found in {REPORT}")
        return 1
    latest = rows[0]
    print(
        "benchmark baseline ok: "
        f"{len(rows)} row(s); latest={latest['date']} {latest['config']} commit={latest['commit']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
