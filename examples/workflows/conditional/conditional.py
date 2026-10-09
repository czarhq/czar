"""
Conditional execution example.

Demonstrates the `when:` DSL in Czar workflows. The workflow checks a dataset's
quality score and routes processing through exactly one of two branches:
  - process_high_quality  (when score > 0.8)
  - process_low_quality   (when score <= 0.8)

The `notify` task always runs regardless of which branch was taken.
"""
from typing import TypedDict


# ── Output types ──────────────────────────────────────────────────────────────

class QualityResult(TypedDict):
    score: float
    category: str
    item_count: int


class ProcessResult(TypedDict):
    processed_count: int
    route: str


# ── Tasks ─────────────────────────────────────────────────────────────────────

def check_quality(dataset: str) -> QualityResult:
    """Assess dataset quality and return a score between 0 and 1."""
    print(f"[check_quality] Assessing dataset: {dataset!r}")

    # Simulated quality assessment — swap these to test both branches:
    score = 0.92   # try 0.5 to exercise the low-quality branch
    category = "high" if score > 0.8 else "low"
    item_count = 1_000

    result: QualityResult = {
        "score": score,
        "category": category,
        "item_count": item_count,
    }
    print(f"[check_quality] score={score:.2f}, category={category!r}")
    return result


def process_high_quality(score: float, item_count: int) -> ProcessResult:
    """Process a high-quality dataset (fast path)."""
    print(f"[process_high_quality] score={score:.2f}, items={item_count}")
    print("[process_high_quality] Running optimised high-quality pipeline …")

    result: ProcessResult = {
        "processed_count": item_count,
        "route": "high",
    }
    print(f"[process_high_quality] Processed {item_count} items via high-quality route")
    return result


def process_low_quality(score: float, item_count: int) -> ProcessResult:
    """Clean and re-process a low-quality dataset (slow path with remediation)."""
    print(f"[process_low_quality] score={score:.2f}, items={item_count}")
    print("[process_low_quality] Applying quality remediation steps …")

    # Simulate some items being dropped during cleanup
    cleaned = max(0, item_count - int(item_count * (1.0 - score)))
    result: ProcessResult = {
        "processed_count": cleaned,
        "route": "low",
    }
    print(f"[process_low_quality] Processed {cleaned} items after remediation")
    return result


def notify(score: float, category: str) -> None:
    """Always runs — summarises the quality check and which route was taken."""
    print(f"[notify] Quality check complete: score={score:.2f}, category={category!r}")
    if score > 0.8:
        print("[notify] ✓ High-quality route was executed")
    else:
        print("[notify] ⚠ Low-quality remediation route was executed")
