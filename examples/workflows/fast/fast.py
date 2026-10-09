import time
from typing import TypedDict


class TaskResult(TypedDict):
    task: str
    status: str
    timestamp: float


def task_a() -> TaskResult:
    """First task - just prints and returns."""
    print("Task A: Starting processing")
    result: TaskResult = {"task": "A", "status": "completed", "timestamp": time.time()}
    print(f"Task A: Result = {result}")
    return result


def task_b(task_a: TaskResult) -> TaskResult:
    """Second task - depends on A, uses its output."""
    print("Task B: Starting processing")
    print(f"Task B: Received from A: {task_a}")
    result: TaskResult = {"task": "B", "status": "completed", "timestamp": time.time()}
    print(f"Task B: Result = {result}")
    return result


def task_c(task_b: TaskResult) -> TaskResult:
    """Third task - depends on B, uses its output."""
    print("Task C: Starting processing")
    print(f"Task C: Received from B: {task_b}")
    result: TaskResult = {"task": "C", "status": "completed", "timestamp": time.time()}
    print(f"Task C: Result = {result}")
    return result
