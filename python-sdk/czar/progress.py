"""In-task progress reporting for Czar.

Tasks can call :func:`czar_progress` in a loop to surface live progress in the
Czar UI without splitting each element into a separate task.

The function prints a structured ``CZAR_PROGRESS:`` line to stdout which the
process executor detects, strips from the user-visible log stream, and forwards
to the scheduler as a structured progress update.  The scheduler stores the
latest snapshot on the ``tasks.progress`` column, which is returned by the
``/api/runs/{id}`` endpoint as ``progress: {current, total, message}``.

Example::

    from czar import czar_progress

    def process_items(items: list) -> dict:
        total = len(items)
        for i, item in enumerate(items):
            do_work(item)
            czar_progress(i + 1, total, f"processed {item!r}")
        return {"processed": total}
"""

import json
import sys


def czar_progress(current: int, total: int, message: str = "") -> None:
    """Report in-task progress to the Czar orchestrator.

    Prints an ``CZAR_PROGRESS:`` line to stdout that the process executor
    intercepts.  The line is **not** included in the task's log output; it is
    converted to a structured progress update and stored in the database so the
    UI can display a live progress bar.

    Args:
        current: Number of items/steps completed so far.
        total:   Total number of items/steps in the task.
        message: Optional human-readable description of the current step.
    """
    payload: dict = {"current": current, "total": total}
    if message:
        payload["message"] = message
    print(f"CZAR_PROGRESS:{json.dumps(payload)}", flush=True)
