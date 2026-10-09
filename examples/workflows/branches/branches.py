"""Branches workflow - TypedDict example"""
import time
from datetime import datetime, timezone
from typing import TypedDict


class HelloOutput(TypedDict):
    message: str
    timestamp: datetime


class ComplimentOutput(TypedDict):
    line: str
    adjective: str


def hello(name: str = "world", delay: float = 1.0) -> HelloOutput:
    """Generate a greeting message."""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: HelloOutput = {"message": message, "timestamp": datetime.now(timezone.utc)}
    print(result["message"])
    return result


def compliment(left: HelloOutput, right: HelloOutput, adjective: str = "fast") -> ComplimentOutput:
    """Create a compliment based on two branch outputs."""
    greeting_message = left.get("message") or right.get("message") or "Hello!"

    time.sleep(5)

    line = f"{greeting_message} - you built Czar to be {adjective}!"
    result: ComplimentOutput = {"line": line, "adjective": adjective}
    print(result["line"])
    return result
