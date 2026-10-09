"""Demo workflow - TypedDict example"""
import json
import time
from datetime import datetime, timezone
from typing import TypedDict


class GreetOutput(TypedDict):
    message: str
    timestamp: datetime


class ComplimentOutput(TypedDict):
    line: str
    adjective: str


class ShellEchoOutput(TypedDict):
    note: str
    finished_at: datetime


def greet(name: str = "world", delay: float = 1.0) -> GreetOutput:
    """Generate a greeting message."""
    if delay > 0:
        time.sleep(delay)

    message = f"Hello, {name}!"
    result: GreetOutput = {"message": message, "timestamp": datetime.now(timezone.utc)}
    print(result["message"])
    return result


def compliment(greet: GreetOutput, adjective: str = "fast") -> ComplimentOutput:
    """Create a compliment based on greeting output."""
    greeting_message = greet.get("message", "Hello!")

    total = 5
    for i in range(1, total + 1):
        print(f"step {i}/{total}")
        print(
            "CZAR_PROGRESS:"
            + json.dumps(
                {
                    "current": i,
                    "total": total,
                    "message": f"step {i}/{total}",
                }
            )
        )
        time.sleep(1)

    line = f"{greeting_message} - you built Czar to be {adjective}!"
    result: ComplimentOutput = {"line": line, "adjective": adjective}
    print(result["line"])
    return result


def shell_echo(
    compliment: ComplimentOutput,
    delay: float = 5.0,
    note: str = "python fallback for shell commands",
) -> ShellEchoOutput:
    """Process a note with some delay."""
    time.sleep(delay)
    final_note = f"{note}: {compliment['line']}"
    result: ShellEchoOutput = {
        "note": final_note,
        "finished_at": datetime.now(timezone.utc),
    }
    print(result["note"])
    return result
