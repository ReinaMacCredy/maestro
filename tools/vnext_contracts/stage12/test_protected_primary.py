"""Disposable-repository tests for the protected-primary currentness boundary."""

from __future__ import annotations

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


sys.dont_write_bytecode = True
ROOT = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT))

import protected_primary  # type: ignore[import-not-found]  # noqa: E402


class ProtectedPrimaryCurrentnessTests(unittest.TestCase):
    def _git(self, repository: Path, *arguments: str) -> str:
        result = subprocess.run(
            ["git", "-C", str(repository), *arguments],
            check=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
        )
        return result.stdout.strip()

    def _fixture(self, root: Path) -> tuple[Path, dict[str, object]]:
        repository = root / "primary"
        self._git(root, "init", str(repository))
        self._git(repository, "config", "user.name", "Primary Test")
        self._git(repository, "config", "user.email", "primary@example.invalid")
        tracked = repository / "tracked.txt"
        tracked.write_text("committed\n", encoding="utf-8")
        self._git(repository, "add", "tracked.txt")
        self._git(repository, "commit", "-m", "primary preimage")
        tracked.write_text("dirty tracked bytes\n", encoding="utf-8")
        untracked = repository / "untracked.txt"
        untracked.write_text("untracked bytes\n", encoding="utf-8")
        observation = protected_primary.observe_currentness(repository)
        core: dict[str, object] = {
            "schema": protected_primary.BINDING_SCHEMA,
            **observation,
            "boundary_identity": "1" * 64,
            "boundary_file_sha256": "2" * 64,
            "policy": protected_primary.BINDING_POLICY,
        }
        return repository, protected_primary.with_identity(core)

    def test_exact_commit_tree_status_diff_and_untracked_identity_accept(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, binding = self._fixture(Path(directory))
            self.assertEqual(
                protected_primary.verify_currentness(binding, repository),
                protected_primary.observe_currentness(repository),
            )

    def test_tracked_byte_mutant_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, binding = self._fixture(Path(directory))
            (repository / "tracked.txt").write_text(
                "different dirty tracked bytes\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(
                protected_primary.ProtectedPrimaryError,
                "currentness differs",
            ):
                protected_primary.verify_currentness(binding, repository)

    def test_dirty_path_mutant_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, binding = self._fixture(Path(directory))
            (repository / "another-untracked.txt").write_text(
                "new path\n", encoding="utf-8"
            )
            with self.assertRaisesRegex(
                protected_primary.ProtectedPrimaryError,
                "currentness differs",
            ):
                protected_primary.verify_currentness(binding, repository)

    def test_untracked_mode_mutant_refuses(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, binding = self._fixture(Path(directory))
            os.chmod(repository / "untracked.txt", 0o600)
            with self.assertRaisesRegex(
                protected_primary.ProtectedPrimaryError,
                "currentness differs",
            ):
                protected_primary.verify_currentness(binding, repository)


if __name__ == "__main__":
    unittest.main()
