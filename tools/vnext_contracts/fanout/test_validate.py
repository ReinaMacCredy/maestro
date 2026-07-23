from __future__ import annotations

import copy
import hashlib
import json
import os
import sqlite3
import stat
import subprocess
import sys
import tempfile
import unittest
import zlib
from contextlib import closing
from pathlib import Path

from tools.vnext_contracts.fanout import validate


class FanoutOwnershipTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls) -> None:
        cls._fixture_directory = tempfile.TemporaryDirectory()
        cls._certified_fixture = Path(cls._fixture_directory.name) / "certified.git"
        workspace = Path(__file__).resolve().parents[3]
        subprocess.run(
            ["git", "init", "--bare", "--quiet", str(cls._certified_fixture)],
            check=True,
            capture_output=True,
        )
        subprocess.run(
            [
                "git",
                "-C",
                str(cls._certified_fixture),
                "fetch",
                "--quiet",
                "--no-tags",
                f"file://{workspace}",
                f"{validate.CERTIFIED_STAGE5['commit']}:refs/heads/certified",
            ],
            check=True,
            capture_output=True,
        )

    @classmethod
    def tearDownClass(cls) -> None:
        cls._fixture_directory.cleanup()

    def setUp(self) -> None:
        self.manifest = validate.load_manifest()

    def test_frozen_manifest_has_exact_nonoverlapping_stage_owners(self) -> None:
        validate.validate_manifest(self.manifest)

    def test_fanout_base_set_is_manifest_shared_files_plus_exact_seeds(self) -> None:
        shared = dict(self.manifest["fanout_base"]["orchestrator_owned_files"])
        seeds = {
            seed
            for owner in self.manifest["stage_owners"]
            for seed in owner["mutable_seed_files"]
        }
        inherited_seeds = {
            seed
            for owner in self.manifest["stage_owners"]
            for seed in owner["inherited_mutable_seed_files"]
        }
        canonical_inputs = {path for path, _, _ in validate.CANONICAL_INPUTS}
        self.assertEqual(len(shared), 68)
        self.assertEqual(len(seeds), 28)
        self.assertEqual(
            inherited_seeds,
            {
                "src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs",
                "src/domain/vnext/integration/mod.rs",
                "src/domain/vnext/integration/trusted_host_diagnostic_stage10_seed.rs",
                "src/domain/vnext/persistence/mod.rs",
                "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs",
            },
        )
        self.assertEqual(
            {path for path in canonical_inputs if shared.get(path) == "A"},
            canonical_inputs,
        )
        self.assertTrue(set(shared).isdisjoint(seeds))
        self.assertTrue(set(shared).isdisjoint(inherited_seeds))
        self.assertTrue(seeds.isdisjoint(inherited_seeds))
        self.assertEqual(len(validate.expected_fanout_base_changes(self.manifest)), 96)
        self.assertTrue(
            inherited_seeds.isdisjoint(
                validate.expected_fanout_base_changes(self.manifest)
            )
        )
        self.assertEqual(
            validate.expected_fanout_base_changes(self.manifest),
            shared | {seed: "A" for seed in seeds},
        )

    def test_canonical_input_exports_match_exact_paths_bytes_and_modes(self) -> None:
        workspace = Path(__file__).resolve().parents[3]
        expected_paths = {
            validate.DESIGN_PATH,
            *(
                f".maestro/cards/{card_id}/card.yaml"
                for card_id, _, _ in validate.SUCCESSOR_DECISIONS
            ),
        }
        self.assertEqual(
            {path for path, _, _ in validate.CANONICAL_INPUTS},
            expected_paths,
        )
        for path, expected_sha256, expected_byte_length in validate.CANONICAL_INPUTS:
            target = workspace / path
            with self.subTest(path=path):
                metadata = target.lstat()
                self.assertTrue(stat.S_ISREG(metadata.st_mode))
                self.assertEqual(stat.S_IMODE(metadata.st_mode), 0o644)
                contents = target.read_bytes()
                self.assertEqual(len(contents), expected_byte_length)
                self.assertEqual(hashlib.sha256(contents).hexdigest(), expected_sha256)

    def test_design_decisions_and_certified_receipt_bindings_are_exact(self) -> None:
        mutants = []
        wrong_design = copy.deepcopy(self.manifest)
        wrong_design["design"]["sha256"] = "0" * 64
        mutants.append(wrong_design)
        missing_decision = copy.deepcopy(self.manifest)
        missing_decision["successor_decisions"].pop()
        mutants.append(missing_decision)
        wrong_decision = copy.deepcopy(self.manifest)
        wrong_decision["successor_decisions"][0]["status"] = "superseded"
        mutants.append(wrong_decision)
        wrong_release = copy.deepcopy(self.manifest)
        wrong_release["certified_stage5"]["release_identity"] = "not-an-identity"
        mutants.append(wrong_release)
        for mutant in mutants:
            with self.subTest(mutant=mutant), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.validate_manifest(mutant)

    def test_inherited_seed_policy_rejects_missing_substituted_and_overlapping_paths(
        self,
    ) -> None:
        stage8_index = next(
            index
            for index, row in enumerate(self.manifest["stage_owners"])
            if row["stage"] == 8
        )
        stage9_index = next(
            index
            for index, row in enumerate(self.manifest["stage_owners"])
            if row["stage"] == 9
        )
        stage8_seed = self.manifest["stage_owners"][stage8_index][
            "inherited_mutable_seed_files"
        ][0]

        missing = copy.deepcopy(self.manifest)
        missing["stage_owners"][stage8_index]["inherited_mutable_seed_files"].clear()

        substituted = copy.deepcopy(self.manifest)
        substituted["stage_owners"][stage9_index]["inherited_mutable_seed_files"][0] = (
            "src/domain/vnext/persistence/nonexistent_stage9_seed.rs"
        )

        nonexistent = copy.deepcopy(self.manifest)
        nonexistent["stage_owners"][stage8_index]["inherited_mutable_seed_files"].append(
            "src/domain/vnext/authority/nonexistent_stage8_seed.rs"
        )

        overlapping = copy.deepcopy(self.manifest)
        overlapping["stage_owners"][stage9_index]["inherited_mutable_seed_files"].append(
            stage8_seed
        )

        for name, mutant in (
            ("missing", missing),
            ("substituted", substituted),
            ("nonexistent", nonexistent),
            ("overlapping", overlapping),
        ):
            with self.subTest(name=name), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.validate_manifest(mutant)

    def test_every_certified_stage5_field_rejects_valid_same_shape_substitution(self) -> None:
        replacements = {
            "commit": "0" * 40,
            "tree": "1" * 40,
            "publication_pointer": (
                "contracts/vnext/stage5/evidence-gates/alternate-proof.json"
            ),
            "release_identity": f"sha256:{'2' * 64}",
            "plan_identity": f"sha256:{'3' * 64}",
            "snapshot_identity": f"sha256:{'4' * 64}",
        }
        for field, replacement in replacements.items():
            mutant = copy.deepcopy(self.manifest)
            mutant["certified_stage5"][field] = replacement
            with self.subTest(field=field), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.validate_manifest(mutant)

    def test_every_security_policy_field_is_exact(self) -> None:
        for section in ("authority", "scheduling", "path_policy"):
            for field, value in self.manifest[section].items():
                mutant = copy.deepcopy(self.manifest)
                mutant[section][field] = self._same_shape_substitution(value)
                with self.subTest(section=section, field=field), self.assertRaises(
                    validate.FanoutValidationError
                ):
                    validate.validate_manifest(mutant)

        for field, value in self.manifest["fanout_base"][
            "orchestrator_owned_files"
        ].items():
            mutant = copy.deepcopy(self.manifest)
            mutant["fanout_base"]["orchestrator_owned_files"][field] = (
                "A" if value == "M" else "M"
            )
            with self.subTest(section="fanout_base", field=field), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.validate_manifest(mutant)

        for index, value in enumerate(self.manifest["frozen_interface_roots"]):
            mutant = copy.deepcopy(self.manifest)
            mutant["frozen_interface_roots"][index] = f"{value}mutant/"
            with self.subTest(
                section="frozen_interface_roots", index=index
            ), self.assertRaises(validate.FanoutValidationError):
                validate.validate_manifest(mutant)

        for field, values in self.manifest["shared_denylist"].items():
            for index, value in enumerate(values):
                mutant = copy.deepcopy(self.manifest)
                mutant["shared_denylist"][field][index] = f"{value}mutant"
                with self.subTest(
                    section="shared_denylist", field=field, index=index
                ), self.assertRaises(validate.FanoutValidationError):
                    validate.validate_manifest(mutant)

        for index, row in enumerate(self.manifest["stage_owners"]):
            for field, value in row.items():
                mutant = copy.deepcopy(self.manifest)
                mutant["stage_owners"][index][field] = self._same_shape_substitution(
                    value
                )
                with self.subTest(stage=row["stage"], field=field), self.assertRaises(
                    validate.FanoutValidationError
                ):
                    validate.validate_manifest(mutant)

        extra_field = copy.deepcopy(self.manifest)
        extra_field["authority"]["worker_release_authority"] = False
        with self.assertRaises(validate.FanoutValidationError):
            validate.validate_manifest(extra_field)

    def test_repository_paths_reject_metadata_controls_and_portable_aliases(self) -> None:
        attacks = (
            ".git/config",
            "src/domain/vnext/projection/.GiT/config",
            "tests/vnext_stage6_packet.rs\nCargo.toml",
            "src/domain/vnext/projection/back\\slash.rs",
            "src/domain/vnext/projection/nul\x00alias.rs",
            "src/domain/vnext/projection/trailing-space.rs ",
            "src/domain/vnext/projection/trailing-dot.rs.",
            "src/domain/vnext/projection/Cafe\u0301.rs",
            "src/domain/vnext/projection/Ｆullwidth.rs",
        )
        for path in attacks:
            with self.subTest(path=path), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.normalized_path(path, prefix=False)

        self.assertEqual(
            validate.normalized_path(
                "embedded/vnext/bootstrap/skills/maestro/SKILL.md", prefix=False
            ),
            "embedded/vnext/bootstrap/skills/maestro/SKILL.md",
        )

    def test_tree_keys_reject_case_normalization_and_directory_alias_collisions(
        self,
    ) -> None:
        attacks = (
            (
                "src/domain/vnext/projection/Packet.rs",
                "src/domain/vnext/projection/packet.rs",
            ),
            (
                "src/domain/vnext/projection/Straße.rs",
                "src/domain/vnext/projection/STRASSE.rs",
            ),
            (
                "src/domain/vnext/projection/File.rs",
                "src/domain/vnext/Projection/nested.rs",
            ),
        )
        for paths in attacks:
            with self.subTest(paths=paths), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.validate_tree_path_keys(paths, "attacked tree")

    def test_owned_seed_and_new_path_are_accepted(self) -> None:
        accepted = validate.validate_changes(
            self.manifest,
            6,
            [
                ("M", "src/domain/vnext/projection/mod.rs", "100644", "100644"),
                ("A", "src/domain/vnext/projection/packet.rs", "000000", "100644"),
                ("A", "tests/vnext_stage6_projection.rs", "000000", "100644"),
              ],
              {"src/domain/vnext/projection/mod.rs"},
          )
        self.assertEqual(
            accepted,
            [
                "src/domain/vnext/projection/mod.rs",
                "src/domain/vnext/projection/packet.rs",
                "tests/vnext_stage6_projection.rs",
            ],
        )

    def test_inherited_seed_change_shapes_and_stage_scope_are_exact(self) -> None:
        for owner in self.manifest["stage_owners"]:
            stage = owner["stage"]
            non_owner = 6 if stage != 6 else 7
            for inherited_seed in owner["inherited_mutable_seed_files"]:
                attacks = (
                    (
                        "add",
                        stage,
                        [("A", inherited_seed, "000000", "100644")],
                        {inherited_seed},
                        "requires status M",
                    ),
                    (
                        "delete",
                        stage,
                        [("D", inherited_seed, "100644", "000000")],
                        {inherited_seed},
                        "forbidden diff status",
                    ),
                    (
                        "mode change",
                        stage,
                        [("M", inherited_seed, "100644", "100755")],
                        {inherited_seed},
                        "object type or mode",
                    ),
                    (
                        "non-owner",
                        non_owner,
                        [("M", inherited_seed, "100644", "100644")],
                        {inherited_seed},
                        "touched shared path",
                    ),
                )
                for name, candidate_stage, changes, existing, expected_error in attacks:
                    with self.subTest(
                        seed=inherited_seed,
                        name=name,
                    ), self.assertRaisesRegex(
                        validate.FanoutValidationError, expected_error
                    ):
                        validate.validate_changes(
                            self.manifest,
                            candidate_stage,
                            changes,
                            existing,
                        )

        with self.assertRaisesRegex(
            validate.FanoutValidationError, "touched shared path"
        ):
            validate.validate_changes(
                self.manifest,
                10,
                [
                    (
                        "A",
                        "src/domain/vnext/integration/other.rs",
                        "000000",
                        "100644",
                    )
                ],
                set(),
            )

    def test_real_stage8_stage9_and_stage10_candidates_modify_exact_inherited_seeds(
        self,
    ) -> None:
        for stage in (8, 9, 10):
            with self.subTest(stage=stage), tempfile.TemporaryDirectory() as directory:
                repository, fanout_commit = self._fanout_repository(Path(directory))
                comparison_commit = self._comparison_before_stage(repository, stage)
                inherited_seeds = sorted(
                    validate.owner_for_stage(self.manifest, stage)[
                        "inherited_mutable_seed_files"
                    ]
                )
                candidate_commit = self._commit_paths(
                    repository,
                    {
                        path: (repository / path).read_bytes()
                        + f"\nstage{stage} owner\n".encode()
                        for path in inherited_seeds
                    },
                    f"stage{stage} inherited seed candidate",
                )

                result = validate.validate_candidate(
                    repository,
                    validate.MANIFEST_PATH,
                    stage,
                    fanout_commit,
                    comparison_commit,
                    candidate_commit,
                )
                self.assertEqual(result["changed_paths"], inherited_seeds)

    def test_real_validator_rejects_missing_or_substituted_inherited_seeds(
        self,
    ) -> None:
        inherited_seeds = (
            "src/domain/vnext/authority/protected_diagnostic_envelope_stage8_seed.rs",
            "src/domain/vnext/integration/mod.rs",
            "src/domain/vnext/integration/trusted_host_diagnostic_stage10_seed.rs",
            "src/domain/vnext/persistence/mod.rs",
            "src/domain/vnext/persistence/protected_diagnostic_stage9_seed.rs",
        )
        with tempfile.TemporaryDirectory() as directory:
            repository, valid_fanout = self._fanout_repository(Path(directory))
            for inherited_seed in inherited_seeds:
                original = (repository / inherited_seed).read_bytes()
                for attack, paths, remove_paths, expected_error in (
                    (
                        "missing",
                        {},
                        {inherited_seed},
                        "inherited mutable seed is absent",
                    ),
                    (
                        "substituted",
                        {inherited_seed: original + b"\nfanout drift\n"},
                        set(),
                        "inherited mutable seed differs",
                    ),
                ):
                    with self.subTest(seed=inherited_seed, attack=attack):
                        attacked_fanout = self._commit_tree_paths(
                            repository,
                            base_treeish=valid_fanout,
                            parent=validate.CERTIFIED_STAGE5["commit"],
                            paths=paths,
                            remove_paths=remove_paths,
                            message=f"{attack} inherited seed {inherited_seed}",
                        )
                        candidate_commit = self._commit_tree_paths(
                            repository,
                            base_treeish=attacked_fanout,
                            parent=attacked_fanout,
                            paths={
                                "src/domain/vnext/projection/packet.rs": b"candidate\n"
                            },
                            message=f"stage6 candidate after {attack}",
                        )
                        with self.assertRaisesRegex(
                            validate.FanoutValidationError,
                            expected_error,
                        ):
                            validate.validate_candidate(
                                repository,
                                validate.MANIFEST_PATH,
                                6,
                                attacked_fanout,
                                attacked_fanout,
                                candidate_commit,
                            )

    def test_cross_stage_and_shared_paths_are_rejected(self) -> None:
        for path in (
            "src/domain/vnext/planning/advice.rs",
            "src/domain/vnext/mod.rs",
            "contracts/vnext/stage6/generated.json",
        ):
            with self.subTest(path=path), self.assertRaises(validate.FanoutValidationError):
                validate.validate_changes(
                    self.manifest, 6, [("A", path, "000000", "100644")], set()
                )

    def test_existing_nonseed_and_deletion_are_rejected(self) -> None:
        with self.assertRaises(validate.FanoutValidationError):
            validate.validate_changes(
                self.manifest,
                6,
                [
                    (
                        "M",
                        "src/domain/vnext/projection/existing.rs",
                        "100644",
                        "100644",
                    )
                ],
                {"src/domain/vnext/projection/existing.rs"},
            )
        with self.assertRaises(validate.FanoutValidationError):
            validate.validate_changes(
                self.manifest,
                6,
                [("D", "src/domain/vnext/projection/mod.rs", "100644", "000000")],
                {"src/domain/vnext/projection/mod.rs"},
            )

    def test_injected_overlap_and_stage12_production_write_are_rejected(self) -> None:
        overlap = copy.deepcopy(self.manifest)
        overlap["stage_owners"][1]["write_prefixes"].append(
            "src/domain/vnext/projection/nested/"
        )
        with self.assertRaises(validate.FanoutValidationError):
            validate.validate_manifest(overlap)
        with self.assertRaises(validate.FanoutValidationError):
            validate.validate_changes(
                self.manifest,
                12,
                [("A", "src/domain/vnext/removal.rs", "000000", "100644")],
                set(),
            )

    def test_raw_diff_parser_is_nul_safe_and_mode_preserving(self) -> None:
        zero = "0" * 40
        old = "1" * 40
        new = "2" * 40
        self.assertEqual(
            validate.parse_raw_changes(
                (
                    f":100644 100644 {old} {new} M\0"
                    "src/domain/vnext/projection/mod.rs\0"
                    f":000000 100644 {zero} {new} A\0"
                    "tests/vnext_stage6_x.rs\0"
                ).encode()
            ),
            [
                ("M", "src/domain/vnext/projection/mod.rs", "100644", "100644"),
                ("A", "tests/vnext_stage6_x.rs", "000000", "100644"),
            ],
        )

    def test_symlink_gitlink_and_unapproved_executable_are_rejected(self) -> None:
        for path, mode in (
            ("src/domain/vnext/projection/link.rs", "120000"),
            ("src/domain/vnext/projection/vendor", "160000"),
            ("src/domain/vnext/projection/data.json", "100755"),
        ):
            with self.subTest(path=path, mode=mode), self.assertRaises(
                validate.FanoutValidationError
            ):
                validate.validate_changes(
                    self.manifest,
                    6,
                    [("A", path, "000000", mode)],
                    set(),
                )

    def test_owned_executable_script_is_accepted(self) -> None:
        self.assertEqual(
            validate.validate_changes(
                self.manifest,
                6,
                [("A", "tools/vnext_contracts/stage6/prove.sh", "000000", "100755")],
                set(),
            ),
            ["tools/vnext_contracts/stage6/prove.sh"],
        )

    def test_real_git_symlink_and_gitlink_modes_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository = Path(directory) / "repository"
            repository.mkdir()
            self._git(repository, "init", "--quiet")
            owned = repository / "src/domain/vnext/projection"
            owned.mkdir(parents=True)
            os.symlink("outside.rs", owned / "link.rs")

            gitlink = owned / "vendor"
            gitlink.mkdir()
            self._git(gitlink, "init", "--quiet")
            (gitlink / "marker").write_text("nested\n", encoding="utf-8")
            self._git(gitlink, "add", "marker")
            self._git(
                gitlink,
                "-c",
                "user.name=Fanout Test",
                "-c",
                "user.email=fanout@example.invalid",
                "commit",
                "--quiet",
                "-m",
                "nested",
            )

            self._git(
                repository,
                "add",
                "src/domain/vnext/projection/link.rs",
                "src/domain/vnext/projection/vendor",
            )
            raw = self._git(
                repository,
                "diff",
                "--cached",
                "--raw",
                "--abbrev=64",
                "-z",
                "--no-renames",
            )
            changes = validate.parse_raw_changes(raw)
            self.assertEqual({row[3] for row in changes}, {"120000", "160000"})
            with self.assertRaises(validate.FanoutValidationError):
                validate.validate_changes(self.manifest, 6, changes, set())

    def test_real_git_artifacts_and_cli_validate_one_stage6_candidate(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, fanout_commit = self._fanout_repository(Path(directory))
            candidate_commit = self._commit_paths(
                repository,
                {"src/domain/vnext/projection/packet.rs": b"candidate\n"},
                "stage6 candidate",
            )

            result = validate.validate_candidate(
                repository,
                validate.MANIFEST_PATH,
                6,
                fanout_commit,
                fanout_commit,
                candidate_commit,
            )
            self.assertEqual(
                result["changed_paths"],
                ["src/domain/vnext/projection/packet.rs"],
            )

            completed = self._run_cli(
                repository,
                stage=6,
                fanout=fanout_commit,
                comparison=fanout_commit,
                candidate=candidate_commit,
            )
            self.assertEqual(completed.returncode, 0, completed.stderr)
            self.assertEqual(
                json.loads(completed.stdout)["changed_paths"],
                ["src/domain/vnext/projection/packet.rs"],
            )

    def test_real_validator_rejects_canonical_input_tree_attacks(self) -> None:
        workspace = Path(__file__).resolve().parents[3]
        with tempfile.TemporaryDirectory() as directory:
            repository, valid_fanout = self._fanout_repository(Path(directory))
            for path, _, _ in validate.CANONICAL_INPUTS:
                original = (workspace / path).read_bytes()
                substituted = bytes([original[0] ^ 1]) + original[1:]
                attacks = (
                    (
                        "missing",
                        {},
                        {path},
                        {},
                        "canonical fanout input is absent",
                    ),
                    (
                        "substituted",
                        {path: substituted},
                        set(),
                        {},
                        "canonical fanout input SHA-256 differs",
                    ),
                    (
                        "wrong length",
                        {path: original[:-1]},
                        set(),
                        {},
                        "canonical fanout input byte length differs",
                    ),
                    (
                        "wrong mode",
                        {path: original},
                        set(),
                        {path: "100755"},
                        "canonical fanout input mode or path differs",
                    ),
                )
                for attack, paths, remove_paths, path_modes, expected_error in attacks:
                    with self.subTest(path=path, attack=attack):
                        attacked_fanout = self._commit_tree_paths(
                            repository,
                            base_treeish=valid_fanout,
                            parent=validate.CERTIFIED_STAGE5["commit"],
                            paths=paths,
                            remove_paths=remove_paths,
                            path_modes=path_modes,
                            message=f"{attack} canonical input {path}",
                        )
                        candidate_commit = self._commit_tree_paths(
                            repository,
                            base_treeish=attacked_fanout,
                            parent=attacked_fanout,
                            paths={
                                "src/domain/vnext/projection/packet.rs": b"candidate\n"
                            },
                            message=f"stage6 candidate after {attack}",
                        )
                        with self.assertRaisesRegex(
                            validate.FanoutValidationError,
                            expected_error,
                        ):
                            validate.validate_candidate(
                                repository,
                                validate.MANIFEST_PATH,
                                6,
                                attacked_fanout,
                                attacked_fanout,
                                candidate_commit,
                            )

    def test_real_validator_ignores_store_sqlite_canonical_input_decoy(self) -> None:
        path, expected_sha256, expected_byte_length = validate.CANONICAL_INPUTS[0]
        workspace = Path(__file__).resolve().parents[3]
        contents = (workspace / path).read_bytes()
        self.assertEqual(len(contents), expected_byte_length)
        self.assertEqual(hashlib.sha256(contents).hexdigest(), expected_sha256)

        with tempfile.TemporaryDirectory() as directory:
            repository, valid_fanout = self._fanout_repository(Path(directory))
            attacked_fanout = self._commit_tree_paths(
                repository,
                base_treeish=valid_fanout,
                parent=validate.CERTIFIED_STAGE5["commit"],
                paths={},
                remove_paths={path},
                message="missing canonical input with working-tree Store decoy",
            )
            candidate_commit = self._commit_tree_paths(
                repository,
                base_treeish=attacked_fanout,
                parent=attacked_fanout,
                paths={"src/domain/vnext/projection/packet.rs": b"candidate\n"},
                message="stage6 candidate",
            )

            (repository / path).unlink()
            store_path = repository / ".maestro/store.sqlite"
            self.assertTrue(store_path.is_file())
            with closing(sqlite3.connect(store_path)) as store:
                store.execute(
                    """
                    INSERT OR REPLACE INTO card_files
                        (card_id, path, mode, contents, sha256, updated_at)
                    VALUES (?, ?, ?, ?, ?, ?)
                    """,
                    (
                        "maestro-whole-flow-architecture-refoundation",
                        "design.md",
                        0o644,
                        contents,
                        expected_sha256,
                        "2026-07-21T23:57:41.213Z",
                    ),
                )
                store.commit()

            with closing(
                sqlite3.connect(
                    f"file:{store_path}?mode=ro&immutable=1",
                    uri=True,
                )
            ) as store:
                row = store.execute(
                    """
                    SELECT mode, length(contents), sha256
                    FROM card_files
                    WHERE card_id = ? AND path = ?
                    """,
                    (
                        "maestro-whole-flow-architecture-refoundation",
                        "design.md",
                    ),
                ).fetchone()
            self.assertEqual(row, (0o644, expected_byte_length, expected_sha256))

            with self.assertRaisesRegex(
                validate.FanoutValidationError,
                "canonical fanout input is absent",
            ):
                validate.validate_candidate(
                    repository,
                    validate.MANIFEST_PATH,
                    6,
                    attacked_fanout,
                    attacked_fanout,
                    candidate_commit,
                )

    def test_cli_rejects_case_aliases_in_candidate_and_fanout_trees(self) -> None:
        attacks = ("candidate", "fanout")
        for attack in attacks:
            with self.subTest(attack=attack), tempfile.TemporaryDirectory() as directory:
                repository, fanout_commit = self._fanout_repository(Path(directory))
                if attack == "candidate":
                    candidate_commit = self._commit_tree_paths(
                        repository,
                        base_treeish=fanout_commit,
                        parent=fanout_commit,
                        paths={
                            "src/domain/vnext/projection/Straße.rs": b"first\n",
                            "src/domain/vnext/projection/STRASSE.rs": b"second\n",
                        },
                        message="candidate case alias attack",
                    )
                    attacked_fanout = fanout_commit
                else:
                    attacked_fanout = self._commit_tree_paths(
                        repository,
                        base_treeish=fanout_commit,
                        parent=validate.CERTIFIED_STAGE5["commit"],
                        paths={"cargo.toml": b"case alias of protected Cargo.toml\n"},
                        message="fanout case alias attack",
                    )
                    candidate_commit = self._commit_tree_paths(
                        repository,
                        base_treeish=attacked_fanout,
                        parent=attacked_fanout,
                        paths={"src/domain/vnext/projection/packet.rs": b"candidate\n"},
                        message="stage6 candidate",
                    )

                completed = self._run_cli(
                    repository,
                    stage=6,
                    fanout=attacked_fanout,
                    comparison=attacked_fanout,
                    candidate=candidate_commit,
                )
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn("case/normalization alias collision", completed.stderr)

    def test_cli_rejects_undeclared_additions_hidden_in_certified_to_fanout_range(
        self,
    ) -> None:
        attacks = (
            ("src/domain/vnext/authority/hidden.rs", "frozen fanout interface"),
            ("src/domain/vnext/projection/hidden.rs", "unreviewed path"),
            (
                ".maestro/cards/maestro-whole-flow-architecture-refoundation/notes.md",
                "unreviewed path",
            ),
        )
        for attack_path, expected_error in attacks:
            with self.subTest(path=attack_path), tempfile.TemporaryDirectory() as directory:
                repository, fanout_commit = self._fanout_repository(
                    Path(directory),
                    extra_fanout_changes={attack_path: b"hidden mutation\n"},
                )
                candidate_commit = self._commit_paths(
                    repository,
                    {"src/domain/vnext/projection/packet.rs": b"candidate\n"},
                    "stage6 candidate",
                )
                completed = self._run_cli(
                    repository,
                    stage=6,
                    fanout=fanout_commit,
                    comparison=fanout_commit,
                    candidate=candidate_commit,
                )
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn(expected_error, completed.stderr)

    def test_cli_rejects_stage5_release_object_byte_change_in_fanout_range(
        self,
    ) -> None:
        release_digest = validate.CERTIFIED_STAGE5["release_identity"].removeprefix(
            "sha256:"
        )
        release_path = (
            "contracts/vnext/stage5/evidence-gates/releases/objects/"
            f"{release_digest}/release.json"
        )
        with tempfile.TemporaryDirectory() as directory:
            repository, valid_fanout = self._fanout_repository(Path(directory))
            attacked_fanout = self._commit_tree_paths(
                repository,
                base_treeish=valid_fanout,
                parent=validate.CERTIFIED_STAGE5["commit"],
                paths={
                    release_path: (repository / release_path).read_bytes()
                    + b"\nfanout mutation\n"
                },
                message="fanout Stage5 release object byte attack",
            )
            candidate_commit = self._commit_tree_paths(
                repository,
                base_treeish=attacked_fanout,
                parent=attacked_fanout,
                paths={"src/domain/vnext/projection/packet.rs": b"candidate\n"},
                message="stage6 candidate",
            )
            completed = self._run_cli(
                repository,
                stage=6,
                fanout=attacked_fanout,
                comparison=attacked_fanout,
                candidate=candidate_commit,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("frozen fanout interface", completed.stderr)

    def test_cli_rejects_later_stage_production_changes_hidden_before_comparison(self) -> None:
        attacks: tuple[tuple[str, bytes | None], ...] = (
            ("src/domain/vnext/installation/hidden.rs", b"hidden addition\n"),
            ("src/domain/vnext/installation/mod.rs", None),
        )
        for attack_path, contents in attacks:
            with self.subTest(path=attack_path), tempfile.TemporaryDirectory() as directory:
                repository, fanout_commit = self._fanout_repository(Path(directory))
                comparison_commit = self._commit_paths(
                    repository,
                    {
                        "src/domain/vnext/projection/integrated.rs": b"stage6\n",
                        attack_path: contents,
                    },
                    "stage6 comparison with hidden later-stage change",
                )
                candidate_commit = self._commit_paths(
                    repository,
                    {"src/domain/vnext/planning/candidate.rs": b"stage7\n"},
                    "stage7 candidate",
                )
                completed = self._run_cli(
                    repository,
                    stage=7,
                    fanout=fanout_commit,
                    comparison=comparison_commit,
                    candidate=candidate_commit,
                )
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn("hidden comparison range", completed.stderr)

    def test_cli_ignores_git_replace_refs_that_hide_a_comparison_attack(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, fanout_commit = self._fanout_repository(Path(directory))
            honest_stage6 = self._commit_paths(
                repository,
                {"src/domain/vnext/projection/integrated.rs": b"stage6\n"},
                "honest stage6 comparison",
            )
            self._git(repository, "checkout", "--quiet", "--detach", fanout_commit)
            attacked_stage6 = self._commit_paths(
                repository,
                {
                    "src/domain/vnext/projection/integrated.rs": b"stage6\n",
                    "src/domain/vnext/installation/hidden.rs": b"hidden stage9\n",
                },
                "attacked stage6 comparison",
            )
            candidate_commit = self._commit_paths(
                repository,
                {"src/domain/vnext/planning/candidate.rs": b"stage7\n"},
                "stage7 candidate",
            )
            self._git(repository, "replace", attacked_stage6, honest_stage6)

            completed = self._run_cli(
                repository,
                stage=7,
                fanout=fanout_commit,
                comparison=attacked_stage6,
                candidate=candidate_commit,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("hidden comparison range", completed.stderr)

    def test_cli_rejects_candidate_that_is_not_one_direct_child(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, fanout_commit = self._fanout_repository(Path(directory))
            self._commit_paths(
                repository,
                {"src/domain/vnext/projection/prelude.rs": b"prelude\n"},
                "unreviewed candidate ancestor",
            )
            candidate_commit = self._commit_paths(
                repository,
                {"src/domain/vnext/projection/candidate.rs": b"candidate\n"},
                "stage6 candidate",
            )

            completed = self._run_cli(
                repository,
                stage=6,
                fanout=fanout_commit,
                comparison=fanout_commit,
                candidate=candidate_commit,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("sole direct child", completed.stderr)

    def test_diff_and_submodule_ignore_config_keys_are_forbidden(self) -> None:
        for key in ("diff.ignoreSubmodules", "submodule.hidden.ignore"):
            with self.subTest(key=key), self.assertRaisesRegex(
                validate.FanoutValidationError, "diff/submodule-ignore Git config"
            ):
                validate.validate_git_config_listing(
                    f"{key}\nall\0".encode(), "test Git config"
                )

        validate.validate_git_config_listing(
            b"core.repositoryformatversion\n0\0extensions.worktreeconfig\ntrue\0",
            "test Git config",
        )

    def test_common_and_worktree_ignore_cannot_hide_candidate_gitlink(self) -> None:
        visible_path = "src/domain/vnext/projection/visible.rs"
        gitlink_path = "src/domain/vnext/projection/hidden-submodule"
        for config_surface in ("common", "worktree"):
            with self.subTest(
                config_surface=config_surface
            ), tempfile.TemporaryDirectory() as directory:
                repository, fanout_commit = self._fanout_repository(Path(directory))
                candidate_commit = self._commit_tree_paths(
                    repository,
                    base_treeish=fanout_commit,
                    parent=fanout_commit,
                    paths={visible_path: b"visible owned change\n"},
                    gitlinks={gitlink_path: fanout_commit},
                    message=f"{config_surface} hidden gitlink attack",
                )
                if config_surface == "common":
                    self._git(
                        repository, "config", "diff.ignoreSubmodules", "all"
                    )
                else:
                    self._git(
                        repository, "config", "extensions.worktreeConfig", "true"
                    )
                    self._git(
                        repository,
                        "config",
                        "--worktree",
                        "diff.ignoreSubmodules",
                        "all",
                    )

                hidden_diff = self._git(
                    repository,
                    "diff",
                    "--raw",
                    "--no-renames",
                    fanout_commit,
                    candidate_commit,
                )
                self.assertIn(visible_path.encode(), hidden_diff)
                self.assertNotIn(gitlink_path.encode(), hidden_diff)

                forced_paths = {
                    path
                    for _, path, _, _ in validate.raw_changes_between(
                        repository, fanout_commit, candidate_commit
                    )
                }
                self.assertEqual(forced_paths, {visible_path, gitlink_path})

                _, ordinary_entries = validate.authenticated_tree(
                    repository, fanout_commit, "ordinary fanout tree"
                )
                self.assertTrue(
                    all(
                        entry.object_type == "blob"
                        and entry.mode in validate.REGULAR_BLOB_MODES
                        for entry in ordinary_entries.values()
                    )
                )
                with self.assertRaisesRegex(
                    validate.FanoutValidationError, "forbidden tree entry"
                ):
                    validate.authenticated_tree(
                        repository, candidate_commit, "attacked candidate tree"
                    )

                completed = self._run_cli(
                    repository,
                    stage=6,
                    fanout=fanout_commit,
                    comparison=fanout_commit,
                    candidate=candidate_commit,
                )
                self.assertNotEqual(completed.returncode, 0)
                self.assertIn(
                    "forbidden diff/submodule-ignore Git config", completed.stderr
                )

    def test_local_and_worktree_config_files_must_be_single_link(self) -> None:
        for config_surface in ("common", "worktree"):
            with self.subTest(
                config_surface=config_surface
            ), tempfile.TemporaryDirectory() as directory:
                repository, _ = self._fanout_repository(Path(directory))
                if config_surface == "common":
                    config_path = self._git_common_directory(repository) / "config"
                else:
                    self._git(
                        repository, "config", "extensions.worktreeConfig", "true"
                    )
                    self._git(
                        repository,
                        "config",
                        "--worktree",
                        "user.fanout-test",
                        "ordinary",
                    )
                    config_path = self._git_path(repository, "config.worktree")
                external_config = Path(directory) / f"{config_surface}.config"
                config_path.rename(external_config)
                os.link(external_config, config_path)

                with self.assertRaisesRegex(
                    validate.FanoutValidationError, "hardlinked"
                ):
                    validate.validate_repository_object_store(repository)

    def test_object_store_preflight_supports_an_ordinary_linked_worktree(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository, fanout_commit = self._fanout_repository(root)
            linked_worktree = root / "linked-worktree"
            self._git(
                repository,
                "worktree",
                "add",
                "--quiet",
                "--detach",
                str(linked_worktree),
                fanout_commit,
            )
            validate.validate_repository_object_store(linked_worktree)

    def test_object_store_preflight_rejects_a_real_shared_clone(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            repository, _ = self._fanout_repository(root / "source")
            shared = root / "shared"
            subprocess.run(
                [
                    "git",
                    "clone",
                    "--quiet",
                    "--shared",
                    "--no-checkout",
                    str(repository),
                    str(shared),
                ],
                check=True,
                capture_output=True,
            )
            with self.assertRaisesRegex(
                validate.FanoutValidationError, "object alternates"
            ):
                validate.validate_repository_object_store(shared)

    def test_object_store_preflight_rejects_shallow_and_alternates_links(self) -> None:
        attacks = ("shallow", "broken-alternates-link")
        for attack in attacks:
            with self.subTest(attack=attack), tempfile.TemporaryDirectory() as directory:
                repository, fanout_commit = self._fanout_repository(Path(directory))
                if attack == "shallow":
                    shallow = self._git_path(repository, "shallow")
                    shallow.write_text(f"{fanout_commit}\n", encoding="ascii")
                    expected = "shallow metadata"
                else:
                    alternates = self._git_path(repository, "objects") / "info" / "alternates"
                    alternates.parent.mkdir(parents=True, exist_ok=True)
                    os.symlink("missing-object-store", alternates)
                    expected = "symlinked entry"
                with self.assertRaisesRegex(validate.FanoutValidationError, expected):
                    validate.validate_repository_object_store(repository)

    def test_object_store_preflight_rejects_promisor_and_external_config(self) -> None:
        attacks = ("promisor-pack", "promisor-config", "external-include")
        for attack in attacks:
            with self.subTest(attack=attack), tempfile.TemporaryDirectory() as directory:
                repository, _ = self._fanout_repository(Path(directory))
                if attack == "promisor-pack":
                    pack = self._git_path(repository, "objects") / "pack" / "attack.promisor"
                    pack.parent.mkdir(parents=True, exist_ok=True)
                    pack.write_bytes(b"")
                    expected = "promisor pack"
                elif attack == "promisor-config":
                    self._git(repository, "config", "remote.origin.promisor", "true")
                    expected = "promisor/lazy Git config"
                else:
                    self._git(
                        repository,
                        "config",
                        "include.path",
                        str(Path(directory) / "external.gitconfig"),
                    )
                    expected = "external Git config inclusion"
                with self.assertRaisesRegex(validate.FanoutValidationError, expected):
                    validate.validate_repository_object_store(repository)

    def test_object_store_preflight_rejects_loose_object_substitution(self) -> None:
        attacks = ("symlink", "hardlink", "hash-mismatch")
        for attack in attacks:
            with self.subTest(attack=attack), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                repository, _ = self._fanout_repository(root)
                candidate_commit = self._commit_paths(
                    repository,
                    {"src/domain/vnext/projection/candidate.rs": b"candidate\n"},
                    "stage6 candidate",
                )
                object_path = (
                    self._git_path(repository, "objects")
                    / candidate_commit[:2]
                    / candidate_commit[2:]
                )
                if attack in {"symlink", "hardlink"}:
                    external_object = root / f"external-{attack}-object"
                    object_path.rename(external_object)
                    if attack == "symlink":
                        os.symlink(external_object, object_path)
                    else:
                        os.link(external_object, object_path)
                    expected = "symlinked, hardlinked, or non-ordinary"
                else:
                    compressed = object_path.read_bytes()
                    inflated = zlib.decompress(compressed)
                    self.assertIn(b"stage6 candidate", inflated)
                    forged = inflated.replace(
                        b"stage6 candidate", b"forged candidate", 1
                    )
                    os.chmod(object_path, object_path.stat().st_mode | 0o200)
                    object_path.write_bytes(zlib.compress(forged))
                    expected = "fails cryptographic identity"

                self.assertIn(
                    b"tree ", self._git(repository, "cat-file", "commit", candidate_commit)
                )
                with self.assertRaisesRegex(validate.FanoutValidationError, expected):
                    validate.validate_repository_object_store(repository)

    def test_cli_rejects_grafts_that_hide_a_reverted_shared_path_attack(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            repository, fanout_commit = self._fanout_repository(Path(directory))
            architecture = (repository / "ARCHITECTURE.md").read_bytes()
            hidden_commit = self._commit_paths(
                repository,
                {"ARCHITECTURE.md": architecture + b"\nhidden shared mutation\n"},
                "hidden shared mutation",
            )
            comparison_commit = self._commit_paths(
                repository,
                {
                    "ARCHITECTURE.md": architecture,
                    "src/domain/vnext/projection/integrated.rs": b"stage6\n",
                },
                "stage6 comparison reverting hidden shared mutation",
            )
            candidate_commit = self._commit_paths(
                repository,
                {"src/domain/vnext/planning/candidate.rs": b"stage7\n"},
                "stage7 candidate",
            )
            grafts = self._git_common_directory(repository) / "info" / "grafts"
            grafts.parent.mkdir(parents=True, exist_ok=True)
            grafts.write_text(
                f"{comparison_commit} {fanout_commit}\n", encoding="ascii"
            )

            self.assertEqual(
                validate.raw_commit_object(repository, comparison_commit).parents,
                (hidden_commit,),
            )
            completed = self._run_cli(
                repository,
                stage=7,
                fanout=fanout_commit,
                comparison=comparison_commit,
                candidate=candidate_commit,
            )
            self.assertNotEqual(completed.returncode, 0)
            self.assertIn("Git grafts metadata", completed.stderr)

    def test_validator_and_certified_readback_use_fail_closed_git_controls(self) -> None:
        validator_source = Path(validate.__file__).read_text(encoding="utf-8")
        self.assertIn('"--no-lazy-fetch"', validator_source)
        self.assertIn('"--ignore-submodules=none"', validator_source)
        self.assertIn('"GIT_NO_LAZY_FETCH": "1"', validator_source)
        self.assertIn('"index-pack"', validator_source)
        self.assertIn("validate_git_object_bytes", validator_source)
        self.assertNotIn('"merge-base"', validator_source)
        self.assertNotIn('"rev-list"', validator_source)

        stage5_source = (
            Path(__file__).resolve().parents[3] / "tests/vnext_stage5_contracts.rs"
        ).read_text(encoding="utf-8")
        for required in (
            'arg("--no-replace-objects")',
            'arg("--no-lazy-fetch")',
            'arg("--no-optional-locks")',
            'env_remove',
            'GIT_CONFIG_GLOBAL',
            'GIT_CONFIG_NOSYSTEM',
            'GIT_NO_LAZY_FETCH',
        ):
            with self.subTest(required=required):
                self.assertIn(required, stage5_source)

    @staticmethod
    def _same_shape_substitution(value: object) -> object:
        if isinstance(value, bool):
            return not value
        if isinstance(value, int):
            return value + 1
        if isinstance(value, str):
            return f"{value}-mutant"
        if isinstance(value, list):
            replacement = copy.deepcopy(value)
            if replacement:
                first = replacement[0]
                replacement[0] = first + 100 if isinstance(first, int) else f"{first}-mutant"
            else:
                replacement.append("mutant")
            return replacement
        raise AssertionError(f"unsupported policy value: {value!r}")

    def _comparison_before_stage(self, repository: Path, candidate_stage: int) -> str:
        comparison_commit = ""
        for stage, path in (
            (6, "src/domain/vnext/projection/integrated.rs"),
            (7, "src/domain/vnext/planning/integrated.rs"),
            (8, "src/domain/vnext/search/integrated.rs"),
            (9, "src/domain/vnext/installation/integrated.rs"),
        ):
            if stage >= candidate_stage:
                break
            comparison_commit = self._commit_paths(
                repository,
                {path: f"stage{stage}\n".encode()},
                f"stage{stage} comparison",
            )
        return comparison_commit

    def _fanout_repository(
        self,
        directory: Path,
        *,
        extra_fanout_changes: dict[str, bytes | None] | None = None,
    ) -> tuple[Path, str]:
        directory.mkdir(parents=True, exist_ok=True)
        repository = directory / "repository"
        subprocess.run(
            [
                "git",
                "clone",
                "--quiet",
                "--no-hardlinks",
                "--no-checkout",
                str(self._certified_fixture),
                str(repository),
            ],
            check=True,
            capture_output=True,
        )
        self._git(repository, "config", "user.name", "Fanout Test")
        self._git(repository, "config", "user.email", "fanout@example.invalid")
        self._git(repository, "config", "gc.auto", "0")
        self._git(repository, "config", "maintenance.auto", "false")
        self._git(repository, "checkout", "--quiet", validate.CERTIFIED_STAGE5["commit"])

        changes: dict[str, bytes | None] = {}
        canonical_inputs = {path for path, _, _ in validate.CANONICAL_INPUTS}
        workspace = Path(__file__).resolve().parents[3]
        for path, status in validate.expected_fanout_base_changes(
            self.manifest
        ).items():
            target = repository / path
            if path == validate.MANIFEST_REPOSITORY_PATH:
                changes[path] = validate.MANIFEST_PATH.read_bytes()
            elif path in canonical_inputs:
                changes[path] = (workspace / path).read_bytes()
            elif status == "A":
                changes[path] = b"fanout seed\n"
            else:
                changes[path] = target.read_bytes() + b"\nfanout seed\n"
        changes.update(extra_fanout_changes or {})
        fanout_commit = self._commit_paths(repository, changes, "fanout base")
        return repository, fanout_commit

    def _commit_paths(
        self,
        repository: Path,
        changes: dict[str, bytes | None],
        message: str,
    ) -> str:
        for path, contents in changes.items():
            target = repository / path
            if contents is None:
                target.unlink()
            else:
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes(contents)
        self._git(repository, "add", "--all")
        ignored_additions = [
            path
            for path, contents in changes.items()
            if contents is not None and path.startswith(".maestro/")
        ]
        if ignored_additions:
            self._git(repository, "add", "--force", "--", *ignored_additions)
        self._git(repository, "commit", "--quiet", "-m", message)
        return self._git(repository, "rev-parse", "HEAD").decode().strip()

    def _commit_tree_paths(
        self,
        repository: Path,
        *,
        base_treeish: str,
        parent: str,
        paths: dict[str, bytes],
        gitlinks: dict[str, str] | None = None,
        path_modes: dict[str, str] | None = None,
        remove_paths: set[str] | None = None,
        message: str,
    ) -> str:
        modes = path_modes or {}
        removals = remove_paths or set()
        if set(modes) - set(paths):
            raise AssertionError("path_modes must name only paths being written")
        if set(paths).intersection(removals):
            raise AssertionError("paths and remove_paths must be disjoint")
        with tempfile.TemporaryDirectory(dir=repository.parent) as index_directory:
            environment = os.environ.copy()
            environment["GIT_INDEX_FILE"] = str(Path(index_directory) / "index")
            self._run_git(repository, ["read-tree", base_treeish], environment=environment)
            for path, contents in paths.items():
                blob = self._run_git(
                    repository,
                    ["hash-object", "-w", "--stdin"],
                    input_bytes=contents,
                ).decode().strip()
                self._run_git(
                    repository,
                    [
                        "update-index",
                        "--add",
                        "--cacheinfo",
                        f"{modes.get(path, '100644')},{blob},{path}",
                    ],
                    environment=environment,
                )
            for path in removals:
                self._run_git(
                    repository,
                    ["update-index", "--force-remove", "--", path],
                    environment=environment,
                )
            for path, commit in (gitlinks or {}).items():
                self._run_git(
                    repository,
                    ["update-index", "--add", "--cacheinfo", f"160000,{commit},{path}"],
                    environment=environment,
                )
            tree = self._run_git(
                repository, ["write-tree"], environment=environment
            ).decode().strip()
            return self._run_git(
                repository,
                ["commit-tree", tree, "-p", parent],
                input_bytes=f"{message}\n".encode(),
            ).decode().strip()

    @staticmethod
    def _run_cli(
        repository: Path,
        *,
        stage: int,
        fanout: str,
        comparison: str,
        candidate: str,
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                sys.executable,
                str(validate.__file__),
                "--repository",
                str(repository),
                "--manifest",
                str(validate.MANIFEST_PATH),
                "--stage",
                str(stage),
                "--fanout-base",
                fanout,
                "--comparison-base",
                comparison,
                "--candidate",
                candidate,
            ],
            check=False,
            capture_output=True,
            text=True,
        )

    @staticmethod
    def _git(repository: Path, *arguments: str) -> bytes:
        return FanoutOwnershipTests._run_git(repository, list(arguments))

    @staticmethod
    def _run_git(
        repository: Path,
        arguments: list[str],
        *,
        environment: dict[str, str] | None = None,
        input_bytes: bytes | None = None,
    ) -> bytes:
        return subprocess.run(
            ["git", "-C", str(repository), *arguments],
            check=True,
            capture_output=True,
            env=environment,
            input=input_bytes,
        ).stdout

    def _git_path(self, repository: Path, name: str) -> Path:
        return Path(
            self._git(
                repository,
                "rev-parse",
                "--path-format=absolute",
                "--git-path",
                name,
            )
            .decode("utf-8")
            .strip()
        )

    def _git_common_directory(self, repository: Path) -> Path:
        return Path(
            self._git(
                repository,
                "rev-parse",
                "--path-format=absolute",
                "--git-common-dir",
            )
            .decode("utf-8")
            .strip()
        )


if __name__ == "__main__":
    unittest.main()
