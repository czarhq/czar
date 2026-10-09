"""Progress demo workflow.

Demonstrates in-task progress visualization using ``czar_progress``.
The ``process_items`` task runs a loop and reports incremental progress
so the Czar UI can display a live progress bar without splitting each
element into a separate task. The ``process_items_tqdm`` task demonstrates
that regular ``tqdm`` usage also updates Czar progress automatically.
"""

import time
from typing import TypedDict

from czar import czar_progress

try:
    from tqdm import tqdm
except ImportError:
    tqdm = None


class ProcessResult(TypedDict):
    processed: int
    total: int
    summary: str


class SummaryResult(TypedDict):
    message: str


def process_items(item_count: int = 20, step_delay: float = 0.3) -> ProcessResult:
    """Process a batch of items with live progress reporting.

    Each iteration emits an ``CZAR_PROGRESS:`` line that the Czar scheduler
    intercepts and stores in ``tasks.progress``.  The UI polls for updates
    and renders a progress bar without any changes to the workflow definition.
    """
    total = item_count
    for i in range(total):
        # Simulate work
        time.sleep(step_delay)

        completed = i + 1
        pct = int(completed / total * 100)
        czar_progress(completed, total, f"item {completed}/{total} ({pct}%)")
        print(f"processed item {completed}/{total}")

    summary = f"Finished processing {total} items"
    print(summary)
    return {"processed": total, "total": total, "summary": summary}


def process_items_tqdm(item_count: int = 12, step_delay: float = 0.25) -> ProcessResult:
    """Process items using a standard tqdm loop.

    No Czar-specific calls are required in this function. The runtime bridge
    captures tqdm updates and forwards them as CZAR_PROGRESS events. If
    ``tqdm`` is unavailable, the task falls back to explicit ``czar_progress``
    updates so the example still runs.
    """
    total = item_count
    if tqdm is None:
        for i in range(total):
            time.sleep(step_delay)
            completed = i + 1
            czar_progress(completed, total, f"item {completed}/{total}")
            print(f"processed item {completed}/{total}")
    else:
        for _ in tqdm(range(total), desc="tqdm items", unit="item"):
            time.sleep(step_delay)

    summary = f"Finished tqdm processing of {total} items"
    print(summary)
    return {"processed": total, "total": total, "summary": summary}


def summarize(result: ProcessResult) -> SummaryResult:
    """Summarize the processing result."""
    msg = f"{result['summary']} — all {result['total']} items handled successfully"
    print(msg)
    return {"message": msg}
