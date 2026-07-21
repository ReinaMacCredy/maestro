from __future__ import annotations

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

from tools.vnext_contracts.stage5.evidence_gates import consensus, seal, toolchain


class Stage5ToolchainClosureTests(unittest.TestCase):
    DRIVER_NAME = "librustc_driver-0123456789abcdef.dylib"

    @unittest.skipUnless(sys.platform == "darwin", "macOS developer-tool integration")
    def test_exact_macos_developer_tools_freeze_and_execute_after_relocation(self) -> None:
        closure = seal.build_developer_toolchain_closure(
            Path("/private/tmp/maestro-vnext-stage5-developer-toolchain-test")
        )
        seal.require_frozen_snapshot(closure)
        seal.verify_developer_toolchain_execution(closure)
        with tempfile.TemporaryDirectory() as directory:
            cache_root = Path(directory) / "cache"
            cache_root.mkdir()
            escaped = Path(directory) / "escaped"
            escaped.mkdir()
            (cache_root / "objects").symlink_to(escaped, target_is_directory=True)
            with self.assertRaisesRegex(RuntimeError, "object root is unsafe"):
                seal.build_developer_toolchain_closure(cache_root)
            self.assertEqual(list(escaped.iterdir()), [])

    def fixture(
        self,
    ) -> tuple[tempfile.TemporaryDirectory[str], Path, Path, Path, Path, list[Path]]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        rustc = root / "rustc"
        driver = root / "file"
        target_lib = root / "target-lib"
        output = root / "output"
        rustc.write_bytes(b"rustc-fixture")
        rustc.chmod(0o755)
        driver.write_bytes(b"driver-fixture")
        target_lib.mkdir()
        (target_lib / "libstd-fixture.rlib").write_bytes(b"std-fixture")
        tools = []
        for name in [
            "cargo",
            "python",
            "ruby",
            "cc",
            "cxx",
            "ar",
            "ranlib",
            "lib-lto",
            "git",
        ]:
            path = root / name
            path.write_bytes(f"{name}-fixture".encode("ascii"))
            path.chmod(0o755)
            tools.append(path)
        return temporary, rustc, driver, target_lib, output, tools

    @staticmethod
    def tool_arguments(tools: list[Path]) -> list[str]:
        names = [
            "cargo",
            "python",
            "ruby",
            "cc",
            "cxx",
            "ar",
            "ranlib",
            "lib-lto",
            "git",
        ]
        return [
            value
            for name, path in zip(names, tools, strict=True)
            for value in [f"--{name}", str(path)]
        ]

    def test_materializes_exact_minimal_compiler_closure(self) -> None:
        temporary, rustc, driver, target_lib, output, tools = self.fixture()
        self.addCleanup(temporary.cleanup)
        arguments = [
            "toolchain.py",
            "--rustc",
            str(rustc),
            *self.tool_arguments(tools),
            "--driver",
            str(driver),
            "--driver-name",
            self.DRIVER_NAME,
            "--target-lib",
            str(target_lib),
            "--target",
            "aarch64-apple-darwin",
            "--output-root",
            str(output),
        ]
        with (
            mock.patch("sys.argv", arguments),
            mock.patch.object(toolchain, "verify_developer_toolchain_execution"),
        ):
            self.assertEqual(toolchain.main(), 0)
        receipt = json.loads(
            (output / "rust-toolchain-closure.v1.json").read_text(encoding="ascii")
        )
        self.assertEqual(receipt["target"], "aarch64-apple-darwin")
        self.assertEqual(len(receipt["files"]), 12)
        self.assertEqual(
            receipt["files"], sorted(receipt["files"], key=lambda row: str(row[0]))
        )
        self.assertTrue(
            consensus.validate_toolchain(
                receipt,
                output / "rust-toolchain-closure.v1.json",
                "aarch64-apple-darwin",
            )
        )
        self.assertEqual((output / "toolchain/bin/rustc").read_bytes(), b"rustc-fixture")
        self.assertEqual((output / "toolchain/bin/python3").read_bytes(), b"python-fixture")
        self.assertEqual((output / "toolchain/bin/ruby").read_bytes(), b"ruby-fixture")
        self.assertEqual((output / "toolchain/lib/libLTO.dylib").read_bytes(), b"lib-lto-fixture")
        self.assertEqual(
            (output / "toolchain/lib" / self.DRIVER_NAME).read_bytes(), b"driver-fixture"
        )

    @unittest.skipUnless(sys.platform == "darwin", "macOS developer-tool integration")
    def test_materialized_macos_developer_tools_execute_after_relocation(self) -> None:
        temporary, rustc, driver, target_lib, output, tools = self.fixture()
        self.addCleanup(temporary.cleanup)
        developer = seal.build_developer_toolchain_closure(
            Path("/private/tmp/maestro-vnext-stage5-developer-toolchain-test")
        )
        replacements = {
            "cc": developer / "usr/bin/clang",
            "cxx": developer / "usr/bin/clang++",
            "ar": developer / "usr/bin/ar",
            "ranlib": developer / "usr/bin/ranlib",
            "lib-lto": developer / "usr/lib/libLTO.dylib",
        }
        names = [
            "cargo",
            "python",
            "ruby",
            "cc",
            "cxx",
            "ar",
            "ranlib",
            "lib-lto",
            "git",
        ]
        exact_tools = [replacements.get(name, path) for name, path in zip(names, tools, strict=True)]
        arguments = [
            "toolchain.py",
            "--rustc",
            str(rustc),
            *self.tool_arguments(exact_tools),
            "--driver",
            str(driver),
            "--driver-name",
            self.DRIVER_NAME,
            "--target-lib",
            str(target_lib),
            "--target",
            "aarch64-apple-darwin",
            "--output-root",
            str(output),
        ]
        with mock.patch("sys.argv", arguments):
            self.assertEqual(toolchain.main(), 0)
        toolchain.verify_developer_toolchain_execution(output / "toolchain")

    def test_rejects_nested_tool_aba_during_materialization(self) -> None:
        temporary, rustc, driver, target_lib, output, tools = self.fixture()
        self.addCleanup(temporary.cleanup)
        cargo = tools[0]
        original = toolchain.read_regular
        first = True

        def mutate_after_binding(path: Path) -> tuple[bytes, bool]:
            nonlocal first
            result = original(path)
            if path == cargo and first:
                first = False
                cargo.write_bytes(b"cargo-substituted")
                cargo.chmod(0o755)
            return result

        arguments = [
            "toolchain.py",
            "--rustc",
            str(rustc),
            *self.tool_arguments(tools),
            "--driver",
            str(driver),
            "--driver-name",
            self.DRIVER_NAME,
            "--target-lib",
            str(target_lib),
            "--target",
            "aarch64-apple-darwin",
            "--output-root",
            str(output),
        ]
        with (
            mock.patch("sys.argv", arguments),
            mock.patch.object(toolchain, "read_regular", side_effect=mutate_after_binding),
            self.assertRaisesRegex(RuntimeError, "changed while it was copied"),
        ):
            toolchain.main()

    def test_rejects_invalid_driver_basename(self) -> None:
        temporary, rustc, driver, target_lib, output, tools = self.fixture()
        self.addCleanup(temporary.cleanup)
        arguments = [
            "toolchain.py",
            "--rustc",
            str(rustc),
            *self.tool_arguments(tools),
            "--driver",
            str(driver),
            "--driver-name",
            "../librustc_driver-0123456789abcdef.dylib",
            "--target-lib",
            str(target_lib),
            "--target",
            "aarch64-apple-darwin",
            "--output-root",
            str(output),
        ]
        with mock.patch("sys.argv", arguments), self.assertRaisesRegex(
            RuntimeError, "driver identity is invalid"
        ):
            toolchain.main()

    def test_rejects_driver_substitution_during_materialization(self) -> None:
        temporary, rustc, driver, target_lib, output, tools = self.fixture()
        self.addCleanup(temporary.cleanup)
        original = toolchain.read_regular
        first = True

        def mutate_after_binding(path: Path) -> tuple[bytes, bool]:
            nonlocal first
            result = original(path)
            if path == driver and first:
                first = False
                driver.write_bytes(b"driver-substituted")
            return result

        arguments = [
            "toolchain.py",
            "--rustc",
            str(rustc),
            *self.tool_arguments(tools),
            "--driver",
            str(driver),
            "--driver-name",
            self.DRIVER_NAME,
            "--target-lib",
            str(target_lib),
            "--target",
            "aarch64-apple-darwin",
            "--output-root",
            str(output),
        ]
        with (
            mock.patch("sys.argv", arguments),
            mock.patch.object(toolchain, "read_regular", side_effect=mutate_after_binding),
            self.assertRaisesRegex(RuntimeError, "changed while it was copied"),
        ):
            toolchain.main()

    def test_rejects_symlinked_target_library_entry(self) -> None:
        temporary, rustc, driver, target_lib, output, tools = self.fixture()
        self.addCleanup(temporary.cleanup)
        (target_lib / "substituted.rlib").symlink_to(target_lib / "libstd-fixture.rlib")
        arguments = [
            "toolchain.py",
            "--rustc",
            str(rustc),
            *self.tool_arguments(tools),
            "--driver",
            str(driver),
            "--driver-name",
            self.DRIVER_NAME,
            "--target-lib",
            str(target_lib),
            "--target",
            "aarch64-apple-darwin",
            "--output-root",
            str(output),
        ]
        with mock.patch("sys.argv", arguments), self.assertRaisesRegex(
            RuntimeError, "unsafe entry"
        ):
            toolchain.main()


if __name__ == "__main__":
    unittest.main()
