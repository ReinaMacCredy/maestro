# Python Style Guide

## Formatting
- Follow PEP 8 (enforced by ruff/black)
- Max line length: 88 characters (black default)
- Use trailing commas in multi-line collections

## Naming
- Modules/packages: lowercase_with_underscores
- Classes: PascalCase
- Functions/variables: snake_case
- Constants: UPPER_SNAKE_CASE
- Private: prefix with single underscore `_`
- Name-mangled: prefix with double underscore `__` (rare)

## Type Hints
- Use type hints for function signatures and class attributes
- Use `from __future__ import annotations` for forward references
- Prefer `X | Y` syntax over `Union[X, Y]` (Python 3.10+)
- Use `typing.TypeAlias` for complex type expressions

## Functions
- Prefer keyword arguments for functions with 3+ parameters
- Use `*` to force keyword-only arguments
- Docstrings: Google style (Args, Returns, Raises sections)
- Use `@staticmethod` / `@classmethod` appropriately

## Error Handling
- Catch specific exceptions, never bare `except:`
- Use custom exception classes for domain errors
- Context managers (`with`) for resource management
- Prefer EAFP (try/except) over LBYL (if/check)

## Imports
- Group: stdlib > third-party > local
- Absolute imports preferred over relative
- Avoid `from module import *`
- Use `__all__` to control public API

## Data Classes
- Use `@dataclass` or Pydantic `BaseModel` for data containers
- Prefer `NamedTuple` for simple immutable records
- Use `Enum` for fixed sets of values

## Testing
- Use `pytest` (not unittest)
- Fixtures for setup/teardown
- Parametrize for data-driven tests
- Use `conftest.py` for shared fixtures
