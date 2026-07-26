from __future__ import annotations

import ast
import json
import os
import pwd
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from typing import cast
from unittest import mock

from tools.vnext_contracts.stage5.evidence_gates import seal, toolchain
from tools.vnext_contracts.proof_engine import (
    CommandSpec,
    EngineError,
    InputBinding,
    PhaseSpec,
    ProofEngine,
    ProofPlan,
    PublicationSpec,
    PublishedOutput,
    ToolSpec,
)


ADAPTER_PHASE_SCRIPT = """
from pathlib import Path
import sys

phase = sys.argv[1]
output = Path(sys.argv[2])
marker = Path(sys.argv[3])
counts = Path(sys.argv[4])
counts.mkdir(parents=True, exist_ok=True)
with (counts / phase).open("a", encoding="ascii") as stream:
    stream.write("run\\n")
if phase == "predecessor" and not marker.exists():
    marker.write_text("interrupted", encoding="ascii")
    raise SystemExit(19)
(output / "result.txt").write_text(phase, encoding="ascii")
""".strip()


class Stage5SnapshotTests(unittest.TestCase):
    def fixture(self) -> tuple[tempfile.TemporaryDirectory[str], Path, Path]:
        temporary = tempfile.TemporaryDirectory()
        root = Path(temporary.name)
        workspace = root / "workspace"
        cache = root / "cache"
        workspace.mkdir()
        (workspace / "source.txt").write_bytes(b"bound-source\n")
        return temporary, workspace, cache

    def test_snapshot_cache_ignores_substituted_source_pointer_and_reconstructs_closure(
        self,
    ) -> None:
        temporary, workspace, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        pointer = cache / "by-source" / f"{'a' * 64}.json"
        pointer.parent.mkdir(parents=True)
        pointer.write_text(
            json.dumps(
                {
                    "snapshot_identity": f"sha256:{'b' * 64}",
                    "source_identity": f"sha256:{'a' * 64}",
                }
            ),
            encoding="ascii",
        )
        substituted = cache / "objects" / ("b" * 64)
        substituted.mkdir(parents=True)
        (substituted / "source.txt").write_text("substituted\n", encoding="ascii")
        vendored = mock.Mock(returncode=0, stdout=b"", stderr=b"")
        with (
            mock.patch.object(seal, "WORKSPACE", workspace),
            mock.patch.object(seal, "SNAPSHOT_PATHS", ("source.txt",)),
            mock.patch.object(subprocess, "run", return_value=vendored),
        ):
            result = seal.build_snapshot(Path("/bin/false"), cache)
        self.assertNotEqual(result, substituted)
        self.assertEqual((result / "source.txt").read_bytes(), b"bound-source\n")

    def test_snapshot_cache_reuses_a_frozen_content_bound_snapshot_without_revendoring(
        self,
    ) -> None:
        temporary, workspace, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        vendored = mock.Mock(return_value=mock.Mock(returncode=0, stdout=b"", stderr=b""))
        with (
            mock.patch.object(seal, "WORKSPACE", workspace),
            mock.patch.object(seal, "SNAPSHOT_PATHS", ("source.txt",)),
            mock.patch.object(subprocess, "run", vendored),
        ):
            first = seal.build_snapshot(Path("/bin/false"), cache)
            second = seal.build_snapshot(Path("/bin/false"), cache)

        self.assertEqual(first, second)
        self.assertEqual(vendored.call_count, 1)

    def test_snapshot_bootstrap_executes_the_immutable_seal_copy(self) -> None:
        temporary, workspace, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        snapshot = workspace / "snapshot"
        snapshot_script = snapshot / "tools/vnext_contracts/stage5/evidence_gates/seal.py"
        snapshot_script.parent.mkdir(parents=True)
        snapshot_script.write_text("# immutable seal fixture\n", encoding="ascii")
        sdk_root = workspace / "sdk"
        sdk_root.mkdir()
        with (
            mock.patch.object(sys, "argv", ["seal.py", "--cache-root", str(cache)]),
            mock.patch.object(seal, "build_snapshot", return_value=snapshot),
            mock.patch.object(seal, "macos_sdk_root", return_value=sdk_root),
            mock.patch.object(seal, "toolchain_binary", return_value=Path("/bin/true")),
            mock.patch.object(os, "execve", side_effect=RuntimeError("exec-boundary")) as execute,
            self.assertRaisesRegex(RuntimeError, "exec-boundary"),
        ):
            seal.main()
        executable, arguments, environment = execute.call_args.args
        self.assertEqual(executable, str(Path(sys.executable).resolve(strict=True)))
        self.assertEqual(arguments[1], str(snapshot_script))
        self.assertIn("--immutable-snapshot", arguments)
        self.assertEqual(arguments[arguments.index("--sdk-root") + 1], str(sdk_root))
        self.assertEqual(arguments[arguments.index("--max-workers") + 1], "6")
        self.assertEqual(arguments[arguments.index("--compile-workers") + 1], "2")
        self.assertEqual(environment["PYTHONDONTWRITEBYTECODE"], "1")
        self.assertNotIn("PYTHONPATH", environment)

    def test_snapshot_copy_rejects_source_change(self) -> None:
        temporary, workspace, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        original = seal.copy_snapshot_sources

        def copy_then_mutate(destination: Path, rows: list[list[object]]) -> None:
            original(destination, rows)
            (workspace / "source.txt").write_bytes(b"changed-source\n")

        with (
            mock.patch.object(seal, "WORKSPACE", workspace),
            mock.patch.object(seal, "SNAPSHOT_PATHS", ("source.txt",)),
            mock.patch.object(seal, "copy_snapshot_sources", copy_then_mutate),
            self.assertRaisesRegex(RuntimeError, "changed while it was copied"),
        ):
            seal.build_snapshot(Path("/bin/false"), cache)

    def test_reconstruction_does_not_readmit_a_mutable_snapshot_object(self) -> None:
        temporary, workspace, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        vendored = mock.Mock(returncode=0, stdout=b"", stderr=b"")
        with (
            mock.patch.object(seal, "WORKSPACE", workspace),
            mock.patch.object(seal, "SNAPSHOT_PATHS", ("source.txt",)),
            mock.patch.object(subprocess, "run", return_value=vendored),
        ):
            target = seal.build_snapshot(Path("/bin/false"), cache)
            target.chmod(0o755)
            with self.assertRaisesRegex(RuntimeError, "mutable or unsafe"):
                seal.build_snapshot(Path("/bin/false"), cache)

    def test_ruby_verifier_executes_its_exact_test_output_parser(self) -> None:
        ruby = seal.required_tool("ruby")
        ruby_before, _ = seal.read_regular_file(ruby)
        home = pwd.getpwuid(os.getuid()).pw_dir
        completed = subprocess.run(
            [str(ruby), str(Path(seal.__file__).with_name("verify.rb")), "--self-test-output-parser"],
            capture_output=True,
            check=False,
            text=True,
            env={
                "HOME": home,
                "LANG": "C",
                "LC_ALL": "C",
                "PATH": "/usr/bin:/bin",
                "RUBYLIB": "",
                "RUBYOPT": "",
            },
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        ruby_after, _ = seal.read_regular_file(ruby)
        self.assertEqual(seal.sha256(ruby_before), seal.sha256(ruby_after))
        self.assertEqual(
            json.loads(completed.stdout),
            {
                "behavior_manifest_identity": "sha256:fe5df73a47fb802b0ef87afafab04267c0b8a540931c8a6e667749f3a60131a5",
                "exact_test_output_parser": "pass",
            },
        )

    def test_driver_name_literal_is_identity_only_while_static_argument_executes(self) -> None:
        temporary, _, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        script = root / "record-driver.py"
        script.write_text(
            "from pathlib import Path\n"
            "import sys\n"
            "source = Path(sys.argv[1])\n"
            "destination = Path(sys.argv[3])\n"
            "destination.write_text(f'{source.name}:{sys.argv[2]}', encoding='ascii')\n",
            encoding="ascii",
        )
        driver = root / "file"
        driver.write_bytes(b"driver-fixture")
        driver_name = "librustc_driver-0123456789abcdef.dylib"
        self.assertIsNotNone(toolchain.DRIVER_NAME.fullmatch(driver_name))
        plan = ProofPlan(
            inputs=(
                InputBinding.file("script", script),
                InputBinding.file("rustc-driver", driver),
                InputBinding.literal("rustc-driver-name", driver_name),
            ),
            tools=(ToolSpec("python", Path(sys.executable), ("--version",)),),
            phases=(
                PhaseSpec(
                    name="toolchain",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                "{input:rustc-driver}",
                                driver_name,
                                "{phase_root}/driver-name.txt",
                            ),
                            label="static-driver-name",
                        ),
                    ),
                    inputs=("script", "rustc-driver", "rustc-driver-name"),
                ),
            ),
        )
        result = ProofEngine().execute(
            plan,
            run_root=root / "run",
            cache_root=cache,
            run_token="stage5-driver-name-static-argument",
        )
        self.assertEqual(
            (result.phases[0].output_root / "driver-name.txt").read_text(encoding="ascii"),
            f"file:{driver_name}",
        )

    def test_seven_phase_adapter_resumes_exact_topology_after_interruption(self) -> None:
        temporary, _, cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        root = Path(temporary.name)
        source = (Path(seal.__file__).read_text(encoding="utf-8"))
        calls = sorted(
            (
                node
                for node in ast.walk(ast.parse(source))
                if isinstance(node, ast.Call)
                and isinstance(node.func, ast.Name)
                and node.func.id == "PhaseSpec"
            ),
            key=lambda node: node.lineno,
        )

        def literal(call: ast.Call, key: str, default: object) -> object:
            keyword = next((item for item in call.keywords if item.arg == key), None)
            return default if keyword is None else ast.literal_eval(keyword.value)

        topology: list[tuple[str, tuple[str, ...], str, str]] = [
            (
                str(literal(call, "name", "")),
                cast(tuple[str, ...], literal(call, "dependencies", ())),
                str(literal(call, "cache_mode", "run")),
                str(literal(call, "resource_class", "light")),
            )
            for call in calls
        ]
        self.assertEqual(
            [name for name, _, _, _ in topology],
            ["toolchain", "predecessor", "harness", "builder", "validator", "ruby", "consensus"],
        )
        self.assertEqual(
            [mode for _, _, mode, _ in topology],
            ["content", "content", *(["run"] * 5)],
        )
        self.assertEqual(
            [resource_class for _, _, _, resource_class in topology],
            ["light", "light", "light", "compile", "compile", "compile", "light"],
        )
        dependencies = {name: deps for name, deps, _, _ in topology}
        self.assertEqual(dependencies["predecessor"], ())
        self.assertEqual(dependencies["harness"], ())

        script = root / "adapter-phase.py"
        script.write_text(ADAPTER_PHASE_SCRIPT + "\n", encoding="utf-8")
        marker = root / "interrupt-once"
        counts = root / "counts"
        run_root = root / "run"
        publication = root / "publication"
        pointer = root / "current.json"
        plan = ProofPlan(
            inputs=(InputBinding.file("script", script),),
            tools=(ToolSpec("python", Path(sys.executable), ("--version",)),),
            phases=tuple(
                PhaseSpec(
                    name=name,
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                name,
                                "{phase_root}",
                                str(marker),
                                str(counts),
                            ),
                            label=f"stage5-adapter-{name}",
                        ),
                    ),
                      inputs=("script",),
                      dependencies=dependencies,
                      cache_mode=mode,
                      resource_class=resource_class,
                  )
                    for name, dependencies, mode, resource_class in topology
                ),
            publication=PublicationSpec(
                release_root=publication,
                pointer_path=pointer,
                outputs=(PublishedOutput("consensus", "result.txt", "result.txt"),),
            ),
        )
        engine = ProofEngine()
        with self.assertRaises(EngineError):
            engine.execute(
                plan,
                run_root=run_root,
                cache_root=cache,
                run_token="stage5-adapter-resume",
            )
        result = engine.execute(
            plan,
            run_root=run_root,
            cache_root=cache,
            run_token="stage5-adapter-resume",
        )
        self.assertEqual([phase.name for phase in result.phases], [row[0] for row in topology])
        self.assertTrue(pointer.is_file())
        self.assertEqual((counts / "toolchain").read_text(encoding="ascii"), "run\n")
        self.assertEqual((counts / "predecessor").read_text(encoding="ascii"), "run\nrun\n")
        for name in ["harness", "builder", "validator", "ruby", "consensus"]:
            self.assertEqual((counts / name).read_text(encoding="ascii"), "run\n")

    def test_snapshot_source_rejects_symlinked_directory(self) -> None:
        temporary, workspace, _cache = self.fixture()
        self.addCleanup(temporary.cleanup)
        foreign = Path(temporary.name) / "foreign"
        foreign.mkdir()
        (foreign / "substituted.txt").write_text("substituted", encoding="ascii")
        (workspace / "linked").symlink_to(foreign, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "unsafe entry"):
            seal.path_rows(workspace, (".",))


if __name__ == "__main__":
    unittest.main()
