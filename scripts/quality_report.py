#!/usr/bin/env python3
from __future__ import annotations

import sys
from datetime import UTC, datetime
from pathlib import Path

from rich.live import Live
from rich.panel import Panel

from quality_report_memory import (
    MemorySnapshot,
    describe_cargo_build_jobs_policy,
    format_memory_minimum,
    format_memory_progress,
)
from quality_report_rendering import render_progress_table, status_display
from quality_report_support import (
    CONSOLE,
    EXAMPLE_TIMEOUT_S,
    OUTPUT_ROOT,
    CategorySummary,
    CheckResult,
    CheckSpec,
    CommandSpec,
    EnvironmentInfo,
    ProgressRow,
    ReportContext,
    command_summary,
    gather_host_specs,
    load_examples_summary,
    load_frontend_perf_summary,
    load_perf_summary,
    parse_license_summary,
    read_text_output,
    render_markdown,
    run_command_group,
    tail_text,
    write_json,
)


def build_context(output_root: Path, timeout_s: int, timestamp: str) -> ReportContext:
    bundle_dir = output_root / timestamp
    logs_dir = bundle_dir / "logs"
    artifacts_dir = bundle_dir / "artifacts"
    logs_dir.mkdir(parents=True, exist_ok=True)
    artifacts_dir.mkdir(parents=True, exist_ok=True)

    examples_validate_path = artifacts_dir / "examples-validate.json"
    examples_check_path = artifacts_dir / "examples-check.json"

    check_specs = [
        CheckSpec("build", "Build", [CommandSpec(["just", "quality-build"])]),
        CheckSpec("lint", "Lint", [CommandSpec(["just", "quality-lint"])]),
        CheckSpec("build_docker", "Docker Image Smoke", [CommandSpec(["just", "quality-docker"])]),
        CheckSpec("tests_rust_unit", "Rust Unit Tests", [CommandSpec(["just", "quality-test-rust-unit"])]),
        CheckSpec(
            "tests_rust_integration",
            "Rust Integration Tests",
            [CommandSpec(["just", "quality-test-rust-integration"])],
        ),
        CheckSpec("tests_frontend", "Frontend Tests", [CommandSpec(["just", "quality-test-frontend"])]),
        CheckSpec("tests_smoke", "Smoke / E2E Tests", [CommandSpec(["just", "quality-smoke"])]),
        CheckSpec(
            "tests_smoke_dev_stack",
            "Dev Stack Smoke Test",
            [CommandSpec(["just", "quality-smoke-dev-stack"])],
        ),
        CheckSpec("docs_checks", "Docs Checks", [CommandSpec(["just", "quality-docs"])]),
        CheckSpec(
            "examples_validate",
            "Examples Validation",
            [
                CommandSpec(
                    ["just", "quality-examples-validate"],
                    env={"CZAR_QUALITY_EXAMPLES_VALIDATE_RESULTS": str(examples_validate_path)},
                )
            ],
        ),
        CheckSpec(
            "examples_system",
            "Examples System Tests",
            [
                CommandSpec(
                    ["just", "quality-examples-system"],
                    env={
                        "CZAR_QUALITY_EXAMPLES_TIMEOUT": str(timeout_s),
                        "CZAR_QUALITY_EXAMPLES_RESULTS": str(examples_check_path),
                    },
                )
            ],
        ),
        CheckSpec("deps_licenses", "Dependency Licenses", [CommandSpec(["just", "licenses"])]),
        CheckSpec("deps_freshness", "Dependency Freshness", [CommandSpec(["just", "freshness"])]),
        CheckSpec("deps_vulns", "Dependency Vulnerabilities", [CommandSpec(["just", "vulns"])]),
        CheckSpec("deps_outdated", "Dependency Outdated Report", [CommandSpec(["just", "outdated"])], critical=False),
        CheckSpec(
            "benchmark_summary",
            "Benchmark Baseline Summary",
            [CommandSpec(["just", "benchmark-summary"])],
            critical=False,
        ),
        CheckSpec(
            "perf_frontend",
            "Frontend Performance (Lighthouse)",
            [CommandSpec(["just", "perf-frontend"])],
            critical=False,
        ),
    ]

    CONSOLE.print(
        Panel.fit(
            f"[bold]Writing quality report to[/]\n{bundle_dir}",
            title="Quality Report",
            border_style="cyan",
        )
    )
    CONSOLE.print("[bold cyan]Running quality report...[/]")
    CONSOLE.print(f"[dim]{describe_cargo_build_jobs_policy()}[/]")

    progress_rows: list[ProgressRow] = [
        ProgressRow(
            index=index,
            title=spec.title,
            command=command_summary(spec.commands),
            status="PENDING",
            duration="-",
            memory="-",
            log_path=f"logs/{spec.key}.log",
        )
        for index, spec in enumerate(check_specs, start=1)
    ]
    checks: list[CheckResult] = []
    total_checks = len(check_specs)
    with Live(render_progress_table(progress_rows), console=CONSOLE, refresh_per_second=8) as live:
        for index, spec in enumerate(check_specs, start=1):
            progress_rows[index - 1]["status"] = "RUNNING"
            progress_rows[index - 1]["duration"] = 0.0
            progress_rows[index - 1]["memory"] = "sampling..."
            live.update(render_progress_table(progress_rows))

            row_index = index - 1

            def update_progress(
                elapsed_s: float,
                memory: MemorySnapshot,
                *,
                current_row_index: int = row_index,
            ) -> None:
                progress_rows[current_row_index]["duration"] = elapsed_s
                progress_rows[current_row_index]["memory"] = format_memory_progress(memory)
                live.update(render_progress_table(progress_rows))

            check = run_command_group(spec, logs_dir, on_progress=update_progress)
            check["display_status"] = status_display(check["status"])
            checks.append(check)
            progress_rows[index - 1]["status"] = check["status"]
            progress_rows[index - 1]["duration"] = float(check["duration_s"])
            progress_rows[index - 1]["memory"] = format_memory_minimum(
                MemorySnapshot(
                    current_available_bytes=check["memory_end_available_bytes"],
                    min_available_bytes=check["memory_min_available_bytes"],
                    source=check["memory_source"],
                )
            )
            progress_rows[index - 1]["log_path"] = check["log_path"]
            live.update(render_progress_table(progress_rows))
            if check["status"] == "FAIL":
                CONSOLE.print(
                    f"[red]Failure detected in step {index}/{total_checks}. "
                    f"See {check['log_path']} for full output.[/]"
                )

    checks_by_key = {check["key"]: check for check in checks}
    write_json(artifacts_dir / "checks.json", checks)

    environment = EnvironmentInfo(
        generated_at=datetime.now(UTC).strftime("%Y-%m-%d %H:%M:%SZ"),
        git_branch=read_text_output(["git", "rev-parse", "--abbrev-ref", "HEAD"]),
        git_commit=read_text_output(["git", "rev-parse", "--short", "HEAD"]),
        git_status=read_text_output(["git", "status", "--short"]).splitlines(),
        rustc_vv=read_text_output(["rustc", "-vV"]),
        cargo_version=read_text_output(["cargo", "--version"]),
        python_version=read_text_output(["python3", "--version"]),
        uv_version=read_text_output(["uv", "--version"]),
        pnpm_version=read_text_output(["pnpm", "--version"]),
        just_version=read_text_output(["just", "--version"]),
        rust_targets=read_text_output(["rustup", "target", "list", "--installed"]).splitlines(),
        host_specs=gather_host_specs(),
        host_target="unknown",
        rust_target_count=0,
    )
    environment["host_target"] = next(
        (line.split(": ", 1)[1] for line in environment["rustc_vv"].splitlines() if line.startswith("host: ")),
        "unknown",
    )
    environment["rust_target_count"] = len(environment["rust_targets"])
    write_json(artifacts_dir / "environment.json", environment)

    examples = load_examples_summary(artifacts_dir / "examples-check.json")
    write_json(artifacts_dir / "examples-summary.json", examples)

    perf = load_perf_summary()
    write_json(artifacts_dir / "perf-summary.json", perf)

    frontend_perf = load_frontend_perf_summary()
    write_json(artifacts_dir / "frontend-perf-summary.json", frontend_perf)

    licenses_log = (logs_dir / "deps_licenses.log").read_text(encoding="utf-8").strip().splitlines()
    license_summary = parse_license_summary(licenses_log[-1] if licenses_log else "")

    overall_status = "FAIL" if any(check["critical"] and check["exit_code"] != 0 for check in checks) else "PASS"
    categories: list[CategorySummary] = [
        CategorySummary(
            title="Environment",
            status="INFO",
            display_status=status_display("INFO"),
            detail="environment.md",
        ),
        CategorySummary(
            title="Build and lint",
            status=(
                "FAIL"
                if any(checks_by_key[key]["exit_code"] != 0 for key in ("build", "lint", "build_docker"))
                else "PASS"
            ),
            display_status="",
            detail="build.md",
        ),
        CategorySummary(
            title="Tests",
            status=(
                "FAIL"
                if any(
                    checks_by_key[key]["exit_code"] != 0
                    for key in (
                        "tests_rust_unit",
                        "tests_rust_integration",
                        "tests_frontend",
                        "tests_smoke",
                        "tests_smoke_dev_stack",
                    )
                )
                else "PASS"
            ),
            display_status="",
            detail="tests.md",
        ),
        CategorySummary(
            title="Docs",
            status=(
                "FAIL"
                if any(checks_by_key[key]["exit_code"] != 0 for key in ("docs_checks", "examples_validate"))
                else "PASS"
            ),
            display_status="",
            detail="docs.md",
        ),
        CategorySummary(
            title="Examples",
            status=checks_by_key["examples_system"]["status"],
            display_status="",
            detail="examples.md",
        ),
        CategorySummary(
            title="Dependencies",
            status=(
                "FAIL"
                if any(
                    checks_by_key[key]["exit_code"] != 0
                    for key in ("deps_licenses", "deps_freshness", "deps_vulns")
                )
                else "PASS"
            ),
            display_status="",
            detail="dependencies.md",
        ),
        CategorySummary(
            title="Benchmarks",
            status=checks_by_key["benchmark_summary"]["status"],
            display_status=checks_by_key["benchmark_summary"]["display_status"],
            detail="benchmark.md",
        ),
        CategorySummary(
            title="Frontend Performance",
            status=checks_by_key["perf_frontend"]["status"],
            display_status=checks_by_key["perf_frontend"]["display_status"],
            detail="frontend_perf.md",
        ),
    ]
    for category in categories:
        if not category["display_status"]:
            category["display_status"] = status_display(category["status"])

    context = ReportContext(
        bundle_dir=bundle_dir,
        output_root=output_root,
        timestamp=timestamp,
        timeout_s=timeout_s,
        checks=checks,
        checks_by_key=checks_by_key,
        environment=environment,
        examples=examples,
        perf=perf,
        frontend_perf=frontend_perf,
        license_summary=license_summary,
        overall_status=overall_status,
        overall_status_display=status_display(overall_status),
        categories=categories,
        generated_at=environment["generated_at"],
        log_tail={check["key"]: tail_text(logs_dir / f"{check['key']}.log") for check in checks},
    )
    write_json(
        artifacts_dir / "report-context.json",
        {
            "overall_status": overall_status,
            "checks": checks,
            "environment": environment,
            "examples": examples,
            "perf": perf,
            "license_summary": license_summary,
        },
    )
    return context


