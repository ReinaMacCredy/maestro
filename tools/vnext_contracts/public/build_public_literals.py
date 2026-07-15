#!/usr/bin/env python3
"""Generate the closed candidate-only public vNext literal fixtures."""

from __future__ import annotations

import argparse
import hashlib
import itertools
import json
from collections import Counter
from pathlib import Path


RECIPE_ROWS = [
    ("bounded-continuation", "Bounded Continuation", "ContinuationOverlay"),
    ("conflict-handoff", "Conflict Handoff", "Primary"),
    ("design-relay", "Design Relay", "Primary"),
    ("fanout", "Fanout", "Primary"),
    ("intake-triage", "Intake Triage", "Primary"),
    ("learning", "Learning", "Primary"),
    ("setup", "Setup", "Primary"),
    ("ship", "Ship", "Primary"),
    ("synthesize", "Synthesize Results", "Primary"),
    ("wayfinding", "Wayfinding", "Primary"),
]
PHASES = ["Perceive", "Choose", "Act", "Observe", "Learn", "Continue"]
OUTCOMES = ["NotApplicable", "RestrictiveAdvice", "HardStop"]
PROFILE_IDS = ["attended", "unattended"]
RESOLUTION_BASIS_REF = "candidate:orchestration:recipe-resolution-basis:v1"
CHECK_ONLY = False
CHECK_MISMATCHES: list[str] = []

JOBS = ["Setup", "Research", "Design", "Review", "Execute", "Recover", "Adapt"]
METHOD_ROWS = {
    "Setup": [],
    "Research": [],
    "Design": [
        "DDD", "DomainModel", "Grilling", "PRD", "ArchitectureDeepening", "Probe",
        "GenerateAndFilter", "QABaseline",
    ],
    "Review": [
        "Audit", "ArchitectureReview", "AdversarialReview", "GenerateAndFilter",
        "QAReplay", "CloseReview", "Verification",
    ],
    "Execute": ["TDD", "Simplify"],
    "Recover": [],
    "Adapt": ["ExtensionLaw", "GenerateAndFilter"],
}
METHOD_PATHS = {
    "DDD": "skills/maestro/methods/ddd.md",
    "DomainModel": "skills/maestro/methods/domain-model.md",
    "Grilling": "skills/maestro/methods/grilling.md",
    "PRD": "skills/maestro/methods/prd.md",
    "ArchitectureDeepening": "skills/maestro/methods/architecture-deepening.md",
    "Probe": "skills/maestro/methods/probe.md",
    "GenerateAndFilter": "skills/maestro/methods/generate-filter.md",
    "QABaseline": "skills/maestro/methods/qa-baseline.md",
    "Audit": "skills/maestro/methods/audit.md",
    "ArchitectureReview": "skills/maestro/methods/architecture-review.md",
    "AdversarialReview": "skills/maestro/methods/adversarial-review.md",
    "QAReplay": "skills/maestro/methods/qa-replay.md",
    "CloseReview": "skills/maestro/methods/close-review.md",
    "Verification": "skills/maestro/methods/verification.md",
    "TDD": "skills/maestro/methods/tdd.md",
    "Simplify": "skills/maestro/methods/simplify.md",
    "ExtensionLaw": "skills/maestro/methods/extension-law.md",
}
JOB_PATHS = {job: f"skills/maestro/jobs/{job.lower()}.md" for job in JOBS}
TDD_CHILDREN = {
    "TestDesign": ("skills/maestro/methods/tdd/test-design.md", "TestsExposeBehavior"),
    "InterfaceDesign": ("skills/maestro/methods/tdd/interface-design.md", "InterfaceBeforeImplementation"),
    "Mocking": ("skills/maestro/methods/tdd/mocking.md", "IsolationBoundary"),
    "Refactoring": ("skills/maestro/methods/tdd/refactoring.md", "BehaviorPreservingCleanup"),
    "DeepModules": ("skills/maestro/methods/tdd/deep-modules.md", "DeepModuleBoundary"),
}
RESEARCH_EXAMPLE_PATH = "skills/maestro/examples/research.md"
JOB_RECIPE_ROWS = {
    "bounded-continuation": JOBS,
    "conflict-handoff": ["Execute", "Recover"],
    "design-relay": ["Design"],
    "fanout": ["Execute"],
    "intake-triage": ["Research", "Design"],
    "learning": ["Review", "Recover", "Adapt"],
    "setup": ["Setup"],
    "ship": ["Execute"],
    "synthesize": ["Execute", "Recover"],
    "wayfinding": ["Research", "Design"],
}
REVIEW_MODES = ["Inspect", "Audit", "AdversarialReview", "QAReplay", "CloseReview"]
REVIEW_PRIMARY = {
    "Inspect": None,
    "Audit": "Audit",
    "AdversarialReview": "AdversarialReview",
    "QAReplay": "QAReplay",
    "CloseReview": "CloseReview",
}
REVIEW_AUXILIARIES = ["ArchitectureReview", "GenerateAndFilter", "Verification"]
REVIEW_ADMITTED_AUXILIARIES = {
    "Inspect": [],
    "Audit": REVIEW_AUXILIARIES,
    "AdversarialReview": [],
    "QAReplay": [],
    "CloseReview": ["Verification"],
}
ROUTE_SELECTED = {
    "BootstrapRequired": "Setup",
    "SetupRequired": "Setup",
    "ExplicitResearch": "Research",
    "ContextUnknown": "Research",
    "DesignRequired": "Design",
    "ExplicitReview": "Review",
    "ReviewRequired": "Review",
    "StepRunnable": "Execute",
    "RecoveryRequired": "Recover",
    "ExtensionRequired": "Adapt",
}
ROUTE_AMBIGUOUS = ["ConflictingReadIntent", "ConflictingRouteFacts"]
ROUTE_BLOCKED = [
    "MissingRouteMapping", "CanonicalWaitOrStop", "StaleRouteInput",
    "IncompatibleRouteInput", "UnavailableResourceClosure",
]
SETUP_MODES = ["Install", "Adopt", "Migrate", "Update", "Repair", "Rollback", "Uninstall"]
SETUP_BLOCKED_REASONS = [
    "UnsupportedSchemaOrCatalog", "ContextLegalityMismatch", "LocalityMismatch",
    "CrossDomainAggregate", "StaleFactView", "StaleOrUnadvertisedOperation",
    "RecoveryRequired", "EffectInDoubt", "ConflictingOwnerFacts",
    "UnsafeOrAmbiguousTarget", "GenerationUnresolved", "MigrationNotReady",
    "TargetOrCustodyUnavailable", "DesiredStateUnresolved", "NoEligibleSnapshot",
    "AlreadyCurrent", "NoEligibleMode", "RequestedModeIneligible",
    "OperationModeMismatch", "AuthorityUnavailable", "CapabilityUnavailable",
]

BASE_ACTION_FAMILIES = {
    "Work": ["CreateDraftWork", "CancelWork", "CompleteWork", "AbsorbWork"],
    "Contract": ["PublishInitialContract", "AmendContract"],
    "Design": ["BeginDesignStream", "AppendDesignRevision", "RecordNoDesignClosure", "SealDesignFinalization"],
    "Decision": ["BeginDecision", "AppendDecisionRevision", "ResolveDecision", "ResolveDecisionSet", "SupersedeDecisionResolution"],
    "Execution": [
        "AcquireStepExecution", "RenewStepLeaseTerm", "SubmitStep", "AbandonStepAttempt",
        "OriginateEffectIntent", "OriginateCoordinationDelivery", "RecordDispatchOutcome",
        "ReconcileEffectIntent", "ReserveBootstrapMandateInteractionEffect",
        "PublishBootstrapMandateInteractionOutcome", "ReconcileBootstrapMandateInteractionEffect",
        "ReserveContinuityMaintenanceEffect", "PublishContinuityMaintenanceEffectOutcome",
        "ReconcileContinuityMaintenanceEffect",
    ],
    "Evidence": [
        "PublishObservation", "PublishAssessment", "InvalidateAssessment", "SecurityEraseEvidencePayload",
        "PublishBootstrapMandatePresentationObservation", "PublishBootstrapMandateResponseObservation",
        "PublishContinuityMaintenanceObservation",
    ],
    "Authority": [
        "FirstHumanBindingEnrollment", "AddHumanBinding", "ReplaceHumanBindingAndRevokeOld",
        "RevokeHumanBinding", "EnrollRecoveryCommitmentSelection", "RotateRecoveryCommitmentSelection",
        "RevokeRecoveryCommitmentSelection", "IssueBootstrapMandate", "AdmitEpochRootProposal",
        "DisposeEpochRootProposal", "AllocateGovernedCapacitySlot", "EstablishConsumptionCellRoot",
        "PartitionCapacity", "RetireCapacityPartition", "RetireConsumptionCellRoot",
        "IssueRootAttachedBoundedGrant", "ReissueRootAttachedGrantOneToOne", "RevokeGrant",
        "DelegateGrant", "CorrectConsumptionCellAssertionOneToOne", "RotateTrustedTimePolicyStack",
        "RepairTrustedTimePolicyStack", "AcceptContinuityFloor", "SealTerminalExpiry",
        "PreparePlannedEpochTurnover", "ActivatePlannedEpochTurnover",
        "ReserveGovernedReviewPublicationAdmission", "ReconcileGovernedReviewPublicationAdmission",
        "DiscloseGovernedReviewClosure", "PublishGovernedReviewPackage", "IssueGovernanceDowngradeMandate",
        "BindInstallationPrincipal", "RebindInstallationPrincipalAndRevokeOld", "IssueInstallationGrant",
        "ReissueInstallationGrantOneToOne", "RevokeInstallationGrant",
        "RecordInstallationMandateDisposition", "RotateInstallationTrustRoot",
        "RotateInstallationCredentialBinding", "ReplaceInstallationManagerBinding",
        "ReplaceInstallationPolicyBinding", "ReplaceInstallationStructuralRootAndFloor",
        "SuspendInstallationAuthority", "EnrollInstallationRecoveryCommitment",
        "RotateInstallationRecoveryCommitment", "RevokeInstallationRecoveryCommitment",
        "AdmitWriterCohort", "FenceWriterCohort",
    ],
    "Coordination": [
        "PublishInitialMessage", "PublishMessage", "AcknowledgeMessage", "ReplaceFocus", "WithdrawFocus",
        "PublishScope", "WithdrawScope", "AssertConflict", "ResolveConflict",
    ],
    "Planning": [
        "PublishPlanningProposal", "DisposePlanningProposal", "PublishSchedulingPolicyBinding",
        "PublishSchedulingAssessment",
    ],
    "Persistence": [
        "PublishLogicalTombstone", "PublishSecurityErasureObligation", "ExecuteGcSweep",
        "CreateSealedExport", "VerifyBackup", "StageRestoreCandidate", "VerifyRestoreCandidate",
        "ReconcileAppendableHistory", "StageGovernedReviewPublicationClosure",
        "TombstoneGovernedReviewPublicationClosure",
    ],
    "Distribution": [
        "ReserveDistributionTargets", "AdoptManagedRegion", "TransferWholeFileCustody",
        "BeginDistributionTransaction", "CaptureDistributionBeforeState", "StageDistributionCandidate",
        "ReserveDistributionEffect", "PublishDistributionOccurrence", "VerifyDistributionTarget",
        "CommitDistributionTransaction", "RecoverDistributionTransaction",
        "RollbackDistributionTransaction", "ActivateBinarySlot",
    ],
    "SearchMaintenance": ["RebuildSearchIndex", "PurgeSearchIndex"],
    "Memory": [
        "CreateMemoryCandidate", "PromoteMemoryCandidate", "RejectMemoryCandidate",
        "QuarantineMemoryCandidate", "InvalidateMemoryEntry", "SupersedeMemoryEntry",
        "SecurityEraseMemoryPayload",
    ],
    "Intake": ["RecordIntakeSource", "PublishIntakeFinding", "DisposeIntakeSource"],
    "Research": [
        "BeginResearchQuestion", "AppendResearchQuestionRevision", "PublishResearchSynthesis",
        "DisposeResearchQuestion",
    ],
}

