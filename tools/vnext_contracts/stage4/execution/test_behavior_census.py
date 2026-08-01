from __future__ import annotations

import ast
import os
import re
import runpy
import unittest
from pathlib import Path
from unittest import mock


TOOLS = Path(__file__).resolve().parent
BUILD_PY = TOOLS / "build.py"
VALIDATE_PY = TOOLS / "validate.py"
VERIFY_RB = TOOLS / "verify.rb"
TARGET_COMMAND = [
    "cargo",
    "test",
    "--lib",
    "domain::authority::facade::repository_admission::ancestry_tests",
    "--",
    "--nocapture",
]
EXPECTED_BEHAVIOR_COUNTS = [75, 7, 1, 1, 1, 1]


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


def ruby_word_list(name: str) -> list[str]:
    source = VERIFY_RB.read_text(encoding="ascii")
    match = re.search(
        rf"^{name} = %w\[\n(?P<body>.*?)^\]\.freeze$",
        source,
        flags=re.DOTALL | re.MULTILINE,
    )
    if match is None:
        raise AssertionError(f"verify.rb has no {name} assignment")
    return match.group("body").split()


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

    def test_python_engines_use_a_minimal_environment_and_bound_interpreters(self) -> None:
        hostile = {
            "HOME": "/inject",
            "PATH": "/hostile",
            "PYTHONPATH": "/inject",
            "PYTHONHOME": "/inject",
            "RUBYOPT": "-rinject",
            "RUBYLIB": "/inject",
            "RUSTFLAGS": "-C target-cpu=native",
        }
        for path in (BUILD_PY, VALIDATE_PY):
            namespace = runpy.run_path(path)
            environment = namespace["command_environment"]
            environment.__globals__["tool_descriptor"] = lambda _name: {
                "invocation_path": "/bound/rustc"
            }
            with self.subTest(path=path.name), mock.patch.dict(
                os.environ, hostile, clear=True
            ):
                actual = environment()
            self.assertEqual(
                set(actual),
                {
                    "CARGO_HOME",
                    "CARGO_INCREMENTAL",
                    "HOME",
                    "LANG",
                    "LC_ALL",
                    "PATH",
                    "PYTHONDONTWRITEBYTECODE",
                    "RUSTC",
                    "RUSTUP_HOME",
                },
            )
            self.assertEqual(actual["RUSTC"], "/bound/rustc")
            self.assertEqual(actual["PATH"], "/usr/bin:/bin:/usr/sbin:/sbin")
            self.assertNotEqual(actual["HOME"], hostile["HOME"])

        builder = BUILD_PY.read_text(encoding="ascii")
        self.assertIn('tool_descriptor("python-current")', builder)
        self.assertIn('tool_descriptor("ruby")', builder)
        self.assertNotIn('["ruby", str(TOOLS / "verify.rb")', builder)

    def test_ruby_engine_cannot_inherit_hostile_process_environment(self) -> None:
        source = VERIFY_RB.read_text(encoding="ascii")
        self.assertNotIn("ENV.to_h", source)
        match = re.search(
            r"^def command_environment\n(?P<body>.*?)^end$",
            source,
            flags=re.DOTALL | re.MULTILINE,
        )
        self.assertIsNotNone(match)
        body = match.group("body")
        for key in (
            "CARGO_HOME",
            "CARGO_INCREMENTAL",
            "HOME",
            "LANG",
            "LC_ALL",
            "PATH",
            "PYTHONDONTWRITEBYTECODE",
            "RUSTC",
            "RUSTUP_HOME",
        ):
            self.assertIn(f'"{key}" =>', body)
        for forbidden in (
            "PYTHONHOME",
            "PYTHONPATH",
            "RUBYLIB",
            "RUBYOPT",
            "RUSTFLAGS",
        ):
            self.assertNotIn(f'"{forbidden}" => ENV', body)

    def test_authority_extension_source_sets_are_exact_and_include_owner_seed(self) -> None:
        builder = python_assignment(BUILD_PY, "AUTHORITY_EXTENSION_SOURCES")
        validator = python_assignment(VALIDATE_PY, "AUTHORITY_EXTENSION_SOURCES")
        ruby = ruby_word_list("AUTHORITY_EXTENSION_SOURCES")
        self.assertEqual(builder, validator)
        self.assertEqual(builder, ruby)
        self.assertIn(
            "src/foundation/core/descriptor_census_platform_stage11_seed.rs",
            builder,
        )

    def test_shared_installation_source_sets_bind_the_stage11_seed_and_module_facade(self) -> None:
        expected = (
            "src/domain/installation/consumer_snapshot_stage11_seed.rs",
            "src/domain/installation/mod.rs",
        )
        for path in (BUILD_PY, VALIDATE_PY, VERIFY_RB):
            source = path.read_text(encoding="ascii")
            with self.subTest(path=path.name):
                for required in expected:
                    self.assertIn(required, source)


if __name__ == "__main__":
    unittest.main()