def main() -> int:
    if len(sys.argv) != 1:
        print(
            "quality_report.py does not accept command-line overrides; use repo policy defaults",
            file=sys.stderr,
        )
        return 2

    planned_timestamp = datetime.now(UTC).strftime("%Y%m%dT%H%M%SZ")
    context = build_context(OUTPUT_ROOT, EXAMPLE_TIMEOUT_S, planned_timestamp)
    bundle_dir = context["bundle_dir"]
    render_markdown("overview.md.j2", bundle_dir / "overview.md", context)
    render_markdown("environment.md.j2", bundle_dir / "environment.md", context)
    render_markdown("build.md.j2", bundle_dir / "build.md", context)
    render_markdown("tests.md.j2", bundle_dir / "tests.md", context)
    render_markdown("docs.md.j2", bundle_dir / "docs.md", context)
    render_markdown("examples.md.j2", bundle_dir / "examples.md", context)
    render_markdown("dependencies.md.j2", bundle_dir / "dependencies.md", context)
    render_markdown("benchmark.md.j2", bundle_dir / "benchmark.md", context)
    render_markdown("frontend_perf.md.j2", bundle_dir / "frontend_perf.md", context)

    latest_path = OUTPUT_ROOT / "latest"
    if latest_path.exists() or latest_path.is_symlink():
        latest_path.unlink()
    latest_path.symlink_to(bundle_dir.name)

    print(f"Overview: {bundle_dir / 'overview.md'}")
    print(f"Timestamped bundle: {bundle_dir}")
    print(f"Latest symlink: {latest_path} -> {bundle_dir}")
    return 1 if context["overall_status"] == "FAIL" else 0


if __name__ == "__main__":
    raise SystemExit(main())