CEREMONIES = [
    "InstallationContextGenesis",
    "RepositoryV1Cutover",
    "InstallationV1Cutover",
    "RecoverRepositoryStoreGeneration",
    "RecoverInstallationStoreGeneration",
    "ActivateVerifiedRepositoryGeneration",
    "ActivateVerifiedInstallationGeneration",
    "RecoverPreStoreBinarySlot",
    "RecoverPreStoreWriterCohort",
    "EstablishRepositoryRecoveryAdmission",
    "EstablishInstallationRecoveryAdmission",
]


def encoded_json(value: object) -> bytes:
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def dump(path: Path, value: object) -> bytes:
    encoded = encoded_json(value)
    if CHECK_ONLY:
        if not path.is_file() or path.read_bytes() != encoded:
            CHECK_MISMATCHES.append(str(path))
        return encoded
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(encoded)
    return encoded


def pascal(recipe_id: str) -> str:
    return "".join(part.capitalize() for part in recipe_id.split("-"))


def cbor_head(major: int, value: int) -> bytes:
    if value < 24:
        return bytes([(major << 5) | value])
    if value <= 0xFF:
        return bytes([(major << 5) | 24, value])
    if value <= 0xFFFF:
        return bytes([(major << 5) | 25]) + value.to_bytes(2, "big")
    if value <= 0xFFFFFFFF:
        return bytes([(major << 5) | 26]) + value.to_bytes(4, "big")
    return bytes([(major << 5) | 27]) + value.to_bytes(8, "big")


def cbor(value: object) -> bytes:
    if isinstance(value, int) and not isinstance(value, bool):
        return cbor_head(0, value)
    if isinstance(value, bytes):
        return cbor_head(2, len(value)) + value
    if isinstance(value, str):
        raw = value.encode("utf-8")
        return cbor_head(3, len(raw)) + raw
    if isinstance(value, list):
        return cbor_head(4, len(value)) + b"".join(cbor(item) for item in value)
    raise TypeError(f"unsupported canonical value: {value!r}")


def semantic_hash(domain: str, value: object) -> str:
    encoded = cbor(value)
    hasher = hashlib.sha256()
    domain_bytes = domain.encode("utf-8")
    hasher.update(len(domain_bytes).to_bytes(8, "big"))
    hasher.update(domain_bytes)
    hasher.update(len(encoded).to_bytes(8, "big"))
    hasher.update(encoded)
    return hasher.hexdigest()


def selection_canonical(selection: dict) -> list:
    if selection["variant"] == "Absent":
        return [1]
    value = selection["value"]
    return [2, *value] if len(value) == 2 else [3, *value]


def request_hash(request: dict) -> str:
    return semantic_hash(
        "maestro.vnext.recipe-selection-request.v1",
        [
            request["schema_version"],
            request["resolution_basis_ref"],
            selection_canonical(request["primary_selection"]),
            selection_canonical(request["continuation_selection"]),
        ],
    )


def application_hash(application: dict) -> str:
    return semantic_hash(
        "maestro.vnext.recipe-application.v1",
        [
            application["schema_version"],
            application["resolution_basis_ref"],
            application["frontier_ref"],
            selection_canonical(application["primary"]),
            selection_canonical(application["continuation"]),
        ],
    )


def recipe_resource_ref(recipe_id: str) -> str:
    return f"candidate:orchestration:recipe:{recipe_id}:v1"


def profile_resource_ref(profile_id: str) -> str:
    return f"candidate:orchestration:bounded-continuation-profile:{profile_id}:v1"


def return_reason_ref(recipe_id: str, outcome: str) -> str:
    return f"candidate:orchestration:recipe-return-reason:{pascal(recipe_id)}{outcome}:v1"


def build_recipe_resources(repo: Path) -> tuple[dict[str, str], dict[str, str]]:
    recipe_hashes = {}
    for recipe_id, _title, role in RECIPE_ROWS:
        manifest = {
            "recipe_id": recipe_id,
            "semantic_version": [1, 0, 0],
            "recipe_role": role,
            "purpose_guidance_resource_ref": f"candidate:orchestration:recipe:{recipe_id}:purpose:v1",
            "trigger_projection_reason_refs": [f"candidate:orchestration:recipe:{recipe_id}:trigger:v1"],
            "required_contract_refs": [],
            "restrictive_operation_filter_program_ref": f"candidate:orchestration:recipe:{recipe_id}:restrictive-filter:v1",
            "phase_guidance_resource_refs": [
                f"candidate:orchestration:recipe:{recipe_id}:phase:{phase.lower()}:v1" for phase in PHASES
            ],
            "required_projection_predicate_refs": [],
            "hard_stop_predicate_refs": [],
            "completion_predicate_refs": [f"candidate:orchestration:recipe:{recipe_id}:completion:v1"],
            "return_reason_refs": [return_reason_ref(recipe_id, outcome) for outcome in OUTCOMES],
            "operating_limit_program_ref": f"candidate:orchestration:recipe:{recipe_id}:operating-limit:v1",
            "allowed_continuation_profile_refs": (
                [profile_resource_ref(profile_id) for profile_id in PROFILE_IDS]
                if recipe_id == "bounded-continuation"
                else []
            ),
        }
        path = repo / f"embedded/vnext/orchestration/recipes/{recipe_id}/manifest.v1.json"
        encoded = dump(path, manifest)
        recipe_hashes[recipe_id] = hashlib.sha256(encoded).hexdigest()

    profile_hashes = {}
    for profile_id in PROFILE_IDS:
        profile = {
            "schema": "maestro.vnext.bounded-continuation-profile.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "profile_id": profile_id,
            "recipe_id": "bounded-continuation",
            "ambiguity_or_missing_authority": "HardStop" if profile_id == "unattended" else "ReturnForMaterialChoice",
            "may_only_tighten": [
                "cadence", "attempts", "time", "cost", "subagent_count",
                "connector_permissions", "denylist", "hard_stops",
            ],
            "forbidden_semantics": [
                "Recommendation", "OperationSelection", "Authority", "Lifecycle", "Mutation",
                "RetryRight", "Cursor", "WorkerRuntime",
            ],
        }
        path = repo / f"embedded/vnext/orchestration/profiles/bounded-continuation/{profile_id}.v1.json"
        encoded = dump(path, profile)
        profile_hashes[profile_id] = hashlib.sha256(encoded).hexdigest()
    return recipe_hashes, profile_hashes


def optional_primary(recipe_id: str | None, recipe_hashes: dict[str, str]) -> dict:
    if recipe_id is None:
        return {"variant": "Absent"}
    return {
        "variant": "Present",
        "value": [recipe_resource_ref(recipe_id), f"sha256:{recipe_hashes[recipe_id]}"],
    }


def optional_continuation(
    profile_id: str | None,
    recipe_hashes: dict[str, str],
    profile_hashes: dict[str, str],
) -> dict:
    if profile_id is None:
        return {"variant": "Absent"}
    return {
        "variant": "Present",
        "value": [
            recipe_resource_ref("bounded-continuation"),
            f"sha256:{recipe_hashes['bounded-continuation']}",
            f"{profile_resource_ref(profile_id)}@sha256:{profile_hashes[profile_id]}",
        ],
    }


def build_selection_application_vectors(
    repo: Path,
    recipe_hashes: dict[str, str],
    profile_hashes: dict[str, str],
) -> None:
    primary_ids = [None] + [recipe_id for recipe_id, _title, role in RECIPE_ROWS if role == "Primary"]
    continuation_ids = [None, "attended", "unattended"]
    rows = []
    ordinal = 0
    for primary_id in primary_ids:
        for profile_id in continuation_ids:
            ordinal += 1
            primary = optional_primary(primary_id, recipe_hashes)
            continuation = optional_continuation(profile_id, recipe_hashes, profile_hashes)
            request = {
                "schema_version": 1,
                "resolution_basis_ref": RESOLUTION_BASIS_REF,
                "primary_selection": primary,
                "continuation_selection": continuation,
            }
            application = {
                "schema_version": 1,
                "resolution_basis_ref": RESOLUTION_BASIS_REF,
                "frontier_ref": f"fixture:action-frontier:selection-shape:{ordinal:02d}",
                "primary": primary,
                "continuation": continuation,
            }
            components = []
            for slot, recipe_id, selection in (
                ("Primary", primary_id, primary),
                ("Continuation", "bounded-continuation" if profile_id else None, continuation),
            ):
                if recipe_id is None:
                    continue
                occurrence = {
                    "schema_version": 1,
                    "resolution_basis_ref": RESOLUTION_BASIS_REF,
                    "frontier_ref": application["frontier_ref"],
                    "component_slot": slot,
                    "exact_selection": selection,
                    "outcome_tag": "NotApplicable",
                    "return_reason_ref": return_reason_ref(recipe_id, "NotApplicable"),
                }
                output_hash = semantic_hash(
                    "maestro.vnext.recipe-component-output.fixture.v1",
                    [ordinal, slot, "NotApplicable", occurrence["return_reason_ref"]],
                )
                components.append(
                    {
                        "component_slot": slot,
                        "recipe_return_occurrence": occurrence,
                        "component_output_hash": f"sha256:{output_hash}",
                    }
                )
            component_hashes = [row["component_output_hash"] for row in components]
            composed_hash = semantic_hash(
                "maestro.vnext.recipe-composed-advice.fixture.v1",
                [ordinal, *component_hashes],
            )
            rows.append(
                {
                    "enumeration_ordinal_not_identity": ordinal,
                    "shape": {
                        "primary": primary_id or "Absent",
                        "continuation": profile_id or "Absent",
                    },
                    "selection_request": request,
                    "application": application,
                    "packet_recipe_binding_fixture": {
                        "schema_version": 1,
                        "selection_request_hash": f"sha256:{request_hash(request)}",
                        "recipe_application": application,
                        "recipe_application_hash": f"sha256:{application_hash(application)}",
                        "component_provenance": components,
                        "advice_provenance": {
                            "composition_outcome": "CoreOnly" if not components else "NotApplicable",
                            "ordered_component_output_hashes": component_hashes,
                            "composed_output_hash": f"sha256:{composed_hash}",
                        },
                    },
                }
            )
    dump(
        repo / "contracts/vnext/public/recipe_selection_application_vectors.v1.json",
        {
            "schema": "maestro.vnext.recipe-selection-application-vectors.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "ordering_is_enumeration_only": True,
            "primary_axis": ["Absent"] + [row[0] for row in RECIPE_ROWS if row[2] == "Primary"],
            "continuation_axis": ["Absent", "attended", "unattended"],
            "vector_count": len(rows),
            "vectors": rows,
        },
    )


