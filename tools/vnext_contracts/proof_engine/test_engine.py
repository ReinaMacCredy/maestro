from __future__ import annotations

import json
import sys
import tempfile
import unittest
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Sequence

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
other = Path(sys.argv[2])
(own / "ready").write_text("ready", encoding="utf-8")
deadline = time.monotonic() + 3
while not (other / "ready").is_file():
    if time.monotonic() >= deadline:
        raise SystemExit("peer phase did not start concurrently")
    time.sleep(0.01)
(own / "done").write_text("done", encoding="utf-8")
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
            performance_log=run_root / "performance.v1.jsonl" if performance else None,
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
            for line in (self.root / "runs/second/performance.v1.jsonl")
            .read_text(encoding="ascii")
            .splitlines()
        ]
        self.assertEqual(events[-1]["cache_status"], "hit")
        self.assertIn("duration_ms", events[-1])

    def test_run_cache_resumes_one_seal_but_not_a_new_seal(self) -> None:
        plan = self.plan(cache_mode="run")
        first = self.execute(plan, "seal-a-first", run_token="seal-a")
        resumed = self.execute(plan, "seal-a-resumed", run_token="seal-a")
        new_seal = self.execute(plan, "seal-b", run_token="seal-b")

        self.assertEqual(first.phases[0].cache_status, "miss")
        self.assertEqual(resumed.phases[0].cache_status, "hit")
        self.assertEqual(new_seal.phases[0].cache_status, "miss")
        self.assertEqual(first.phases[0].identity, resumed.phases[0].identity)
        self.assertNotEqual(first.phases[0].identity, new_seal.phases[0].identity)

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
        payload.write_text("corrupt", encoding="utf-8")

        with self.assertRaises(CacheCorruptionError):
            self.execute(plan, "second")

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

    def test_input_mutation_is_rejected_before_checkpoint_seal(self) -> None:
        mutator = self.inputs / "mutate.py"
        mutator.write_text(MUTATE_SCRIPT + "\n", encoding="utf-8")

        with self.assertRaises(InputMutationError):
            self.execute(self.plan(script=mutator), "mutator")
        objects = self.cache / "objects"
        self.assertFalse(objects.exists() and any(objects.iterdir()))

    def test_independent_phases_run_in_parallel_with_disjoint_roots(self) -> None:
        parallel_script = self.inputs / "parallel.py"
        parallel_script.write_text(PARALLEL_SCRIPT + "\n", encoding="utf-8")
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
                                "{run_root}/phases/ruby/output",
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
                                "{run_root}/phases/python/output",
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

        with self.assertRaises(InputMutationError):
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

    def test_publication_rejects_output_changed_after_phase_completion(self) -> None:
        class MutatingPublicationEngine(ProofEngine):
            def _publish(
                self,
                publication: PublicationSpec,
                plan_identity: str,
                phases: Sequence[PhaseResult],
            ) -> str:
                (phases[0].output_root / "result.txt").write_text(
                    "changed-after-phase", encoding="utf-8"
                )
                return super()._publish(publication, plan_identity, phases)

        pointer = self.root / "active-proof.json"
        original = b'{"release_identity":"old"}\n'
        pointer.write_bytes(original)
        publication = PublicationSpec(
            release_root=self.root / "releases",
            pointer_path=pointer,
            outputs=(PublishedOutput("builder", "result.txt", "proof/result.txt"),),
        )
        engine = MutatingPublicationEngine()

        with self.assertRaises(InputMutationError):
            engine.execute(
                self.plan(publication=publication),
                run_root=self.root / "runs/publication-mutation",
                cache_root=self.cache,
            )
        self.assertEqual(pointer.read_bytes(), original)

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


if __name__ == "__main__":
    unittest.main()
