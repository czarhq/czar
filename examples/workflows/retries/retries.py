"""Retries workflow - TypedDict example"""
import os
import time
from datetime import datetime, timezone
from typing import TypedDict


class FlakeyOutput(TypedDict):
    ok: bool
    attempt: int


class HelloOutput(TypedDict):
    message: str
    timestamp: datetime


def flakey(fail_times: int = 0) -> FlakeyOutput:
    """Simulates a flakey operation that fails N times before succeeding."""
    attempt = int(os.environ.get("CZAR_ATTEMPT", "1"))

    time.sleep(2)

    if attempt <= fail_times:
        raise RuntimeError(f"Intentional failure on attempt {attempt}")

    result: FlakeyOutput = {"ok": True, "attempt": attempt}
    print(f"succeeded on attempt {result['attempt']}")
    return result


def hello(name: str = "world", delay: float = 1.0) -> HelloOutput:
    """Generate a greeting message."""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: HelloOutput = {"message": message, "timestamp": datetime.now(timezone.utc)}
    print(result["message"])
    return result