def build_return_reasons(repo: Path) -> None:
    members = []
    subsets = []
    tag = 0
    for recipe_id, _title, _role in RECIPE_ROWS:
        refs = []
        for outcome in OUTCOMES:
            tag += 1
            name = pascal(recipe_id) + outcome
            ref = return_reason_ref(recipe_id, outcome)
            refs.append(ref)
            members.append(
                {
                    "tag": tag,
                    "name": name,
                    "recipe_id": recipe_id,
                    "outcome": outcome,
                    "resource_ref": ref,
                    "causal_route_meaning": False,
                }
            )
        subsets.append({"recipe_id": recipe_id, "return_reason_refs": refs})
    occurrence_vectors = [
        {
            "vector_id": "core",
            "component_shape": [],
            "outcomes": [],
            "composition_outcome": "CoreOnly",
        }
    ]
    single_shapes = [
        ("Primary", recipe_id)
        for recipe_id, _title, role in RECIPE_ROWS
        if role == "Primary"
    ] + [("Continuation", "bounded-continuation:attended"), ("Continuation", "bounded-continuation:unattended")]
    for slot, component in single_shapes:
        recipe_id = component.split(":", 1)[0]
        for outcome in OUTCOMES:
            occurrence_vectors.append(
                {
                    "vector_id": f"{slot}:{component}:{outcome}",
                    "component_shape": [{"slot": slot, "component": component}],
                    "outcomes": [
                        {
                            "slot": slot,
                            "outcome": outcome,
                            "return_reason_ref": return_reason_ref(recipe_id, outcome),
                        }
                    ],
                    "composition_outcome": outcome,
                }
            )
    for recipe_id, _title, role in RECIPE_ROWS:
        if role != "Primary":
            continue
        for profile_id in PROFILE_IDS:
            for primary_outcome, continuation_outcome in itertools.product(OUTCOMES, repeat=2):
                if "HardStop" in (primary_outcome, continuation_outcome):
                    composition = "HardStop"
                elif "NotApplicable" in (primary_outcome, continuation_outcome):
                    composition = "NotApplicable"
                else:
                    composition = "RestrictiveAdvice"
                occurrence_vectors.append(
                    {
                        "vector_id": f"Primary:{recipe_id}:{primary_outcome}+Continuation:{profile_id}:{continuation_outcome}",
                        "component_shape": [
                            {"slot": "Primary", "component": recipe_id},
                            {"slot": "Continuation", "component": f"bounded-continuation:{profile_id}"},
                        ],
                        "outcomes": [
                            {
                                "slot": "Primary",
                                "outcome": primary_outcome,
                                "return_reason_ref": return_reason_ref(recipe_id, primary_outcome),
                            },
                            {
                                "slot": "Continuation",
                                "outcome": continuation_outcome,
                                "return_reason_ref": return_reason_ref("bounded-continuation", continuation_outcome),
                            },
                        ],
                        "composition_outcome": composition,
                    }
                )
    dump(
        repo / "contracts/vnext/public/recipe_return_reasons.v1.json",
        {
            "schema": "maestro.vnext.recipe-return-reasons.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "member_count": len(members),
            "manifest_subset_count": len(subsets),
            "members": members,
            "manifest_subsets": subsets,
            "membership_matrix": {"positive": 30, "negative": 270},
            "compatibility_matrix": {"positive": 30, "negative": 870},
            "application_outcome_vector_count": len(occurrence_vectors),
            "application_outcome_vectors": occurrence_vectors,
        },
    )


def exact_selection_shapes() -> list[dict[str, str]]:
    primary_ids = ["Absent"] + [row[0] for row in RECIPE_ROWS if row[2] == "Primary"]
    return [
        {"primary": primary, "continuation": continuation}
        for primary in primary_ids
        for continuation in ["Absent", "attended", "unattended"]
    ]


def build_job_recipe_and_route(repo: Path) -> None:
    rows = []
    for recipe_id, _title, _role in RECIPE_ROWS:
        admitted = JOB_RECIPE_ROWS[recipe_id]
        rows.append(
            {
                "recipe_id": recipe_id,
                "exact_recipe_resource_ref": recipe_resource_ref(recipe_id),
                "eligible_jobs": admitted,
                "refused_jobs": [job for job in JOBS if job not in admitted],
            }
        )
    vectors = []
    for job in JOBS:
        for ordinal, shape in enumerate(exact_selection_shapes(), start=1):
            selected = []
            if shape["primary"] != "Absent":
                selected.append(shape["primary"])
            if shape["continuation"] != "Absent":
                selected.append("bounded-continuation")
            admitted = all(job in JOB_RECIPE_ROWS[recipe] for recipe in selected)
            vectors.append(
                {
                    "job": job,
                    "selection_shape_ordinal": ordinal,
                    "shape": shape,
                    "selected_recipe_ids": selected,
                    "outcome": (
                        "NoRecipe" if not selected else "Admitted" if admitted else "Refused"
                    ),
                    "admitted_recipe_resource_refs": (
                        [recipe_resource_ref(recipe) for recipe in selected] if admitted else []
                    ),
                    "partial_fallback": False,
                }
            )
    dump(
        repo / "contracts/vnext/public/job_recipe_eligibility_vectors.v1.json",
        {
            "schema": "maestro.vnext.job-recipe-eligibility-vectors.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "row_count": len(rows),
            "positive_edges": sum(len(row["eligible_jobs"]) for row in rows),
            "negative_edges": sum(len(row["refused_jobs"]) for row in rows),
            "rows": rows,
            "application_vector_count": len(vectors),
            "admitted_application_count": sum(row["outcome"] != "Refused" for row in vectors),
            "refused_application_count": sum(row["outcome"] == "Refused" for row in vectors),
            "application_vectors": vectors,
        },
    )

    route_rows = [
        {
            "reason": reason,
            "status": "Selected",
            "job": job,
            "initial_load_plan": {"job_resources": 1, "method_resources": 0, "recipe_resources": 0},
        }
        for reason, job in ROUTE_SELECTED.items()
    ] + [
        {"reason": reason, "status": "Ambiguous", "job": None, "instruction_resources": []}
        for reason in ROUTE_AMBIGUOUS
    ] + [
        {"reason": reason, "status": "Blocked", "job": None, "instruction_resources": []}
        for reason in ROUTE_BLOCKED
    ]
    dump(
        repo / "contracts/vnext/public/job_route_contract.v1.json",
        {
            "schema": "maestro.vnext.job-route-contract.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "inputs": ["Packet", "Bootstrap"],
            "explicit_read_intents": ["None", "ResearchReadOnly", "ReviewReadOnly"],
            "bases": ["Bootstrap", "RecoveryState", "ExplicitRequest", "PacketReason"],
            "row_count": len(route_rows),
            "selected_count": len(ROUTE_SELECTED),
            "ambiguous_count": len(ROUTE_AMBIGUOUS),
            "blocked_count": len(ROUTE_BLOCKED),
            "rows": route_rows,
            "precedence": [
                "identity_and_currentness",
                "bootstrap_setup_only",
                "recovery_over_explicit_read_intent",
                "explicit_read_only_research_or_review",
                "total_packet_reason_map",
            ],
            "guidance_is_separate_from_packet": True,
            "owns_recommendation": False,
        },
    )


def instruction_resource_ref(path: str, repo: Path) -> str:
    raw = (repo / "embedded/vnext/capability" / path).read_bytes()
    return f"candidate:capability:instruction-resource:{path}:v1@sha256:{hashlib.sha256(raw).hexdigest()}"


def build_capability_contracts(repo: Path) -> None:
    all_methods = list(METHOD_PATHS)
    job_method_rows = []
    for job in JOBS:
        for method in all_methods:
            job_method_rows.append(
                {
                    "job": job,
                    "method": method,
                    "exact_resource_ref": instruction_resource_ref(METHOD_PATHS[method], repo),
                    "admitted": method in METHOD_ROWS[job],
                }
            )
    degrees = {
        method: sum(method in METHOD_ROWS[job] for job in JOBS) for method in all_methods
    }

    auxiliary_rows = [
        {
            "mode": mode,
            "method": method,
            "admitted": method in REVIEW_ADMITTED_AUXILIARIES[mode],
        }
        for mode in REVIEW_MODES
        for method in REVIEW_AUXILIARIES
    ]
    subset_rows = []
    for mode in REVIEW_MODES:
        for mask in range(8):
            requested = [
                method for index, method in enumerate(REVIEW_AUXILIARIES) if mask & (1 << index)
            ]
            admitted = all(method in REVIEW_ADMITTED_AUXILIARIES[mode] for method in requested)
            selected = ([REVIEW_PRIMARY[mode]] if REVIEW_PRIMARY[mode] else []) + requested
            subset_rows.append(
                {
                    "mode": mode,
                    "requested_auxiliaries": requested,
                    "outcome": "Admitted" if admitted else "Refused",
                    "selected_direct_methods": selected if admitted else [],
                }
            )

    tdd_children = [
        {
            "child": child,
            "exact_resource_ref": instruction_resource_ref(path, repo),
            "typed_need": need,
            "only_parent": "Execute/TDD",
        }
        for child, (path, need) in TDD_CHILDREN.items()
    ]
    tdd_job_refusals = [
        {"job": job, "child": child, "outcome": "RefusedDirectJobChild"}
        for job in JOBS
        for child in TDD_CHILDREN
    ]
    research_rows = [
        {
            "job": job,
            "exact_resource_ref": instruction_resource_ref(RESEARCH_EXAMPLE_PATH, repo),
            "admitted": job == "Research",
            "default_loaded": False,
        }
        for job in JOBS
    ]
    dump(
        repo / "contracts/vnext/public/capability_method_contracts.v1.json",
        {
            "schema": "maestro.vnext.capability-method-contracts.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "skill_ids": ["maestro"],
            "jobs": JOBS,
            "direct_methods": all_methods,
            "instruction_resource_count": 31,
            "job_method": {
                "positive": sum(row["admitted"] for row in job_method_rows),
                "negative": sum(not row["admitted"] for row in job_method_rows),
                "rows": job_method_rows,
                "method_degrees": degrees,
            },
            "capability_method_constraints": {
                "job_only_is_selected_success": True,
                "design_execute_adapt_max_direct_roots": 1,
                "review_uses_review_mode_resolution": True,
                "setup_research_recover_direct_roots": 0,
                "caller_load_plan_forbidden": True,
                "caller_fallback_forbidden": True,
                "caller_private_map_forbidden": True,
            },
            "review": {
                "modes": REVIEW_MODES,
                "primary_method_by_mode": REVIEW_PRIMARY,
                "auxiliary_positive": sum(row["admitted"] for row in auxiliary_rows),
                "auxiliary_negative": sum(not row["admitted"] for row in auxiliary_rows),
                "auxiliary_rows": auxiliary_rows,
                "subset_shape_count": len(subset_rows),
                "admitted_subset_shapes": sum(row["outcome"] == "Admitted" for row in subset_rows),
                "refused_subset_shapes": sum(row["outcome"] == "Refused" for row in subset_rows),
                "subset_rows": subset_rows,
                "invocation_outer_outcomes": ["Produced", "Refused", "Failed"],
                "transport_or_crash_has_result_envelope": False,
                "result_payloads": [
                    "InspectResultV1", "AuditResultV1", "AdversarialReviewResultV1",
                    "QAReplayResultV1", "CloseReviewResultV1",
                ],
            },
            "tdd": {
                "positive_children": len(tdd_children),
                "direct_job_child_refusals": len(tdd_job_refusals),
                "children": tdd_children,
                "job_child_refusals": tdd_job_refusals,
            },
            "research_examples": {
                "positive": sum(row["admitted"] for row in research_rows),
                "negative": sum(not row["admitted"] for row in research_rows),
                "rows": research_rows,
            },
        },
    )


