"""A simple calculator module."""

from typing import List, Optional


def add(a: float, b: float) -> float:
    """Add two numbers."""
    return a + b


def multiply(a: float, b: float) -> float:
    """Multiply two numbers."""
    return a * b


def divide(a: float, b: float) -> Optional[float]:
    """Divide a by b, returns None on division by zero."""
    if b == 0:
        return None
    return a / b


class Calculator:
    """Stateful calculator with history."""

    def __init__(self) -> None:
        self.history: List[float] = []

    def compute(self, a: float, b: float, op: str) -> float:
        """Perform a binary operation and record the result."""
        ops = {"+": add, "*": multiply}
        func = ops.get(op)
        if func is None:
            raise ValueError(f"Unknown operation: {op}")
        result = func(a, b)
        self.history.append(result)
        return result

    def last_result(self) -> Optional[float]:
        """Return the most recent result, or None if no history."""
        return self.history[-1] if self.history else None
