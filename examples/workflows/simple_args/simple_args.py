"""Simple arguments example - consolidated single-file"""


def add(a: int, b: int) -> int:
    """Simple addition task using direct arguments."""
    print(f"Calculating {a} + {b}")
    return a + b


def greet(name: str, shout: bool = False) -> str:
    """Task with optional arguments."""
    message = f"Hello, {name}"
    if shout:
        message = message.upper()
    print(message)
    return message


def untyped_func(a, b):
    """Untyped function example."""
    return a + b