def capability_closures() -> dict[str, list[str]]:
    closures: dict[str, list[str]] = {}
    root = "skills/maestro/SKILL.md"
    for job in JOBS:
        base = [root, JOB_PATHS[job]]
        closures[f"{job}:job-only"] = base
        if job in {"Design", "Adapt"}:
            for method in METHOD_ROWS[job]:
                closures[f"{job}:direct:{method}"] = base + [METHOD_PATHS[method]]
        elif job == "Review":
            for mode in REVIEW_MODES:
                for mask in range(8):
                    requested = [
                        method
                        for index, method in enumerate(REVIEW_AUXILIARIES)
                        if mask & (1 << index)
                    ]
                    if not all(
                        method in REVIEW_ADMITTED_AUXILIARIES[mode] for method in requested
                    ):
                        continue
                    selected = ([REVIEW_PRIMARY[mode]] if REVIEW_PRIMARY[mode] else []) + requested
                    closures[f"Review:{mode}:{'+'.join(requested) or 'none'}"] = base + [
                        METHOD_PATHS[method] for method in selected
                    ]
        elif job == "Execute":
            closures["Execute:direct:Simplify"] = base + [METHOD_PATHS["Simplify"]]
            children = list(TDD_CHILDREN)
            for mask in range(1 << len(children)):
                selected_children = [
                    child for index, child in enumerate(children) if mask & (1 << index)
                ]
                closures[f"Execute:TDD:{'+'.join(selected_children) or 'no-child'}"] = (
                    base
                    + [METHOD_PATHS["TDD"]]
                    + [TDD_CHILDREN[child][0] for child in selected_children]
                )
        elif job == "Research":
            closures["Research:examples"] = base + [RESEARCH_EXAMPLE_PATH]
    return closures


