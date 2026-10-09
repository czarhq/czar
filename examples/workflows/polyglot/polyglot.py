"""Python tasks for polyglot workflow example."""
from typing import TypedDict


class NumbersOutput(TypedDict):
    numbers: list[int]


class RustLibraryOutput(TypedDict):
    tripled: list[int]
    sum: int


class RustProcessSdkOutput(TypedDict):
    total: int
    tripled: list[int]


class RustProcessMacroOutput(TypedDict):
    total: int
    tripled: list[int]
    task_name: str


class RustProcessRawOutput(TypedDict):
    sum: int
    count: int


class RustWasmOutput(TypedDict):
    max: int
    count: int
    squared: list[int]


class RustWasmCliOutput(TypedDict):
    min: int
    count: int
    even_numbers: list[int]


class FormatOutput(TypedDict):
    summary: str
    total: int
    macro_total: int
    macro_task_name: str
    final_value: int
    tripled_numbers: list[int]
    raw_sum: int
    raw_count: int
    embedded_wasm_max: int
    embedded_wasm_count: int
    cli_wasm_min: int
    cli_wasm_even_numbers: list[int]
def generate_numbers(count: int) -> NumbersOutput:
    """Generate a list of numbers (Python task 1)."""
    print(f"[Python] Generating {count} numbers...")
    numbers = list(range(1, count + 1))
    print(f"[Python] Generated: {numbers}")
    return {"numbers": numbers}


def format_result(
    multiplier: int,
    rust_process_sdk: RustProcessSdkOutput,
    rust_process_macro: RustProcessMacroOutput,
    rust_process_raw: RustProcessRawOutput,
    rust_wasm_embedded: RustWasmOutput,
    rust_wasm_cli: RustWasmCliOutput,
) -> FormatOutput:
    """Format the final result (Python task 3)."""
    print(f"[Python] Formatting result with multiplier {multiplier}...")
    print(f"[Python] Received rust_process_sdk data: {rust_process_sdk}")
    print(f"[Python] Received rust_process_macro data: {rust_process_macro}")
    print(f"[Python] Received rust_process_raw data: {rust_process_raw}")
    print(f"[Python] Received rust_wasm_embedded data: {rust_wasm_embedded}")
    print(f"[Python] Received rust_wasm_cli data: {rust_wasm_cli}")

    total = rust_process_sdk.get("total", 0)
    tripled = rust_process_sdk.get("tripled", [])
    macro_total = rust_process_macro.get("total", 0)
    macro_task_name = rust_process_macro.get("task_name", "unknown")
    raw_sum = rust_process_raw.get("sum", 0)
    raw_count = rust_process_raw.get("count", 0)
    embedded_wasm_max = rust_wasm_embedded.get("max", 0)
    embedded_wasm_count = rust_wasm_embedded.get("count", 0)
    cli_wasm_min = rust_wasm_cli.get("min", 0)
    cli_wasm_even_numbers = rust_wasm_cli.get("even_numbers", [])

    result: FormatOutput = {
        "summary": (
            f"Processed {len(tripled)} numbers; macro task {macro_task_name}; embedded wasm max {embedded_wasm_max}, "
            f"CLI wasm min {cli_wasm_min}"
        ),
        "total": total,
        "macro_total": macro_total,
        "macro_task_name": macro_task_name,
        "final_value": total * multiplier,
        "tripled_numbers": tripled,
        "raw_sum": raw_sum,
        "raw_count": raw_count,
        "embedded_wasm_max": embedded_wasm_max,
        "embedded_wasm_count": embedded_wasm_count,
        "cli_wasm_min": cli_wasm_min,
        "cli_wasm_even_numbers": cli_wasm_even_numbers,
    }

    print(f"[Python] Final result: {result}")
    return result
