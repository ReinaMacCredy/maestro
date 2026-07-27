from __future__ import annotations

import importlib.util
import re
import sys
import unittest
from pathlib import Path
from types import ModuleType
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import build


CERTIFIED_TRANSITIVE_RUST_SOURCES = (
    "src/domain/authority/downstream_action_basis.rs",
    "src/domain/authority/protected_diagnostic_envelope.rs",
    "src/domain/authority/protected_diagnostic_envelope_stage8_seed.rs",
    "src/domain/evidence/diagnostics/mod.rs",
    "src/domain/persistence/protected_diagnostic.rs",
    "src/domain/persistence/protected_diagnostic_stage9_seed.rs",
)
VALIDATE_PY = Path(__file__).with_name("validate.py")
VERIFY_RB = Path(__file__).with_name("verify.rb")


def load_python_validator() -> ModuleType:
    spec = importlib.util.spec_from_file_location("stage3_domain_validate", VALIDATE_PY)
    if spec is None or spec.loader is None:
        raise AssertionError("Python Stage-3 validator could not be loaded")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def ruby_source_paths() -> list[str]:
    match = re.search(
        r"^SOURCE_PATHS = %w\[\n(?P<body>.*?)^\]\.freeze$",
        VERIFY_RB.read_text(encoding="ascii"),
        flags=re.DOTALL | re.MULTILINE,
    )
    if match is None:
        raise AssertionError("Ruby Stage-3 SOURCE_PATHS block is missing")
    return match.group("body").split()


class Stage3DomainBuildTest(unittest.TestCase):
    def test_certified_sources_are_in_the_live_transitive_closure(self) -> None:
        validator = load_python_validator()
        ruby_paths = ruby_source_paths()
        for name, source_paths in [
            ("builder", build.SOURCE_PATHS),
            ("Python validator", validator.SOURCE_PATHS),
            ("Ruby verifier", ruby_paths),
        ]:
            with self.subTest(tool=name):
                missing = sorted(
                    set(CERTIFIED_TRANSITIVE_RUST_SOURCES) - set(source_paths)
                )
                self.assertEqual([], missing)

        self.assertEqual(build.SOURCE_PATHS, validator.SOURCE_PATHS)
        self.assertEqual(build.SOURCE_PATHS, ruby_paths)
        build.validate_source_closure()
        validator.validate_source_closure()

    def test_live_transitive_closure_rejects_a_missing_source(self) -> None:
        omitted = CERTIFIED_TRANSITIVE_RUST_SOURCES[0]
        declared_without_omitted = [
            relative for relative in build.SOURCE_PATHS if relative != omitted
        ]
        expected = re.escape(f"missing=['{omitted}'], unexpected=[]")

        with patch.object(build, "SOURCE_PATHS", declared_without_omitted):
            with self.assertRaisesRegex(ValueError, expected):
                build.validate_source_closure()


if __name__ == "__main__":
    unittest.main()
