from __future__ import annotations

import ast
import re
import unittest
from pathlib import Path


TOOLS = Path(__file__).resolve().parent
BUILD_PY = TOOLS / "build.py"
VALIDATE_PY = TOOLS / "validate.py"
VERIFY_RB = TOOLS / "verify.rb"
TARGET_COMMAND = [
    "cargo",
    "test",
    "--lib",
    "domain::vnext::authority::facade::repository_admission::ancestry_tests",
    "--",
    "--nocapture",
]
EXPECTED_BEHAVIOR_COUNTS = [70, 7, 1, 1, 1, 1]


def python_assignment(path: Path, name: str) -> object:
    module = ast.parse(path.read_text(encoding="ascii"), filename=str(path))
    for statement in module.body:
        if not isinstance(statement, ast.Assign):
            continue
        if any(isinstance(target, ast.Name) and target.id == name for target in statement.targets):
            return ast.literal_eval(statement.value)
    raise AssertionError(f"{path.name} has no {name} assignment")


def ruby_behavior_commands() -> list[list[str]]:
    source = VERIFY_RB.read_text(encoding="ascii")
    match = re.search(
        r"^BEHAVIOR_COMMANDS = \[\n(?P<body>.*?)^\s*\]\.freeze$",
        source,
        flags=re.DOTALL | re.MULTILINE,
    )
    if match is None:
        raise AssertionError("verify.rb has no BEHAVIOR_COMMANDS assignment")
    return [command.split() for command in re.findall(r"%w\[([^]]+)\]", match.group("body"))]


def ruby_behavior_counts() -> list[int]:
    source = VERIFY_RB.read_text(encoding="ascii")
    match = re.search(
        r"^\s*BEHAVIOR_EXPECTED_PASSED = \[(?P<body>[0-9, ]+)\]\.freeze$",
        source,
        flags=re.MULTILINE,
    )
    if match is None:
        raise AssertionError("verify.rb has no BEHAVIOR_EXPECTED_PASSED assignment")
    return [int(value) for value in match.group("body").split(",")]


class Stage4BehaviorCensusTest(unittest.TestCase):
    def test_mirrors_pin_the_exact_behavior_command_census(self) -> None:
        builder_commands = python_assignment(BUILD_PY, "BEHAVIOR_COMMANDS")
        validator_commands = python_assignment(VALIDATE_PY, "BEHAVIOR_COMMANDS")
        ruby_commands = ruby_behavior_commands()
        builder_counts = python_assignment(BUILD_PY, "BEHAVIOR_EXPECTED_PASSED")
        validator_counts = python_assignment(VALIDATE_PY, "BEHAVIOR_EXPECTED_PASSED")
        ruby_counts = ruby_behavior_counts()

        self.assertEqual(builder_commands, validator_commands)
        self.assertEqual(builder_commands, ruby_commands)
        self.assertEqual(builder_counts, validator_counts)
        self.assertEqual(builder_counts, ruby_counts)
        self.assertEqual(EXPECTED_BEHAVIOR_COUNTS, builder_counts)
        target_index = builder_commands.index(TARGET_COMMAND)
        self.assertEqual(7, builder_counts[target_index])


if __name__ == "__main__":
    unittest.main()
