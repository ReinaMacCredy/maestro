from __future__ import annotations

import json
import sys
import tempfile
import time
import unittest
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Mapping, Sequence

from tools.vnext_contracts.proof_engine import (
    CacheCorruptionError,
    CommandSpec,
    EngineError,
    InputBinding,
    InputMutationError,
    PhaseResult,
    PhaseSpec,
    PlanError,
    ProofEngine,
    ProofPlan,
    ProofRunResult,
    PublicationError,
    PublicationSpec,
    PublishedOutput,
    ToolSpec,
)


WRITE_SCRIPT = """
from pathlib import Path
import sys

source = Path(sys.argv[1]).read_text(encoding="utf-8")
destination = Path(sys.argv[2]) / "result.txt"
destination.write_text(source + sys.argv[3], encoding="utf-8")
""".strip()

MUTATE_SCRIPT = """
from pathlib import Path
import sys

source = Path(sys.argv[1])
source.write_text("mutated", encoding="utf-8")
(Path(sys.argv[2]) / "result.txt").write_text("unsafe", encoding="utf-8")
""".strip()

PARALLEL_SCRIPT = """
from pathlib import Path
import sys
import time

own = Path(sys.argv[1])
barrier = Path(sys.argv[2])
name = sys.argv[3]
(barrier / f"{name}.ready").write_text("ready", encoding="utf-8")
deadline = time.monotonic() + 3
while len(list(barrier.glob("*.ready"))) < 2:
    if time.monotonic() >= deadline:
        raise SystemExit("peer phase did not start concurrently")
    time.sleep(0.01)
(own / "done").write_text("done", encoding="utf-8")
""".strip()

PINNED_INPUT_SCRIPT = """
from pathlib import Path
import sys
import time

source = Path(sys.argv[1])
output = Path(sys.argv[2])
barrier = Path(sys.argv[3])
(barrier / "ready").write_text("ready", encoding="utf-8")
while not (barrier / "read").is_file():
    time.sleep(0.01)
value = source.read_text(encoding="utf-8")
(barrier / "consumed").write_text("consumed", encoding="utf-8")
while not (barrier / "finish").is_file():
    time.sleep(0.01)
(output / "result.txt").write_text(value, encoding="utf-8")
""".strip()

COUNT_SCRIPT = """
from pathlib import Path
import sys

output = Path(sys.argv[1])
counter = Path(sys.argv[2])
with counter.open("a", encoding="utf-8") as handle:
    handle.write("run\\n")
(output / "result.txt").write_text("done", encoding="utf-8")
""".strip()

DEPENDENCY_BUILDER_SCRIPT = """
from pathlib import Path
import sys

(Path(sys.argv[1]) / "seed.txt").write_text("seed", encoding="utf-8")
""".strip()

DEPENDENCY_MUTATOR_SCRIPT = """
from pathlib import Path
import sys

dependency = Path(sys.argv[1])
(dependency / "seed.txt").write_text("mutated", encoding="utf-8")
(Path(sys.argv[2]) / "child.txt").write_text("child", encoding="utf-8")
""".strip()

NONDETERMINISTIC_RACE_SCRIPT = """
from pathlib import Path
import sys
import time

phase_root = Path(sys.argv[1])
barrier = Path(sys.argv[2])
run_name = phase_root.parents[2].name
(barrier / run_name).write_text("ready", encoding="utf-8")
deadline = time.monotonic() + 3
while len(list(barrier.iterdir())) < 2:
    if time.monotonic() >= deadline:
        raise SystemExit("concurrent cache writer did not arrive")
    time.sleep(0.01)
(phase_root / "result.txt").write_text(run_name, encoding="utf-8")
""".strip()

FAIL_ONCE_SCRIPT = """
from pathlib import Path
import sys

marker = Path(sys.argv[1])
output = Path(sys.argv[2]) / "finished.txt"
if not marker.exists():
    marker.write_text("interrupted", encoding="utf-8")
    raise SystemExit(17)
output.write_text("resumed", encoding="utf-8")
""".strip()

PHASE_TEMP_SCRIPT = """
from pathlib import Path
import sys

temporary = Path(sys.argv[1])
output = Path(sys.argv[2])
(temporary / "scratch.txt").write_text("scratch", encoding="utf-8")
(output / "result.txt").write_text("done", encoding="utf-8")
""".strip()


class ProofEngineTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory(prefix="maestro-proof-engine-test-")
        self.root = Path(self.temporary.name)
        self.inputs = self.root / "inputs"
        self.inputs.mkdir()
        self.script = self.inputs / "write.py"
        self.script.write_text(WRITE_SCRIPT + "\n", encoding="utf-8")
        self.source = self.inputs / "source.txt"
        self.source.write_text("source-v1", encoding="utf-8")
        self.cache = self.root / "cache"
        self.engine = ProofEngine()

    def tearDown(self) -> None:
        self.temporary.cleanup()

    def plan(
        self,
        *,
        cache_mode: str = "content",
        suffix: str = "|default",
        fact: str = "aarch64-apple-darwin",
        publication: PublicationSpec | None = None,
        script: Path | None = None,
    ) -> ProofPlan:
        selected_script = script or self.script
        return ProofPlan(
            inputs=(
                InputBinding.file("script", selected_script),
                InputBinding.file("source", self.source),
                InputBinding.literal("fact", fact),
            ),
            tools=(ToolSpec("python", Path(sys.executable), ("--version",)),),
            phases=(
                PhaseSpec(
                    name="builder",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=("{input:script}", "{input:source}", "{phase_root}", suffix),
                            label="build-fixture",
                        ),
                    ),
                    inputs=("script", "source", "fact"),
                    cache_mode=cache_mode,
                ),
            ),
            publication=publication,
        )

    def execute(
        self,
        plan: ProofPlan,
        name: str,
        *,
        run_token: str | None = None,
        max_workers: int = 1,
        performance: bool = False,
    ) -> ProofRunResult:
        run_root = self.root / "runs" / name
        return self.engine.execute(
            plan,
            run_root=run_root,
            cache_root=self.cache,
            run_token=run_token,
            max_workers=max_workers,
            performance_log=self.root / "performance" / f"{name}.v1.jsonl"
            if performance
            else None,
        )

    def test_content_cache_hit_and_performance_data_is_non_canonical(self) -> None:
        plan = self.plan()
        first = self.execute(plan, "first", performance=True)
        second = self.execute(plan, "second", performance=True)

        self.assertEqual(first.plan_identity, second.plan_identity)
        self.assertEqual(first.phases[0].identity, second.phases[0].identity)
        self.assertEqual(first.phases[0].cache_status, "miss")
        self.assertEqual(second.phases[0].cache_status, "hit")
        checkpoint = (
            self.cache
            / "objects"
            / first.phases[0].identity.removeprefix("sha256:")
            / "checkpoint.json"
        ).read_text(encoding="ascii")
        self.assertNotIn("duration_ms", checkpoint)
        events = [
            json.loads(line)
            for line in (self.root / "performance/second.v1.jsonl")
            .read_text(encoding="ascii")
            .splitlines()
        ]
        self.assertEqual(events[-1]["cache_status"], "hit")
        self.assertIn("duration_ms", events[-1])

    def test_completed_run_token_cannot_replay_but_new_seal_reexecutes(self) -> None:
        plan = self.plan(cache_mode="run")
        first = self.execute(plan, "seal-a-first", run_token="seal-a")
        with self.assertRaises(PlanError):
            self.execute(plan, "seal-a-replayed", run_token="seal-a")
        new_seal = self.execute(plan, "seal-b", run_token="seal-b")

        self.assertEqual(first.phases[0].cache_status, "miss")
        self.assertEqual(new_seal.phases[0].cache_status, "miss")
        self.assertNotEqual(first.phases[0].identity, new_seal.phases[0].identity)
        self.assertFalse(
            (self.root / "runs/seal-a-first/.maestro-proof-complete.v1.json").exists()
        )
        self.assertEqual(len(list((self.cache / "completed-runs").glob("*.json"))), 2)

    def test_interrupted_run_resumes_only_its_completed_checkpoints(self) -> None:
        finisher = self.inputs / "fail-once.py"
        finisher.write_text(FAIL_ONCE_SCRIPT + "\n", encoding="utf-8")
        external_marker = self.root / "interruption-marker"
        plan = ProofPlan(
            inputs=(
                InputBinding.file("builder-script", self.script),
                InputBinding.file("source", self.source),
                InputBinding.file("finisher-script", finisher),
            ),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="builder",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:builder-script}",
                                "{input:source}",
                                "{phase_root}",
                                "|resume",
                            ),
                        ),
                    ),
                    inputs=("builder-script", "source"),
                    cache_mode="run",
                ),
                PhaseSpec(
                    name="finisher",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:finisher-script}",
                                str(external_marker),
                                "{phase_root}",
                            ),
                        ),
                    ),
                    inputs=("finisher-script",),
                    dependencies=("builder",),
                    cache_mode="run",
                ),
            ),
        )
        with self.assertRaises(EngineError):
            self.execute(plan, "interrupted", run_token="one-seal")
        resumed = self.execute(plan, "interrupted", run_token="one-seal")

        self.assertEqual(resumed.phases[0].cache_status, "hit")
        self.assertEqual(resumed.phases[1].cache_status, "miss")
        self.assertEqual(
            (resumed.phases[1].output_root / "finished.txt").read_text(encoding="utf-8"),
            "resumed",
        )

    def test_source_command_and_literal_changes_each_invalidate_cache(self) -> None:
        baseline = self.execute(self.plan(), "baseline")

        self.source.write_text("source-v2", encoding="utf-8")
        source_change = self.execute(self.plan(), "source-change")
        command_change = self.execute(self.plan(suffix="|changed"), "command-change")
        literal_change = self.execute(
            self.plan(suffix="|changed", fact="x86_64-unknown-linux-gnu"),
            "literal-change",
        )

        results = [baseline, source_change, command_change, literal_change]
        self.assertTrue(all(result.phases[0].cache_status == "miss" for result in results))
        self.assertEqual(len({result.phases[0].identity for result in results}), len(results))

    def test_internal_symlink_tree_is_content_bound_and_copied_into_the_frozen_run(self) -> None:
        sdk = self.inputs / "sdk"
        (sdk / "real").mkdir(parents=True)
        (sdk / "real/header.txt").write_text("bound-sdk", encoding="utf-8")
        (sdk / "alias").symlink_to("real", target_is_directory=True)
        plan = ProofPlan(
            inputs=(
                InputBinding.file("script", self.script),
                InputBinding.symlink_tree("sdk", sdk, path_identity="content"),
            ),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="builder",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                "{input:sdk}/alias/header.txt",
                                "{phase_root}",
                                "|sdk",
                            ),
                        ),
                    ),
                    inputs=("script", "sdk"),
                ),
            ),
        )
        result = self.execute(plan, "symlink-tree")
        self.assertEqual(
            (result.phases[0].output_root / "result.txt").read_text(encoding="utf-8"),
            "bound-sdk|sdk",
        )
        pinned_alias = self.root / "runs/symlink-tree/bindings/inputs/sdk/symlink_tree/alias"
        self.assertTrue(pinned_alias.is_symlink())
        self.assertEqual(pinned_alias.readlink(), Path("real"))

    def test_symlink_tree_rejects_a_link_that_escapes_its_bound_root(self) -> None:
        sdk = self.inputs / "sdk"
        sdk.mkdir()
        (sdk / "escape").symlink_to(self.source)
        plan = ProofPlan(
            inputs=(
                InputBinding.file("script", self.script),
                InputBinding.symlink_tree("sdk", sdk, path_identity="content"),
            ),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="builder",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                "{input:sdk}/escape",
                                "{phase_root}",
                                "|unsafe",
                            ),
                        ),
                    ),
                    inputs=("script", "sdk"),
                ),
            ),
        )
        with self.assertRaisesRegex(EngineError, "escapes or cycles outside"):
            self.execute(plan, "escaping-symlink-tree")

    def test_target_profile_and_mutant_are_independent_cache_key_inputs(self) -> None:
        def keyed_plan(target: str, profile: str, mutant: str) -> ProofPlan:
            return ProofPlan(
                inputs=(
                    InputBinding.file("script", self.script),
                    InputBinding.file("source", self.source),
                    InputBinding.literal("target", target),
                    InputBinding.literal("profile", profile),
                    InputBinding.literal("mutant", mutant),
                ),
                tools=(ToolSpec("python", Path(sys.executable)),),
                phases=(
                    PhaseSpec(
                        name="compiled-mutant",
                        commands=(
                            CommandSpec(
                                tool="python",
                                args=(
                                    "{input:script}",
                                    "{input:source}",
                                    "{phase_root}",
                                    "|compiled",
                                ),
                            ),
                        ),
                        inputs=("script", "source", "target", "profile", "mutant"),
                        cache_mode="content",
                    ),
                ),
            )

        results = [
            self.execute(
                keyed_plan("aarch64-apple-darwin", "release", "same-name-run"),
                "key-baseline",
            ),
            self.execute(
                keyed_plan("x86_64-unknown-linux-gnu", "release", "same-name-run"),
                "key-target",
            ),
            self.execute(
                keyed_plan("aarch64-apple-darwin", "test", "same-name-run"),
                "key-profile",
            ),
            self.execute(
                keyed_plan("aarch64-apple-darwin", "release", "ceremony-replay"),
                "key-mutant",
            ),
        ]

        self.assertTrue(all(result.phases[0].cache_status == "miss" for result in results))
        self.assertEqual(len({result.phases[0].identity for result in results}), len(results))

    def test_tool_byte_change_invalidates_cache(self) -> None:
        tool = self.inputs / "fixture-tool"
        self._write_tool(tool, "one")
        plan = ProofPlan(
            inputs=(InputBinding.literal("target", "aarch64-apple-darwin"),),
            tools=(ToolSpec("fixture", tool),),
            phases=(
                PhaseSpec(
                    name="compile",
                    commands=(
                        CommandSpec(
                            tool="fixture",
                            args=("{phase_root}/result.txt",),
                            label="fixture-tool",
                        ),
                    ),
                    inputs=("target",),
                    cache_mode="content",
                ),
            ),
        )
        first = self.execute(plan, "tool-one")
        self._write_tool(tool, "two")
        second = self.execute(plan, "tool-two")

        self.assertEqual(first.phases[0].cache_status, "miss")
        self.assertEqual(second.phases[0].cache_status, "miss")
        self.assertNotEqual(first.phases[0].identity, second.phases[0].identity)
        self.assertEqual((second.phases[0].output_root / "result.txt").read_text(), "two")

    def test_tool_aba_substitution_cannot_change_the_executed_bytes(self) -> None:
        tool = self.inputs / "fixture-tool"
        barrier = self.root / "tool-aba-barrier"
        barrier.mkdir()
        original = (
            f"#!{sys.executable}\n"
            "from pathlib import Path\n"
            "import sys\n"
            "import time\n"
            "barrier = Path(sys.argv[1])\n"
            "(barrier / 'ready').write_text('ready', encoding='utf-8')\n"
            "while not (barrier / 'finish').is_file():\n"
            "    time.sleep(0.01)\n"
            "Path(sys.argv[2]).write_text('one', encoding='utf-8')\n"
        )
        substituted = original.replace("write_text('one'", "write_text('two'")
        tool.write_text(original, encoding="utf-8")
        tool.chmod(0o755)
        plan = ProofPlan(
            inputs=(InputBinding.literal("target", "aarch64-apple-darwin"),),
            tools=(ToolSpec("fixture", tool),),
            phases=(
                PhaseSpec(
                    name="compile",
                    commands=(
                        CommandSpec(
                            tool="fixture",
                            args=(str(barrier), "{phase_root}/result.txt"),
                            label="fixture-tool-aba",
                        ),
                    ),
                    inputs=("target",),
                    cache_mode="disabled",
                ),
            ),
        )
        with ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(self.execute, plan, "tool-aba")
            self._wait_for(barrier / "ready")
            tool.write_text(substituted, encoding="utf-8")
            tool.chmod(0o755)
            tool.write_text(original, encoding="utf-8")
            tool.chmod(0o755)
            (barrier / "finish").write_text("finish", encoding="utf-8")
            result = future.result(timeout=5)

        self.assertEqual(
            (result.phases[0].output_root / "result.txt").read_text(encoding="utf-8"),
            "one",
        )

    def test_corrupt_checkpoint_fails_closed(self) -> None:
        plan = self.plan()
        first = self.execute(plan, "first")
        payload = (
            self.cache
            / "objects"
            / first.phases[0].identity.removeprefix("sha256:")
            / "payload"
            / "result.txt"
        )
        payload.chmod(0o644)
        payload.write_text("corrupt", encoding="utf-8")

        with self.assertRaises(CacheCorruptionError):
            self.execute(plan, "second")

        unsafe_cache = self.root / "unsafe-cache"
        unsafe_cache.mkdir()
        escaped = self.root / "escaped-cache-objects"
        escaped.mkdir()
        (unsafe_cache / "objects").symlink_to(escaped, target_is_directory=True)
        with self.assertRaises(CacheCorruptionError):
            self.engine.execute(
                plan,
                run_root=self.root / "runs/symlinked-cache-objects",
                cache_root=unsafe_cache,
                run_token="symlinked-cache-objects",
            )
        self.assertEqual(list(escaped.iterdir()), [])

    def test_substituted_checkpoint_binding_fails_closed(self) -> None:
        plan = self.plan()
        first = self.execute(plan, "first")
        checkpoint_path = (
            self.cache
            / "objects"
            / first.phases[0].identity.removeprefix("sha256:")
            / "checkpoint.json"
        )
        checkpoint = json.loads(checkpoint_path.read_text(encoding="ascii"))
        checkpoint_path.chmod(0o644)
        checkpoint["phase_spec_identity"] = "sha256:" + "0" * 64
        unsigned = {key: value for key, value in checkpoint.items() if key != "integrity"}
        canonical = (
            json.dumps(unsigned, sort_keys=True, separators=(",", ":"), ensure_ascii=True)
            + "\n"
        ).encode("ascii")
        import hashlib

        checkpoint["integrity"] = f"sha256:{hashlib.sha256(canonical).hexdigest()}"
        checkpoint_path.write_text(
            json.dumps(checkpoint, sort_keys=True, separators=(",", ":")) + "\n",
            encoding="ascii",
        )

        with self.assertRaises(CacheCorruptionError):
            self.execute(plan, "second")

    def test_resealed_checkpoint_payload_is_rejected(self) -> None:
        from tools.vnext_contracts.proof_engine import engine as engine_module

        plan = self.plan()
        first = self.execute(plan, "first")
        root = (
            self.cache
            / "objects"
            / first.phases[0].identity.removeprefix("sha256:")
        )
        payload = root / "payload"
        result = payload / "result.txt"
        checkpoint_path = root / "checkpoint.json"
        root.chmod(0o755)
        payload.chmod(0o755)
        result.chmod(0o644)
        checkpoint_path.chmod(0o644)
        result.write_text("substituted-output", encoding="utf-8")
        checkpoint = json.loads(checkpoint_path.read_text(encoding="ascii"))
        checkpoint["output_manifest"] = engine_module._output_manifest(payload)
        unsigned = {key: value for key, value in checkpoint.items() if key != "integrity"}
        checkpoint["integrity"] = engine_module._digest(
            engine_module._canonical_bytes(unsigned)
        )
        checkpoint_path.write_bytes(engine_module._canonical_bytes(checkpoint))
        engine_module._freeze_tree(root)

        with self.assertRaises(CacheCorruptionError):
            self.execute(plan, "second")

    def test_command_cannot_reference_an_input_omitted_from_phase_identity(self) -> None:
        plan = self.plan()
        phase = plan.phases[0]
        unbound = ProofPlan(
            inputs=plan.inputs,
            tools=plan.tools,
            phases=(
                PhaseSpec(
                    name=phase.name,
                    commands=phase.commands,
                    inputs=("script", "fact"),
                    cache_mode=phase.cache_mode,
                ),
            ),
        )

        with self.assertRaises(PlanError):
            self.execute(unbound, "unbound-input")

    def test_concurrent_same_identity_different_results_fail_closed(self) -> None:
        racer = self.inputs / "nondeterministic-race.py"
        racer.write_text(NONDETERMINISTIC_RACE_SCRIPT + "\n", encoding="utf-8")
        barrier = self.root / "barrier"
        barrier.mkdir()
        plan = ProofPlan(
            inputs=(InputBinding.file("script", racer),),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="racer",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=("{input:script}", "{phase_root}", str(barrier)),
                        ),
                    ),
                    inputs=("script",),
                    cache_mode="content",
                ),
            ),
        )

        outcomes: list[object] = []
        with ThreadPoolExecutor(max_workers=2) as executor:
            futures = [
                executor.submit(self.execute, plan, name)
                for name in ("race-one", "race-two")
            ]
            for future in as_completed(futures):
                try:
                    outcomes.append(future.result())
                except BaseException as error:
                    outcomes.append(error)
        self.assertEqual(sum(isinstance(value, CacheCorruptionError) for value in outcomes), 1)
        self.assertEqual(sum(not isinstance(value, BaseException) for value in outcomes), 1)

    def test_same_plan_and_run_token_execute_exactly_once(self) -> None:
        script = self.inputs / "count.py"
        script.write_text(COUNT_SCRIPT + "\n", encoding="utf-8")
        counter = self.root / "command-count.txt"
        plan = ProofPlan(
            inputs=(InputBinding.file("script", script),),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="count",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=("{input:script}", "{phase_root}", str(counter)),
                        ),
                    ),
                    inputs=("script",),
                    cache_mode="disabled",
                ),
            ),
        )
        outcomes: list[object] = []
        with ThreadPoolExecutor(max_workers=2) as executor:
            futures = [
                executor.submit(self.execute, plan, name, run_token="one-seal")
                for name in ("same-token-one", "same-token-two")
            ]
            for future in as_completed(futures):
                try:
                    outcomes.append(future.result())
                except BaseException as error:
                    outcomes.append(error)
        self.assertEqual(sum(isinstance(value, PlanError) for value in outcomes), 1)
        self.assertEqual(sum(not isinstance(value, BaseException) for value in outcomes), 1)
        self.assertEqual(counter.read_text(encoding="utf-8").splitlines(), ["run"])

    def test_command_reads_immutable_input_snapshot_during_live_path_aba(self) -> None:
        script = self.inputs / "pinned-input.py"
        script.write_text(PINNED_INPUT_SCRIPT + "\n", encoding="utf-8")
        barrier = self.root / "aba-barrier"
        barrier.mkdir()
        plan = ProofPlan(
            inputs=(
                InputBinding.file("script", script),
                InputBinding.file("source", self.source),
            ),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="aba",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                "{input:source}",
                                "{phase_root}",
                                str(barrier),
                            ),
                        ),
                    ),
                    inputs=("script", "source"),
                    cache_mode="disabled",
                ),
            ),
        )
        with ThreadPoolExecutor(max_workers=1) as executor:
            future = executor.submit(self.execute, plan, "input-aba")
            self._wait_for(barrier / "ready")
            original = self.source.read_bytes()
            self.source.write_text("substituted-live-input", encoding="utf-8")
            (barrier / "read").write_text("read", encoding="utf-8")
            self._wait_for(barrier / "consumed")
            self.source.write_bytes(original)
            (barrier / "finish").write_text("finish", encoding="utf-8")
            result = future.result()
        self.assertEqual(
            (result.phases[0].output_root / "result.txt").read_text(encoding="utf-8"),
            "source-v1",
        )

    def test_input_mutation_is_rejected_before_checkpoint_seal(self) -> None:
        mutator = self.inputs / "mutate.py"
        mutator.write_text(MUTATE_SCRIPT + "\n", encoding="utf-8")

        with self.assertRaises(EngineError):
            self.execute(self.plan(script=mutator), "mutator")
        objects = self.cache / "objects"
        self.assertFalse(objects.exists() and any(objects.iterdir()))

    def test_independent_phases_run_in_parallel_with_disjoint_roots(self) -> None:
        parallel_script = self.inputs / "parallel.py"
        parallel_script.write_text(PARALLEL_SCRIPT + "\n", encoding="utf-8")
        barrier = self.root / "parallel-barrier"
        barrier.mkdir()
        plan = ProofPlan(
            inputs=(InputBinding.file("script", parallel_script),),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="python",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                "{phase_root}",
                                str(barrier),
                                "python",
                            ),
                        ),
                    ),
                    inputs=("script",),
                    cache_mode="disabled",
                ),
                PhaseSpec(
                    name="ruby",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:script}",
                                "{phase_root}",
                                str(barrier),
                                "ruby",
                            ),
                        ),
                    ),
                    inputs=("script",),
                    cache_mode="disabled",
                ),
            ),
        )
        result = self.execute(plan, "parallel", max_workers=2)

        self.assertEqual([phase.name for phase in result.phases], ["python", "ruby"])
        self.assertTrue(all((phase.output_root / "done").is_file() for phase in result.phases))

    def test_run_root_placeholder_is_rejected_to_preserve_phase_isolation(self) -> None:
        phase = self.plan().phases[0]
        plan = ProofPlan(
            inputs=self.plan().inputs,
            tools=self.plan().tools,
            phases=(
                PhaseSpec(
                    name=phase.name,
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=("{input:script}", "{run_root}/peer"),
                        ),
                    ),
                    inputs=("script",),
                ),
            ),
        )
        with self.assertRaises(PlanError):
            self.execute(plan, "run-root-placeholder")

    def test_child_phase_cannot_mutate_completed_dependency_output(self) -> None:
        builder_script = self.inputs / "dependency-builder.py"
        builder_script.write_text(DEPENDENCY_BUILDER_SCRIPT + "\n", encoding="utf-8")
        mutator_script = self.inputs / "dependency-mutator.py"
        mutator_script.write_text(DEPENDENCY_MUTATOR_SCRIPT + "\n", encoding="utf-8")
        plan = ProofPlan(
            inputs=(
                InputBinding.file("builder-script", builder_script),
                InputBinding.file("mutator-script", mutator_script),
            ),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="builder",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=("{input:builder-script}", "{phase_root}"),
                        ),
                    ),
                    inputs=("builder-script",),
                    cache_mode="run",
                ),
                PhaseSpec(
                    name="validator",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=(
                                "{input:mutator-script}",
                                "{dependency:builder}",
                                "{phase_root}",
                            ),
                        ),
                    ),
                    inputs=("mutator-script",),
                    dependencies=("builder",),
                    cache_mode="run",
                ),
            ),
        )

        with self.assertRaises(EngineError):
            self.execute(plan, "dependency-mutator", run_token="dependency-seal")

    def test_success_publishes_content_addressed_release_with_one_atomic_pointer(self) -> None:
        release_root = self.root / "releases"
        pointer = self.root / "active-proof.json"
        publication = PublicationSpec(
            release_root=release_root,
            pointer_path=pointer,
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        result = self.execute(self.plan(publication=publication), "publish")

        pointer_value = json.loads(pointer.read_text(encoding="ascii"))
        self.assertEqual(pointer_value["release_identity"], result.publication_identity)
        published = release_root / pointer_value["object"] / "payload/proof/result.txt"
        self.assertEqual(published.read_text(encoding="utf-8"), "source-v1|default")
        release = json.loads(
            (release_root / pointer_value["object"] / "release.json").read_text(
                encoding="ascii"
            )
        )["canonical_value"]
        self.assertEqual(release["run_token"], result.run_token)
        self.assertEqual(release["plan_identity"], result.plan_identity)
        self.assertEqual(release["plan"]["schema_version"], "maestro.vnext.proof-engine.v1")
        self.assertEqual(release["phase_receipts"][0]["name"], "builder")
        self.assertTrue(release["phase_receipts"][0]["command_receipts"])
        self.assertNotIn("cache_status", release["phase_receipts"][0])
        release_object = release_root / pointer_value["object"]
        self.assertFalse(release_object.stat().st_mode & 0o222)
        self.assertTrue(
            all(not path.stat().st_mode & 0o222 for path in release_object.rglob("*"))
        )

        unsafe_release_root = self.root / "unsafe-releases"
        unsafe_release_root.mkdir()
        escaped = self.root / "escaped-release-objects"
        escaped.mkdir()
        (unsafe_release_root / "objects").symlink_to(escaped, target_is_directory=True)
        unsafe_publication = PublicationSpec(
            release_root=unsafe_release_root,
            pointer_path=self.root / "unsafe-proof.json",
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        with self.assertRaises(PublicationError):
            self.execute(self.plan(publication=unsafe_publication), "publish-symlinked-objects")
        self.assertEqual(list(escaped.iterdir()), [])

    def test_failure_leaves_existing_publication_pointer_untouched(self) -> None:
        pointer = self.root / "active-proof.json"
        original = b'{"release_identity":"old"}\n'
        pointer.write_bytes(original)
        failing = self.inputs / "fail.py"
        failing.write_text("raise SystemExit(7)\n", encoding="utf-8")
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=pointer,
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )

        with self.assertRaises(EngineError):
            self.execute(self.plan(script=failing, publication=publication), "failure")
        self.assertEqual(pointer.read_bytes(), original)

    def test_post_pointer_crash_resumes_without_republishing(self) -> None:
        class CrashAfterPublicationEngine(ProofEngine):
            @staticmethod
            def _complete_run(
                cache_root: Path,
                plan_identity: str,
                run_token: str,
                phases: Sequence[PhaseResult],
                publication_identity: str | None,
            ) -> None:
                raise EngineError("injected crash after pointer publication")

        pointer = self.root / "active-proof.json"
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=pointer,
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        plan = self.plan(cache_mode="run", publication=publication)
        run_root = self.root / "runs/post-pointer-crash"
        with self.assertRaisesRegex(EngineError, "injected crash"):
            CrashAfterPublicationEngine().execute(
                plan,
                run_root=run_root,
                cache_root=self.cache,
                run_token="post-pointer-crash",
            )
        published = pointer.read_bytes()
        resumed = self.engine.execute(
            plan,
            run_root=run_root,
            cache_root=self.cache,
            run_token="post-pointer-crash",
        )
        self.assertEqual(pointer.read_bytes(), published)
        self.assertEqual(resumed.phases[0].cache_status, "hit")

    def test_interrupted_run_cannot_redirect_publication_destination(self) -> None:
        class CrashAfterPublicationEngine(ProofEngine):
            @staticmethod
            def _complete_run(
                cache_root: Path,
                plan_identity: str,
                run_token: str,
                phases: Sequence[PhaseResult],
                publication_identity: str | None,
            ) -> None:
                raise EngineError("injected crash after pointer publication")

        first = PublicationSpec(
            release_root=self.root / "first-releases",
            pointer_path=self.root / "first-proof.json",
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        redirected = PublicationSpec(
            release_root=self.root / "redirected-releases",
            pointer_path=self.root / "redirected-proof.json",
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        run_root = self.root / "runs/redirected-publication"
        with self.assertRaisesRegex(EngineError, "injected crash"):
            CrashAfterPublicationEngine().execute(
                self.plan(cache_mode="run", publication=first),
                run_root=run_root,
                cache_root=self.cache,
                run_token="redirected-publication",
            )

        with self.assertRaisesRegex(PlanError, "another proof plan"):
            self.engine.execute(
                self.plan(cache_mode="run", publication=redirected),
                run_root=run_root,
                cache_root=self.cache,
                run_token="redirected-publication",
            )
        self.assertFalse(redirected.pointer_path.exists())

    def test_old_post_pointer_crash_cannot_regress_a_newer_pointer(self) -> None:
        class CrashAfterPublicationEngine(ProofEngine):
            @staticmethod
            def _complete_run(
                cache_root: Path,
                plan_identity: str,
                run_token: str,
                phases: Sequence[PhaseResult],
                publication_identity: str | None,
            ) -> None:
                raise EngineError("injected crash after pointer publication")

        pointer = self.root / "active-proof.json"
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=pointer,
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        plan = self.plan(cache_mode="run", publication=publication)
        old_root = self.root / "runs/old-post-pointer-crash"
        with self.assertRaisesRegex(EngineError, "injected crash"):
            CrashAfterPublicationEngine().execute(
                plan,
                run_root=old_root,
                cache_root=self.cache,
                run_token="old-post-pointer-crash",
            )
        newer = self.engine.execute(
            plan,
            run_root=self.root / "runs/newer-proof",
            cache_root=self.cache,
            run_token="newer-proof",
        )
        newer_pointer = pointer.read_bytes()
        with self.assertRaisesRegex(PublicationError, "pointer advanced"):
            self.engine.execute(
                plan,
                run_root=old_root,
                cache_root=self.cache,
                run_token="old-post-pointer-crash",
            )
        self.assertEqual(pointer.read_bytes(), newer_pointer)
        self.assertEqual(
            json.loads(newer_pointer)["release_identity"], newer.publication_identity
        )

    def test_publication_rejects_output_changed_after_phase_completion(self) -> None:
        class MutatingPublicationEngine(ProofEngine):
            def _publish(
                self,
                publication: PublicationSpec,
                plan_identity: str,
                plan_value: Mapping[str, Any],
                run_token: str,
                phases: Sequence[PhaseResult],
                initial_pointer_state: Mapping[str, Any] | None,
            ) -> str:
                (phases[0].output_root / "result.txt").write_text(
                    "changed-after-phase", encoding="utf-8"
                )
                return super()._publish(
                    publication,
                    plan_identity,
                    plan_value,
                    run_token,
                    phases,
                    initial_pointer_state,
                )

        pointer = self.root / "active-proof.json"
        original = b'{"release_identity":"old"}\n'
        pointer.write_bytes(original)
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=pointer,
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        engine = MutatingPublicationEngine()

        with self.assertRaises((InputMutationError, PermissionError)):
            engine.execute(
                self.plan(publication=publication),
                run_root=self.root / "runs/publication-mutation",
                cache_root=self.cache,
            )
        self.assertEqual(pointer.read_bytes(), original)

    def test_phase_temp_is_isolated_and_excluded_from_sealed_output(self) -> None:
        script = self.inputs / "phase-temp.py"
        script.write_text(PHASE_TEMP_SCRIPT + "\n", encoding="utf-8")
        plan = ProofPlan(
            inputs=(InputBinding.file("script", script),),
            tools=(ToolSpec("python", Path(sys.executable)),),
            phases=(
                PhaseSpec(
                    name="temporary-work",
                    commands=(
                        CommandSpec(
                            tool="python",
                            args=("{input:script}", "{phase_temp}", "{phase_root}"),
                            label="isolated-temporary-work",
                        ),
                    ),
                    inputs=("script",),
                    cache_mode="run",
                ),
            ),
        )

        result = self.execute(plan, "phase-temp", run_token="phase-temp-token")

        self.assertEqual(
            (result.phases[0].output_root / "result.txt").read_text(encoding="utf-8"),
            "done",
        )
        self.assertFalse((result.phases[0].output_root.parent / "tmp").exists())

    def test_overlapping_publication_destinations_are_rejected(self) -> None:
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=self.root / "active-proof.json",
            outputs=(
                PublishedOutput("builder", "result.txt", "proof"),
                PublishedOutput("builder", "result.txt", "proof/result.txt"),
            ),
        )

        with self.assertRaises(PlanError):
            self.execute(self.plan(publication=publication), "overlap")

    def test_performance_log_cannot_overlap_run_or_publication_paths(self) -> None:
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=self.root / "active-proof.json",
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        plan = self.plan(publication=publication)
        with self.assertRaises(PlanError):
            self.engine.execute(
                plan,
                run_root=self.root / "runs/performance-overlap",
                cache_root=self.cache,
                performance_log=self.root / "runs/performance-overlap/performance.jsonl",
            )
        with self.assertRaises(PlanError):
            self.engine.execute(
                plan,
                run_root=self.root / "runs/performance-publication",
                cache_root=self.cache,
                performance_log=self.root / "releases/performance.jsonl",
            )

    @staticmethod
    def _write_tool(path: Path, value: str) -> None:
        path.write_text(
            f"#!{sys.executable}\n"
            "from pathlib import Path\n"
            "import sys\n"
            f"Path(sys.argv[1]).write_text({value!r}, encoding='utf-8')\n",
            encoding="utf-8",
        )
        path.chmod(0o755)

    @staticmethod
    def _wait_for(path: Path) -> None:
        deadline = time.monotonic() + 5
        while not path.exists():
            if time.monotonic() >= deadline:
                raise AssertionError(f"timed out waiting for {path}")
            time.sleep(0.01)


if __name__ == "__main__":
    unittest.main()