def build_context_budget_profiles(
    repo: Path, recipe_hashes: dict[str, str], profile_hashes: dict[str, str]
) -> None:
    capability = capability_closures()
    measurements = []
    for job in JOBS:
        job_capability = {
            name: paths for name, paths in capability.items() if name.startswith(f"{job}:")
        }
        for shape_ordinal, shape in enumerate(exact_selection_shapes(), start=1):
            selected_recipes = []
            if shape["primary"] != "Absent":
                selected_recipes.append(shape["primary"])
            if shape["continuation"] != "Absent":
                selected_recipes.append("bounded-continuation")
            if not all(job in JOB_RECIPE_ROWS[recipe] for recipe in selected_recipes):
                continue
            recipe_paths = [
                f"orchestration/recipes/{recipe}/manifest.v1.json" for recipe in selected_recipes
            ]
            if shape["continuation"] != "Absent":
                recipe_paths.append(
                    "orchestration/profiles/bounded-continuation/"
                    f"{shape['continuation']}.v1.json"
                )
            for capability_name, instruction_paths in job_capability.items():
                resources: list[tuple[str, Path]] = []
                for path in instruction_paths:
                    resources.append(
                        (
                            instruction_resource_ref(path, repo),
                            repo / "embedded/vnext/capability" / path,
                        )
                    )
                for path in recipe_paths:
                    if "/recipes/" in path:
                        recipe_id = path.split("/recipes/", 1)[1].split("/", 1)[0]
                        ref = f"{recipe_resource_ref(recipe_id)}@sha256:{recipe_hashes[recipe_id]}"
                    else:
                        profile_id = Path(path).name.split(".", 1)[0]
                        ref = f"{profile_resource_ref(profile_id)}@sha256:{profile_hashes[profile_id]}"
                    resources.append((ref, repo / "embedded/vnext" / path))
                refs = [ref for ref, _path in resources]
                utf8_bytes = sum(len(path.read_bytes()) for _ref, path in resources)
                closure_key = f"{capability_name}|selection:{shape_ordinal:02d}"
                measurements.append(
                    {
                        "closure_ref": (
                            "candidate:capability:context-closure:v1@sha256:"
                            + semantic_hash(
                                "maestro.vnext.context-budget-closure.v1", [closure_key, *refs]
                            )
                        ),
                        "job": job,
                        "capability_shape": capability_name,
                        "selection_shape_ordinal": shape_ordinal,
                        "ordered_resource_refs": refs,
                        "utf8_bytes": utf8_bytes,
                    }
                )
    measurements = list(
        {
            tuple(row["ordered_resource_refs"]): row
            for row in measurements
        }.values()
    )
    admitted = sorted({ref for row in measurements for ref in row["ordered_resource_refs"]})
    hosts = [
        ("agents-compatible-cli", "utf8-bytes", lambda count: count),
        ("claude-code", "estimated-four-byte-token-units", lambda count: (count + 3) // 4),
    ]
    profiles = []
    for host_id, meter, unit_fn in hosts:
        rows = [
            {
                **row,
                "host_observed_units": unit_fn(row["utf8_bytes"]),
            }
            for row in measurements
        ]
        profile = {
            "schema": "maestro.vnext.context-budget-profile.v1",
            "schema_version": 1,
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "profile_id": host_id,
            "release_ref": "candidate:release:stage0-public-identity:v1",
            "host_ref": f"candidate:host:{host_id}:v1",
            "renderer_or_meter_ref": f"candidate:context-meter:{meter}:v1",
            "measurement_procedure_ref": "candidate:procedure:exact-release-host-closure-measurement:v1",
            "universal_product_cap": False,
            "admitted_resource_refs": admitted,
            "maximum_utf8_bytes": max(row["utf8_bytes"] for row in rows),
            "maximum_host_observed_units": max(row["host_observed_units"] for row in rows),
            "measurement_count": len(rows),
            "measurements": rows,
        }
        relative = f"embedded/vnext/capability/context-budget/{host_id}.v1.json"
        encoded = dump(repo / relative, profile)
        profiles.append(
            {
                "profile_id": host_id,
                "source_path": relative,
                "profile_sha256": hashlib.sha256(encoded).hexdigest(),
                "measurement_count": len(rows),
                "maximum_utf8_bytes": profile["maximum_utf8_bytes"],
                "maximum_host_observed_units": profile["maximum_host_observed_units"],
            }
        )
    dump(
        repo / "contracts/vnext/public/context_budget_profiles.v1.json",
        {
            "schema": "maestro.vnext.context-budget-profiles.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "release_ref": "candidate:release:stage0-public-identity:v1",
            "host_profile_count": len(profiles),
            "admitted_combined_closure_count": len(measurements),
            "universal_product_cap": False,
            "profiles": profiles,
        },
    )


def record(name: str, fields: list[tuple[str, str]], constraints: list[str] | None = None) -> dict:
    return {
        "name": name,
        "version": 1,
        "kind": "record",
        "ordered_fields": [{"name": field, "type": type_name} for field, type_name in fields],
        "variants": [],
        "cross_constraints": constraints or [],
        "unknown_fields": "reject",
    }


def union(name: str, variants: list[tuple[str, str]], constraints: list[str] | None = None) -> dict:
    return {
        "name": name,
        "version": 1,
        "kind": "union",
        "ordered_fields": [],
        "variants": [{"tag": index, "name": variant, "payload": payload} for index, (variant, payload) in enumerate(variants, 1)],
        "cross_constraints": constraints or [],
        "unknown_fields": "reject",
    }


def enumeration(name: str, members: list[str]) -> dict:
    return {
        "name": name,
        "version": 1,
        "kind": "enum",
        "ordered_fields": [],
        "variants": [{"tag": index, "name": member, "payload": "Unit"} for index, member in enumerate(members, 1)],
        "cross_constraints": [],
        "unknown_fields": "reject",
    }


def public_schema_definitions() -> list[dict]:
    schemas = [
        record("RecipeManifestV1", [
            ("recipe_id", "RecipeIdV1"), ("semantic_version", "Tuple<U64,U64,U64>"),
            ("recipe_role", "RecipeRoleV1"), ("purpose_guidance_resource_ref", "ResourceRefV1"),
            ("trigger_projection_reason_refs", "UniqueOrderedVec<ProjectionReasonRefV1>"),
            ("required_contract_refs", "UniqueOrderedVec<ContractRefV1>"),
            ("restrictive_operation_filter_program_ref", "ResourceRefV1"),
            ("phase_guidance_resource_refs", "ExactArray<ResourceRefV1,6>"),
            ("required_projection_predicate_refs", "UniqueOrderedVec<PredicateRefV1>"),
            ("hard_stop_predicate_refs", "UniqueOrderedVec<PredicateRefV1>"),
            ("completion_predicate_refs", "UniqueOrderedVec<PredicateRefV1>"),
            ("return_reason_refs", "ExactArray<RecipeReturnReasonRefV1,3>"),
            ("operating_limit_program_ref", "ResourceRefV1"),
            ("allowed_continuation_profile_refs", "UniqueOrderedVec<ContextProfileRefV1>"),
        ], ["exactly ten manifests", "BoundedContinuation is sole ContinuationOverlay", "manifest has no self identity or Capability edge"]),
        union("ExactRecipeSelectionV1", [
            ("Absent", "Unit"),
            ("Primary", "Tuple<RecipeResourceRefV1,ManifestContentHashV1>"),
            ("Continuation", "Tuple<RecipeResourceRefV1,ManifestContentHashV1,ContextProfileResourceRefV1>"),
        ], ["Primary excludes BoundedContinuation", "Continuation is exactly BoundedContinuation"]),
        record("RecipeSelectionRequestV1", [
            ("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"),
            ("primary_selection", "ExactRecipeSelectionV1"),
            ("continuation_selection", "ExactRecipeSelectionV1"),
        ], ["both Absent is explicit core", "no Frontier, Packet, authority, cursor, retry, latest or fallback"]),
        record("RecipeApplicationV1", [
            ("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"),
            ("frontier_ref", "ActionFrontierRefV1"), ("primary", "ExactRecipeSelectionV1"),
            ("continuation", "ExactRecipeSelectionV1"),
        ], ["copies one complete request", "exactly one Projection-supplied Frontier"]),
        enumeration("RecipeReturnReasonV1", [pascal(recipe) + outcome for recipe, _title, _role in RECIPE_ROWS for outcome in OUTCOMES]),
        record("RecipeReturnOccurrenceV1", [
            ("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"),
            ("frontier_ref", "ActionFrontierRefV1"), ("component", "RecipeReturnComponentV1"),
            ("outcome_tag", "RecipeComponentOutcomeTagV1"),
            ("return_reason_ref", "RecipeReturnReasonRefV1"),
        ], ["reason is exact recipe by outcome bijection", "same Frontier as application"]),
        union("RecipeComponentEvaluationV1", [
            ("NotApplicable", "RecipeReturnOccurrenceV1"),
            ("RestrictiveAdvice", "Tuple<RecipeAdviceRefV1,RecipeReturnOccurrenceV1>"),
            ("HardStop", "Tuple<RecipeHardStopRefV1,RecipeReturnOccurrenceV1>"),
        ]),
        record("JobRecipeEligibilityRowV1", [("exact_recipe_resource_ref", "RecipeResourceRefV1"), ("eligible_jobs", "NonEmptyUniqueOrderedVec<JobV1>")]),
        record("JobRecipeEligibilityV1", [("schema_version", "Const<1>"), ("orchestration_recipe_catalog_ref", "CatalogRefV1"), ("rows", "ExactArray<JobRecipeEligibilityRowV1,10>")], ["22 admitted and 48 refused edges"]),
        union("SelectedJobRecipeAdmissionOutcomeV1", [("NoRecipe", "Unit"), ("Admitted", "BoundedUniqueOrderedVec<RecipeResourceRefV1,1,2>"), ("Refused", "JobRecipeAdmissionRefusalV1")]),
        record("SelectedJobRecipeAdmissionV1", [("resolution_basis_ref", "ResolutionBasisRefV1"), ("exact_packet_application_ref", "RecipeApplicationRefV1"), ("exact_selected_job_route_ref", "JobRouteRefV1"), ("outcome", "SelectedJobRecipeAdmissionOutcomeV1")], ["all or nothing; no partial fallback"]),
        union("ProjectionScopeV1", [("Repository", "Unit"), ("Work", "ExactWorkRefV1")]),
        union("McpPacketReadModeV1", [("BootstrapNoRecipeV1", "Unit"), ("DiscoverSelectionContextV1", "Unit"), ("ProjectV1", "RecipeSelectionRequestV1")]),
        record("McpPacketReadRequestV1", [
            ("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"),
            ("repository_locator", "RepositoryLocatorV1"),
            ("authenticated_host_connection_context_ref", "HostConnectionContextRefV1"),
            ("projection_scope", "ProjectionScopeV1"), ("expected_release_ref", "ReleaseRefV1"),
            ("expected_public_catalog_ref", "PublicCatalogRefV1"),
            ("bounded_response_redaction_profile", "RedactionProfileRefV1"),
            ("read_mode", "McpPacketReadModeV1"),
        ], ["Bootstrap requires Repository scope and no active Store", "exactly one mode", "no authority, Operation, argv, retry, latest or fallback"]),
        union("McpPacketReadEnvelopeV1", [
            ("Packet", "AgentPacketV1"), ("SelectionContext", "SelectionContextV1"),
            ("NoActiveStore", "Optional<BootstrapRouteFactViewV1>"),
            ("Unavailable", "ReasonRefV1"), ("Stale", "ReasonRefV1"),
            ("Incompatible", "ReasonRefV1"),
        ], ["Packet only from Project", "bootstrap facts only from BootstrapNoRecipe"]),
        record("RecipeSelectionOptionV1", [("primary_selection", "ExactRecipeSelectionV1"), ("continuation_selection", "ExactRecipeSelectionV1")]),
        record("SelectionContextV1", [("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"), ("selection_options", "ExactCanonicalProduct<Primary10,Continuation3>")], ["30 distinct complete options", "order is enumeration only"]),
        record("BootstrapRouteFactViewV1", [("schema_version", "Const<1>"), ("bootstrap_context", "BootstrapContextV1"), ("resolution_basis_ref", "ResolutionBasisRefV1"), ("ordered_source_fact_commitments", "NonEmptyUniqueOrderedVec<FactCommitmentV1>"), ("fact_view_hash", "NonZeroSha256")]),
        record("PacketRecipeComponentProvenanceV1", [("component_slot", "PrimaryOrContinuationV1"), ("recipe_return_occurrence", "RecipeReturnOccurrenceV1"), ("component_output_hash", "NonZeroSha256")]),
        record("PacketRecipeAdviceProvenanceV1", [("composition_outcome", "PacketRecipeAdviceOutcomeV1"), ("ordered_component_output_hashes", "BoundedVec<NonZeroSha256,0,2>"), ("composed_output_hash", "NonZeroSha256")]),
        record("PacketRecipeBindingV1", [
            ("schema_version", "Const<1>"), ("selection_request_hash", "NonZeroSha256"),
            ("recipe_application", "RecipeApplicationV1"), ("recipe_application_hash", "NonZeroSha256"),
            ("component_provenance", "BoundedVec<PacketRecipeComponentProvenanceV1,0,2>"),
            ("advice_provenance", "PacketRecipeAdviceProvenanceV1"),
        ], ["recompute two domain-separated hashes", "0/1/2 rows exactly match selected components", "same Frontier throughout", "selected NotApplicable remains bound and nonactionable"]),
        record("AgentPacketV1", [
            ("schema_version", "Const<1>"), ("packet_id", "NominalPacketIdV1"),
            ("semantic_audit_hash", "NonZeroSha256"), ("as_of_ref", "AsOfRefV1"),
            ("valid_until_ref", "ValidityRefV1"), ("visibility_ref", "VisibilityRefV1"),
            ("scope_manifest", "PacketScopeManifestV1"), ("completeness", "PacketCompletenessV1"),
            ("bounds", "PacketBoundsV1"), ("snapshot_manifest_ref", "SnapshotManifestRefV1"),
            ("projection_result", "PacketProjectionResultV1"), ("blockers", "UniqueOrderedVec<BlockerRefV1>"),
            ("advertised_specs", "UniqueOrderedVec<AdvertisedOperationSpecV1>"),
            ("required_inputs", "UniqueOrderedVec<RequiredInputRefV1>"),
            ("effect_classes", "UniqueOrderedVec<EffectClassV1>"),
            ("idempotency_classes", "UniqueOrderedVec<IdempotencyClassV1>"),
            ("retry_classes", "UniqueOrderedVec<RetryClassV1>"),
            ("inspect_refs", "UniqueOrderedVec<InspectRefV1>"),
            ("recipe_binding", "PacketRecipeBindingV1"),
        ], ["packet hash covers recipe binding", "projection result and binding use one Frontier", "nonactionable Advice advertises zero Operations"]),
        record("McpCliSearchRequestV1", [("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"), ("query", "ExactCommandIdOrBoundedFuzzyIntentV1"), ("finite_bound", "PositiveU64"), ("expected_release_ref", "ReleaseRefV1"), ("expected_public_catalog_ref", "PublicCatalogRefV1")]),
        record("McpCliSearchEnvelopeV1", [("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"), ("running_binary_release", "ReleaseRefV1"), ("binary_digest", "NonZeroSha256"), ("binary_version", "VersionStringV1"), ("executable_slot", "ExecutableSlotRefV1"), ("core_catalog_ref", "CoreCatalogRefV1"), ("public_catalog_ref", "PublicCatalogRefV1"), ("catalog_snapshot_ref", "CatalogSnapshotRefV1"), ("completeness", "SearchCompletenessV1"), ("bounds", "SearchBoundsV1"), ("cursor", "Optional<OpaqueCursorV1>"), ("hits", "BoundedVec<McpCliSearchHitV1>")], ["Complete iff cursor Absent", "BoundedTruncated iff cursor Present", "returned count equals hits"]),
        union("OperationSpecRefV1", [("Action", "ActionSpecRefV1"), ("Ceremony", "CeremonySpecRefV1")]),
        union("OperationRequestV1", [("Action", "ActionRequestV1"), ("Ceremony", "CeremonyRequestV1")]),
        record("ActionRequestV1", [("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"), ("idempotency_key", "NominalIdempotencyKeyV1"), ("semantic_request_hash", "NonZeroSha256"), ("selected_packet_semantic_hash", "NonZeroSha256"), ("action_spec", "ActionSpecRefV1"), ("material_dependency_stamp", "NonZeroSha256"), ("exact_store_generation_ref", "StoreGenerationRefV1"), ("exact_authority_epoch_ref", "AuthorityEpochRefV1"), ("valid_until_ref", "ValidityRefV1"), ("authority_basis", "ActionAuthorityBasisV1"), ("typed_input_cbor", "NonEmptyCanonicalCbor"), ("evidence_refs", "UniqueOrderedVec<EvidenceRefV1>"), ("prerequisite_receipt_refs", "UniqueOrderedVec<ReceiptRefV1>"), ("orchestration_attribution", "Optional<OrchestrationAttributionV1>")]),
        record("CeremonyRequestV1", [("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"), ("idempotency_key", "NominalIdempotencyKeyV1"), ("semantic_request_hash", "NonZeroSha256"), ("ceremony_spec", "CeremonySpecRefV1"), ("request_mode", "CeremonyRequestModeV1"), ("context", "NoStoreOrPreStoreContextV1"), ("branch_authority_ref", "BranchAuthorityRefV1"), ("expected_carrier_token_ref", "CarrierTokenRefV1"), ("typed_input_cbor", "NonEmptyCanonicalCbor"), ("prerequisite_receipt_refs", "UniqueOrderedVec<ReceiptRefV1>"), ("orchestration_attribution", "Optional<OrchestrationAttributionV1>")]),
        union("OperationResultV1", [("Action", "ActionResultV1"), ("Ceremony", "CeremonyResultV1")], ["result branch equals request branch"]),
        record("ActionResultV1", [("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"), ("operation_spec_ref", "ActionSpecRefV1"), ("outcome", "OperationSemanticOutcomeV1"), ("before_revision_refs", "UniqueOrderedVec<RevisionRefV1>"), ("after_revision_refs", "UniqueOrderedVec<RevisionRefV1>"), ("transition_receipt_refs", "UniqueOrderedVec<ReceiptRefV1>"), ("produced_record_refs", "UniqueOrderedVec<RecordRefV1>"), ("next_packet", "Optional<AgentPacketV1>"), ("inspect_ref", "Optional<InspectRefV1>"), ("replayed_delivery", "Bool")]),
        record("CeremonyResultV1", [("schema_version", "Const<1>"), ("request_id", "NominalRequestIdV1"), ("operation_spec_ref", "CeremonySpecRefV1"), ("outcome", "OperationSemanticOutcomeV1"), ("before_revision_refs", "UniqueOrderedVec<RevisionRefV1>"), ("after_revision_refs", "UniqueOrderedVec<RevisionRefV1>"), ("transition_receipt_refs", "UniqueOrderedVec<ReceiptRefV1>"), ("produced_record_refs", "UniqueOrderedVec<RecordRefV1>"), ("next_packet", "Optional<AgentPacketV1>"), ("inspect_ref", "Optional<InspectRefV1>"), ("replayed_delivery", "Bool")]),
        enumeration("OperationSemanticOutcomeV1", ["Committed", "NoOp", "Rejected", "Stale", "Conflict", "Unavailable", "InDoubt"]),
        union("JobRouteInputV1", [("Packet", "ExactPacketIdentityHashValidityV1"), ("Bootstrap", "ExactBootstrapFactViewIdentityHashV1")]),
        record("JobInstructionLoadPlanV1", [("job_resource_ref", "InstructionResourceRefV1"), ("method_resource_refs", "ExactEmptyVec"), ("recipe_resource_refs", "ExactEmptyVec")]),
        union("JobRouteOutcomeV1", [("Selected", "Tuple<JobV1,JobRouteReasonV1,JobInstructionLoadPlanV1>"), ("Ambiguous", "JobRouteReasonV1"), ("Blocked", "JobRouteReasonV1")]),
        record("JobRouteV1", [("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"), ("input", "JobRouteInputV1"), ("explicit_read_intent", "ExplicitReadIntentV1"), ("basis", "JobRouteBasisV1"), ("outcome", "JobRouteOutcomeV1")], ["ten Selected reasons map to one exact job", "two Ambiguous and five Blocked carry no Resource", "Recovery dominates explicit read intent"]),
        union("JobGuidanceEnvelopeV1", [("PacketGuidance", "Tuple<ExactAgentPacketV1,ExactJobRouteV1>"), ("BootstrapGuidance", "Tuple<ExactBootstrapRouteFactViewV1,ExactJobRouteV1>")], ["contained source identity is byte identical", "never enters Packet or MCP"]),
        record("InstructionResourceRefV1", [("logical_path", "InstructionLogicalPathV1"), ("resource_ref", "ContentQualifiedResourceRefV1")]),
        record("JobMethodEligibilityRowV1", [("job", "JobV1"), ("direct_method_resource_refs", "UniqueOrderedVec<DirectMethodResourceRefV1>")]),
        record("JobMethodEligibilityV1", [("schema_version", "Const<1>"), ("capability_job_catalog_ref", "CatalogRefV1"), ("direct_method_catalog_ref", "CatalogRefV1"), ("rows", "ExactArray<JobMethodEligibilityRowV1,7>")], ["19 admitted and 100 refused cells"]),
        record("CapabilityTypedNeedV1", [("schema_version", "Const<1>"), ("need_kind", "CapabilityNeedKindV1"), ("need_subject_ref", "ExactSubjectRefV1")]),
        record("CapabilityMethodIntentV1", [("schema_version", "Const<1>"), ("exact_scope_ref", "ExactScopeRefV1"), ("requested_direct_methods", "UniqueOrderedVec<RequestedDirectMethodV1>"), ("requested_tdd_children", "UniqueOrderedVec<RequestedTddChildV1>"), ("research_examples", "Optional<RequestedResearchExamplesV1>"), ("requested_review_mode", "Optional<ReviewModeRequestV1>")], ["no caller load plan, fallback or private map"]),
        record("CapabilityInstructionLoadPlanV1", [("selected_job_resource_ref", "InstructionResourceRefV1"), ("direct_method_resource_refs", "BoundedUniqueOrderedVec<DirectMethodResourceRefV1,0,4>"), ("tdd_child_resource_refs", "BoundedUniqueOrderedVec<InstructionResourceRefV1,0,5>"), ("research_example_resource_ref", "Optional<InstructionResourceRefV1>")]),
        union("CapabilityMethodResolutionOutcomeV1", [("Selected", "CapabilityInstructionLoadPlanV1"), ("Ambiguous", "CapabilityMethodAmbiguousReasonV1"), ("Blocked", "CapabilityMethodBlockedReasonV1")]),
        record("CapabilityMethodResolutionV1", [("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"), ("exact_selected_job_route_ref", "JobRouteRefV1"), ("exact_intent_ref", "CapabilityMethodIntentRefV1"), ("outcome", "CapabilityMethodResolutionOutcomeV1")], ["job-only is successful Selected", "only Selected is activatable"]),
        record("TddChildEligibilityRowV1", [("child", "TddChildV1"), ("exact_resource_ref", "InstructionResourceRefV1"), ("exact_typed_need", "CapabilityTypedNeedV1")]),
        record("TddChildEligibilityV1", [("schema_version", "Const<1>"), ("rows", "ExactArray<TddChildEligibilityRowV1,5>")], ["only Execute/TDD admits children", "35 direct job-child shapes refuse"]),
        record("ResearchExampleEligibilityV1", [("schema_version", "Const<1>"), ("exact_resource_ref", "InstructionResourceRefV1"), ("eligible_job", "Const<Research>"), ("default_loaded", "Const<false>")], ["one admitted and six refused edges"]),
        record("ReviewModeRequestV1", [("schema_version", "Const<1>"), ("basis", "ReviewModeBasisV1"), ("requested_mode", "Optional<ReviewModeV1>")]),
        record("ReviewMethodLoadPlanV1", [("primary_method", "Optional<DirectMethodResourceRefV1>"), ("auxiliary_methods", "UniqueOrderedVec<DirectMethodResourceRefV1>")]),
        union("ReviewModeResolutionOutcomeV1", [("Selected", "Tuple<ReviewModeV1,ReviewMethodLoadPlanV1>"), ("Ambiguous", "ReviewModeAmbiguousReasonV1"), ("Blocked", "ReviewModeBlockedReasonV1")]),
        record("ReviewModeResolutionV1", [("schema_version", "Const<1>"), ("resolution_basis_ref", "ResolutionBasisRefV1"), ("exact_request_ref", "ReviewModeRequestRefV1"), ("outcome", "ReviewModeResolutionOutcomeV1")], ["five modes", "4/11 auxiliary cells", "13/27 subset shapes"]),
        record("ReviewResultHeaderV1", [("schema_version", "Const<1>"), ("mode", "ReviewModeV1"), ("exact_invocation_ref", "ReviewInvocationRefV1"), ("exact_target_universe_ref", "TargetUniverseRefV1"), ("procedure_ref", "ProcedureRefV1"), ("observed_interval_ref", "ObservedIntervalRefV1"), ("provenance_ref", "ProvenanceRefV1")]),
        record("ReviewCoverageV1", [("examined_targets", "UniqueOrderedVec<TargetRefV1>"), ("unexamined_targets", "UniqueOrderedVec<ReviewUnexaminedTargetV1>"), ("coverage_digest", "NonZeroSha256")], ["examined and unexamined are disjoint and total"]),
        record("InspectResultV1", [("header", "ReviewResultHeaderV1"), ("coverage", "ReviewCoverageV1"), ("rows", "UniqueOrderedVec<InspectResultRowV1>")]),
        record("AuditResultV1", [("header", "ReviewResultHeaderV1"), ("coverage", "ReviewCoverageV1"), ("obligations", "UniqueOrderedVec<AuditObligationResultV1>")]),
        record("AdversarialReviewResultV1", [("header", "ReviewResultHeaderV1"), ("coverage", "ReviewCoverageV1"), ("claims", "UniqueOrderedVec<AdversarialClaimResultV1>")]),
        record("QAReplayResultV1", [("header", "ReviewResultHeaderV1"), ("coverage", "ReviewCoverageV1"), ("scenarios", "UniqueOrderedVec<QAReplayScenarioResultV1>")]),
        record("CloseReviewResultV1", [("header", "ReviewResultHeaderV1"), ("coverage", "ReviewCoverageV1"), ("requirements", "UniqueOrderedVec<CloseReviewRequirementResultV1>")]),
        union("ReviewMethodResultV1", [("Inspect", "InspectResultV1"), ("Audit", "AuditResultV1"), ("AdversarialReview", "AdversarialReviewResultV1"), ("QAReplay", "QAReplayResultV1"), ("CloseReview", "CloseReviewResultV1")]),
        record("ReviewMethodRefusalV1", [("schema_version", "Const<1>"), ("reason", "ReviewMethodRefusalReasonV1"), ("exact_request_ref", "ReviewInvocationRequestRefV1")]),
        record("ReviewMethodFailureV1", [("schema_version", "Const<1>"), ("reason", "ReviewMethodFailureReasonV1"), ("exact_invocation_ref", "ReviewInvocationRefV1")]),
        union("ReviewMethodInvocationV1", [("Produced", "ReviewMethodResultV1"), ("Refused", "ReviewMethodRefusalV1"), ("Failed", "ReviewMethodFailureV1")], ["pre-route refusal has no invocation", "crash, transport or effect uncertainty has no result envelope"]),
        record("ContextBudgetMeasurementV1", [("closure_ref", "ClosureRefV1"), ("ordered_resource_refs", "NonEmptyUniqueVec<ResourceRefV1>"), ("utf8_bytes", "PositiveU64"), ("host_observed_units", "PositiveU64")]),
        record("ContextBudgetProfileV1", [("schema_version", "Const<1>"), ("profile_id", "ContextBudgetProfileIdV1"), ("release_ref", "ReleaseRefV1"), ("host_ref", "HostRefV1"), ("renderer_or_meter_ref", "MeterRefV1"), ("measurement_procedure_ref", "ProcedureRefV1"), ("admitted_resource_refs", "NonEmptyUniqueVec<ResourceRefV1>"), ("maximum_utf8_bytes", "PositiveU64"), ("maximum_host_observed_units", "PositiveU64"), ("measurements", "NonEmptyUniqueVec<ContextBudgetMeasurementV1>")], ["Release and host evidence only; not a universal product cap", "every admitted combined closure measured"]),
        enumeration("SetupModeV1", SETUP_MODES),
        union("SetupModeRequestV1", [("Require", "SetupModeV1"), ("AcceptUniqueEligible", "Unit")], ["missing/null/default cannot construct a request"]),
        union("AcquisitionContextV1", [("ActiveStore", "ActiveStoreAcquisitionV1"), ("PreStore", "PreStoreAcquisitionV1"), ("NoStoreInstallationGenesis", "NoStoreInstallationGenesisV1")]),
        record("SetupFactViewV1", [("schema_version", "Const<1>"), ("resolver_resource_and_release_catalog_closure", "ResolverReleaseCatalogClosureRefV1"), ("acquisition_context", "AcquisitionContextV1"), ("locality_subject", "ExactLocalitySubjectRefV1"), ("source_owner_fact_commitments", "NonEmptyUniqueOrderedVec<SourceOwnerFactCommitmentV1>"), ("advertised_operation_binding", "SetupAdvertisedOperationBindingV1")], ["one coherent locality", "Operation does not contribute to eligibility"]),
        union("SetupModeResolutionOutcomeV1", [("Selected", "Tuple<SetupModeV1,SetupAdvertisedOperationBindingV1>"), ("Ambiguous", "Const<MultipleEligibleModes>"), ("Blocked", "SetupModeBlockedReasonV1")]),
        record("SetupModeResolutionV1", [("schema_version", "Const<1>"), ("resolver_resource_and_release_catalog_closure", "ResolverReleaseCatalogClosureRefV1"), ("fact_view_commitment", "NonZeroSha256"), ("request_commitment", "NonZeroSha256"), ("advertised_operation_binding", "SetupAdvertisedOperationBindingV1"), ("outcome", "SetupModeResolutionOutcomeV1")], ["facts first, request second, Operation validation last", "exactly one Ambiguous and 21 Blocked reasons"]),
        union("SkillActivationAcquisitionContextV1", [("ActiveStore", "RepositoryOrInstallationStoreGenerationV1"), ("Bootstrap", "RepositoryOrInstallationBootstrapCommitmentV1")]),
        record("SkillActivationSubjectV1", [("schema_version", "Const<1>"), ("activation_acquisition_id", "NominalFreshAcquisitionIdV1"), ("acquisition_context", "SkillActivationAcquisitionContextV1"), ("release_ref", "ReleaseRefV1"), ("root_skill_resource_ref", "ExactRootSkillResourceRefV1")]),
        union("SkillActivationRecipeResolutionV1", [("BootstrapNoRecipe", "Unit"), ("PacketAdmission", "Tuple<SelectedJobRecipeAdmissionV1,NoRecipeOrAdmittedOneToTwoV1>")], ["Refused is structurally unrepresentable", "BootstrapNoRecipe only pre-Packet Setup"]),
        record("LoadedResourceClosureV1", [("job_resource_ref", "ExactlyOneInstructionResourceRefV1"), ("direct_method_resource_refs", "BoundedUniqueOrderedVec<InstructionResourceRefV1,0,4>"), ("tdd_child_resource_refs", "BoundedUniqueOrderedVec<InstructionResourceRefV1,0,5>"), ("research_example_resource_ref", "Optional<InstructionResourceRefV1>"), ("recipe_resource_refs", "BoundedUniqueOrderedVec<RecipeResourceRefV1,0,2>"), ("closure_digest", "NonZeroSha256")]),
        record("SkillActivationPayloadV1", [("selected_route", "SelectedJobRouteSnapshotV1"), ("capability_resolution", "SelectedCapabilityMethodResolutionV1"), ("recipe_resolution", "SkillActivationRecipeResolutionV1"), ("context_budget_profile_ref", "ContextBudgetProfileRefV1"), ("loaded_resource_closure", "LoadedResourceClosureV1")], ["Ambiguous, Blocked and Refused structurally unrepresentable", "all Resources one Release and relation-valid"]),
        record("SkillActivationCandidateV1", [("schema_version", "Const<1>"), ("subject", "SkillActivationSubjectV1"), ("payload", "SkillActivationPayloadV1"), ("subject_commitment", "NonZeroSha256"), ("payload_commitment", "NonZeroSha256"), ("candidate_commitment", "NonZeroSha256")], ["commitments use separate domain tags", "ephemeral non-bearer; zero publication is allowed"]),
        record("LegacySkillActivationImportV1", [("schema_version", "Const<1>"), ("source_format", "Const<FORMAT-RUN-EVENT-V1>"), ("source_file_hash", "NonZeroSha256"), ("source_path_bytes", "NonEmptyBytes"), ("record_ordinal", "U64"), ("byte_start", "U64"), ("byte_length", "PositiveU64"), ("newline_state", "LegacyNewlineStateV1"), ("raw_record_hash", "NonZeroSha256"), ("parse_status", "LegacyActivationParseStatusV1"), ("raw_event_spelling", "Optional<Bytes>"), ("skill_name", "Optional<Bytes>"), ("session_annotation", "Optional<Bytes>"), ("agent_runtime_annotation", "Optional<Bytes>"), ("activation_mode_annotation", "Optional<Bytes>"), ("timestamp_annotations", "UniqueVec<Bytes>"), ("disposition", "LegacySkillActivationDispositionV1"), ("reason", "LegacySkillActivationImportReasonV1")], ["MappedNormative is absent", "rerun identity is source hash plus byte range", "timestamps never reorder"]),
        record("McpToolDescriptorV1", [("name", "McpToolNameV1"), ("read_only", "Const<true>"), ("writes", "Const<false>"), ("network_io", "Const<false>")]),
    ]
    names = [schema["name"] for schema in schemas]
    if len(names) != len(set(names)):
        raise SystemExit("duplicate public schema definition")
    return sorted(schemas, key=lambda schema: schema["name"])


def build_activation_contract(repo: Path) -> None:
    generated = repo / "contracts/vnext/catalogs/generated"
    observation = json.loads((generated / "catalog-01-observation.json").read_text())
    action = json.loads((generated / "catalog-09-action-spec.json").read_text())
    effect = json.loads((generated / "catalog-02-effect.json").read_text())
    observation_row = next(row for row in observation["descriptors"] if row["value"][1] == "SkillActivation")
    action_row = next(row for row in action["descriptors"] if row["value"][1] == "PublishObservation")
    contract = {
        "schema": "maestro.vnext.skill-activation-contract.v1",
        "candidate_only": True,
        "runtime_activation": False,
        "runtime_registration": False,
        "subject_ordered_fields": ["schema_version", "activation_acquisition_id", "acquisition_context", "release_ref", "root_skill_resource_ref"],
        "payload_ordered_fields": ["selected_route", "capability_resolution", "recipe_resolution", "context_budget_profile_ref", "loaded_resource_closure"],
        "candidate_ordered_fields": ["schema_version", "subject", "payload", "subject_commitment", "payload_commitment", "candidate_commitment"],
        "acquisition_contexts": ["ActiveStore.Repository", "ActiveStore.Installation", "Bootstrap.RepositoryBootstrap", "Bootstrap.InstallationBootstrap"],
        "selected_route_reason_set": list(ROUTE_SELECTED),
        "candidate_forbidden_route_reasons": ROUTE_AMBIGUOUS + ROUTE_BLOCKED,
        "capability_outcomes": ["Selected"],
        "recipe_resolution": ["BootstrapNoRecipe", "PacketAdmission.NoRecipe", "PacketAdmission.Admitted[1..2]"],
        "loaded_resource_closure_cardinality": {"job": "1", "direct_methods": "0..4", "tdd_children": "0..5", "research_example": "0..1", "recipes": "0..2", "closure_digest": "1"},
        "commitment_domains": {
            "subject": "maestro.vnext.skill-activation-subject.v1",
            "payload": "maestro.vnext.skill-activation-payload.v1",
            "loaded_closure": "maestro.vnext.skill-activation-loaded-closure.v1",
            "candidate": "maestro.vnext.skill-activation-candidate.v1",
        },
        "evidence_catalog_bindings": {
            "observation_member_count": len(observation["descriptors"]),
            "skill_activation_tag": observation_row["value"][0],
            "skill_activation_descriptor_id": observation_row["descriptor_id"],
            "observation_manifest_id": observation["manifest_id"],
            "action_member_count": len(action["descriptors"]),
            "publish_observation_tag": action_row["value"][0],
            "publish_observation_descriptor_id": action_row["descriptor_id"],
            "action_manifest_id": action["manifest_id"],
            "effect_origin_member_count": len(effect["descriptors"]),
            "effect_origin_manifest_id": effect["manifest_id"],
            "activation_specific_membership_delta": 0,
        },
        "predecessor_non_current_evidence": {
            "action_manifest_id": "sha256:bcc5d3ca6c84ae1d293bd31d5729d852435279b132d6b36feff303877dafb050",
            "publish_observation_tag": 30,
            "descriptor_id": "b6a89e1621b0a21bd5473dd4d8b88ab42b836210e6f8f106070e8c16d986f7a1",
            "current_selector": False,
        },
        "publication": {"candidate_per_complete_acquisition": 1, "observations_per_candidate": "0..1", "passive_writes": 0, "additional_resource_reads": 0, "mcp_tools_added": 0},
        "legacy_import": {
            "source_format": "FORMAT-RUN-EVENT-V1",
            "dispositions": ["MappedHistoricalNonBearer", "OpaquePreserved", "Quarantined", "UnavailablePreexistingLoss"],
            "forbidden_disposition": "MappedNormative",
            "inactive_skill_names": ["ask-maestro", "maestro-audit", "maestro-card", "maestro-design", "maestro-research", "maestro-setup", "maestro-witness", "maestro-work"],
        },
        "illegal_union_mutants": [
            "AmbiguousRouteCandidate", "BlockedRouteCandidate", "AmbiguousCapabilityCandidate",
            "BlockedCapabilityCandidate", "RefusedRecipeCandidate", "BootstrapNonSetupCandidate",
            "BootstrapWithRecipeCandidate", "PacketBootstrapNoRecipeCandidate",
            "MixedReleaseClosure", "ZeroClosureDigest", "DuplicateResourceClosure",
            "OverBoundClosure", "MappedNormativeLegacyImport",
        ],
    }
    dump(repo / "contracts/vnext/public/skill_activation_contract.v1.json", contract)


def build_public_contract(repo: Path) -> None:
    definitions = public_schema_definitions()
    dump(
        repo / "contracts/vnext/public/public_contracts.v1.json",
        {
            "schema": "maestro.vnext.public-contracts.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "schema_definition_count": len(definitions),
            "schema_definitions": definitions,
            "closed_totals": {
                "recipes": 10, "continuation_profiles": 2, "selection_shapes": 30,
                "recipe_return_reasons": 30, "recipe_return_vectors": 196,
                "job_recipe_positive": 22, "job_recipe_negative": 48,
                "job_recipe_admitted_applications": 66, "job_recipe_refused_applications": 144,
                "jobs": 7, "instruction_resources": 31, "direct_methods": 17,
                "job_method_positive": 19, "job_method_negative": 100,
                "review_auxiliary_positive": 4, "review_auxiliary_negative": 11,
                "review_admitted_subsets": 13, "review_refused_subsets": 27,
                "tdd_children": 5, "tdd_direct_job_child_refusals": 35,
                "research_examples_positive": 1, "research_examples_negative": 6,
                "setup_modes": 7, "setup_ambiguous_reasons": 1, "setup_blocked_reasons": 21,
                "setup_action_rows": 145, "setup_ceremony_rows": 11,
                "observation_kinds": 43, "action_specs": 145, "effect_origins": 23,
                "mcp_tools": 2, "project_mcp_tools": 0,
            },
            "semantic_artifacts": [
                "recipe_selection_application_vectors.v1.json",
                "recipe_return_reasons.v1.json",
                "job_recipe_eligibility_vectors.v1.json",
                "job_route_contract.v1.json",
                "capability_method_contracts.v1.json",
                "context_budget_profiles.v1.json",
                "setup_operation_compatibility.v1.json",
                "skill_activation_contract.v1.json",
                "v1_skill_ledger.v1.json",
            ],
            "prohibitions": {
                "runtime_activation": True,
                "aliases": True,
                "private_job_or_method_map": True,
                "fallback_or_latest_selection": True,
                "caller_supplied_frontier": True,
                "third_mcp_tool": True,
                "project_mcp_tool": True,
                "passive_skill_activation_publication": True,
                "mapped_normative_legacy_activation": True,
                "recommendation_outside_projection": True,
            },
        },
    )


def build_mcp_source(repo: Path) -> None:
    dump(
        repo / "embedded/vnext/adapter/mcp-tools.v1.json",
        {
            "schema": "maestro.vnext.mcp-tool-source.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "scope": "global-user-agent-installation",
            "tools": [
                {
                    "name": "maestro_packet",
                    "read_only": True,
                    "writes": False,
                    "network_io": False,
                    "request_schema": "McpPacketReadRequestV1",
                    "response_schema": "McpPacketReadEnvelopeV1",
                    "request_modes": ["BootstrapNoRecipeV1", "DiscoverSelectionContextV1", "ProjectV1"],
                    "response_outcomes": ["Packet", "SelectionContext", "NoActiveStore", "Unavailable", "Stale", "Incompatible"],
                },
                {
                    "name": "maestro_cli_search",
                    "read_only": True,
                    "writes": False,
                    "network_io": False,
                    "request_schema": "McpCliSearchRequestV1",
                    "response_schema": "McpCliSearchEnvelopeV1",
                    "cursor_contract": "Complete=Absent; BoundedTruncated=Present",
                    "hit_classifications": ["PureRead", "Action", "Ceremony"],
                },
            ],
            "project_tools": [],
            "governed_operation_execution": "installation-bound CLI",
        },
    )


def current_action_families() -> dict[str, list[str]]:
    families = {name: list(members) for name, members in BASE_ACTION_FAMILIES.items()}
    families["Work"] = families["Work"] + [
        "SubmitWorkCompletion", "RejectWorkCompletion", "ReturnWorkForRepair",
    ]
    families["Step"] = [
        "SubmitStep", "SatisfyStep", "RejectStepSubmission", "RecoverStepSubmission",
    ]
    families["Execution"].remove("SubmitStep")
    families["Execution"].extend(
        [
            "WithdrawEffectIntent", "WithdrawBootstrapMandateInteractionEffect",
            "WithdrawContinuityMaintenanceEffect",
        ]
    )
    order = [
        "Work", "Step", "Contract", "Design", "Decision", "Execution", "Evidence",
        "Authority", "Coordination", "Planning", "Persistence", "Distribution",
        "SearchMaintenance", "Memory", "Intake", "Research",
    ]
    return {name: families[name] for name in order}


def action_compatibility(family: str, action: str) -> tuple[list[str], str]:
    if action in {"AdoptManagedRegion", "TransferWholeFileCustody"}:
        return ["Adopt"], "MatchingAdoptTargetOrCustodySubject"
    if action in {"RecoverDistributionTransaction", "RollbackDistributionTransaction"}:
        return [], "TransactionRecoveryOrRollbackDoesNotValidateOrdinarySetupMode"
    if family == "Distribution":
        return ["Install", "Migrate", "Update", "Repair", "Rollback", "Uninstall"], "ExactSameDomainDistributionPlanIntent"
    return [], "NoSetupMode"


def ceremony_compatibility(ceremony: str) -> tuple[str, list[str], str]:
    if ceremony == "InstallationContextGenesis":
        return "NoStoreInstallationGenesis", ["Install", "Adopt", "Migrate"], "ExplicitFactFirstJourneyLabelOnly"
    if ceremony in {"RepositoryV1Cutover", "InstallationV1Cutover"}:
        return "PreStore", ["Migrate"], "ExactTypedCutover"
    return "PreStore", [], "RecoveryOrAdmissionDoesNotValidateOrdinarySetupMode"


def build_setup_compatibility(repo: Path) -> None:
    generated = repo / "contracts/vnext/catalogs/generated"
    grammar_path = generated / "catalog-profile-grammar-v1.json"
    action_path = generated / "catalog-09-action-spec.json"
    ceremony_path = generated / "catalog-05-ceremony.json"
    grammar = json.loads(grammar_path.read_text(encoding="utf-8"))
    action_catalog = json.loads(action_path.read_text(encoding="utf-8"))
    ceremony_catalog = json.loads(ceremony_path.read_text(encoding="utf-8"))
    if action_catalog["publication_state"] != "inactive_candidate" or ceremony_catalog["publication_state"] != "inactive_candidate":
        raise SystemExit("Setup compatibility may bind only inactive Stage-0 catalogs")

    owner_names = {row["tag"]: row["name"] for row in grammar["owner_profiles"]}
    action_symbols = {row["global_tag"]: row for row in grammar["action_leaf_symbols"]}
    ceremony_symbols = {row["tag"]: row for row in grammar["ceremony_symbols"]}
    action_owner_rows = {row[0]: row for row in action_catalog["primary_owner_relation"]["rows"]}
    ceremony_owner_rows = {row[0]: row for row in ceremony_catalog["primary_owner_relation"]["rows"]}

    action_rows = []
    family_counts: Counter[str] = Counter()
    for descriptor in action_catalog["descriptors"]:
        value = descriptor["value"]
        tag, name, owner_ref, family_tag, family_local_tag = value[:5]
        symbol = action_symbols.get(tag)
        owner_tag, owner_identity = owner_ref
        expected_owner_row = [tag, owner_tag, owner_identity]
        if (
            symbol is None
            or symbol["name"] != name
            or symbol["family_tag"] != family_tag
            or symbol["family_local_tag"] != family_local_tag
            or action_owner_rows.get(tag) != expected_owner_row
            or symbol["owner"] != owner_names.get(owner_tag)
        ):
            raise SystemExit(f"ActionSpec catalog/grammar owner mismatch at tag {tag}")
        family = symbol["owner"]
        family_counts[family] += 1
        modes, predicate = action_compatibility(family, name)
        action_rows.append(
            {
                "catalog_tag": tag,
                "operation_kind": "Action",
                "name": name,
                "descriptor_id": descriptor["descriptor_id"],
                "family": family,
                "family_tag": family_tag,
                "family_local_tag": family_local_tag,
                "primary_owner": owner_names[owner_tag],
                "primary_owner_tag": owner_tag,
                "primary_owner_descriptor_id": owner_identity["bytes"],
                "catalog_context": "ActiveStore",
                "compatible_setup_modes": modes,
                "required_binding_predicate": predicate,
                "operation_never_contributes_to_eligibility": True,
            }
        )
    ceremony_rows = []
    context_names = {2: "NoStoreInstallationGenesis", 3: "PreStore"}
    for descriptor in ceremony_catalog["descriptors"]:
        value = descriptor["value"]
        tag, name, owner_ref, request_modes, _effect_origins, context_tag = value
        symbol = ceremony_symbols.get(tag)
        owner_tag, owner_identity = owner_ref
        expected_owner_row = [tag, owner_tag, owner_identity]
        if (
            symbol is None
            or symbol["name"] != name
            or ceremony_owner_rows.get(tag) != expected_owner_row
            or symbol["owner"] != owner_names.get(owner_tag)
            or context_tag not in context_names
        ):
            raise SystemExit(f"Ceremony catalog/grammar owner or context mismatch at tag {tag}")
        context, modes, predicate = ceremony_compatibility(name)
        if context != context_names[context_tag]:
            raise SystemExit(f"Setup/Ceremony context mismatch at tag {tag}")
        ceremony_rows.append(
            {
                "catalog_tag": tag,
                "operation_kind": "Ceremony",
                "name": name,
                "descriptor_id": descriptor["descriptor_id"],
                "primary_owner": owner_names[owner_tag],
                "primary_owner_tag": owner_tag,
                "primary_owner_descriptor_id": owner_identity["bytes"],
                "catalog_context": context,
                "catalog_context_tag": context_tag,
                "request_mode_tags": request_modes,
                "compatible_setup_modes": modes,
                "required_binding_predicate": predicate,
                "operation_never_contributes_to_eligibility": True,
            }
        )
    dump(
        repo / "contracts/vnext/public/setup_operation_compatibility.v1.json",
        {
            "schema": "maestro.vnext.setup-operation-compatibility.v1",
            "candidate_only": True,
            "runtime_activation": False,
            "runtime_registration": False,
            "catalog_bindings": {
                "catalog_profile_grammar_id": grammar["catalog_profile_grammar"]["catalog_profile_grammar_id"],
                "action_spec_manifest_id": action_catalog["manifest_id"],
                "action_spec_file_sha256": hashlib.sha256(action_path.read_bytes()).hexdigest(),
                "ceremony_manifest_id": ceremony_catalog["manifest_id"],
                "ceremony_file_sha256": hashlib.sha256(ceremony_path.read_bytes()).hexdigest(),
            },
            "action_family_counts": dict(family_counts),
            "action_row_count": len(action_rows),
            "ceremony_row_count": len(ceremony_rows),
            "action_rows": action_rows,
            "ceremony_rows": ceremony_rows,
            "laws": [
                "Eligibility derives only from canonical owner facts.",
                "Compatibility validates one already advertised Operation after request resolution.",
                "An Operation mismatch never selects another mode or another Operation.",
                "Actions are illegal in no-store and pre-store contexts; Ceremonies are illegal in active-store context.",
                "RecoveryRequired and EffectInDoubt dominate every ordinary Setup mode.",
            ],
        },
    )


def main() -> None:
    global CHECK_ONLY
    repo = Path(__file__).resolve().parents[3]
    parser = argparse.ArgumentParser()
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    CHECK_ONLY = args.check
    recipe_hashes, profile_hashes = build_recipe_resources(repo)
    build_selection_application_vectors(repo, recipe_hashes, profile_hashes)
    build_return_reasons(repo)
    build_job_recipe_and_route(repo)
    build_capability_contracts(repo)
    build_context_budget_profiles(repo, recipe_hashes, profile_hashes)
    build_setup_compatibility(repo)
    build_activation_contract(repo)
    build_public_contract(repo)
    build_mcp_source(repo)
    receipt = {
        "schema": "maestro.vnext.public-literal-build-receipt.v1",
        "mode": "check" if CHECK_ONLY else "write",
        "status": "pass" if not CHECK_MISMATCHES else "fail",
        "mismatches": CHECK_MISMATCHES,
    }
    print(json.dumps(receipt, indent=2, sort_keys=True))
    if CHECK_MISMATCHES:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
