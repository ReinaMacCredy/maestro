use std::collections::{BTreeMap, BTreeSet};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::domain::orchestration::literals::{
    BoundedContinuationProfileV1, ExactRecipeSelectionV1, RecipeApplicationV1, RecipeIdV1,
};

pub const PUBLIC_SKILL_IDS_V1: [&str; 1] = ["maestro"];
pub const JOB_NAMES_V1: [&str; 7] = [
    "Setup", "Research", "Design", "Review", "Execute", "Recover", "Adapt",
];
pub const INSTRUCTION_RESOURCE_PATHS_V1: [&str; 31] = [
    "skills/maestro/SKILL.md",
    "skills/maestro/jobs/setup.md",
    "skills/maestro/jobs/research.md",
    "skills/maestro/jobs/design.md",
    "skills/maestro/jobs/review.md",
    "skills/maestro/jobs/execute.md",
    "skills/maestro/jobs/recover.md",
    "skills/maestro/jobs/adapt.md",
    "skills/maestro/methods/ddd.md",
    "skills/maestro/methods/domain-model.md",
    "skills/maestro/methods/grilling.md",
    "skills/maestro/methods/prd.md",
    "skills/maestro/methods/architecture-deepening.md",
    "skills/maestro/methods/probe.md",
    "skills/maestro/methods/generate-filter.md",
    "skills/maestro/methods/qa-baseline.md",
    "skills/maestro/methods/audit.md",
    "skills/maestro/methods/architecture-review.md",
    "skills/maestro/methods/adversarial-review.md",
    "skills/maestro/methods/qa-replay.md",
    "skills/maestro/methods/close-review.md",
    "skills/maestro/methods/verification.md",
    "skills/maestro/methods/tdd.md",
    "skills/maestro/methods/simplify.md",
    "skills/maestro/methods/extension-law.md",
    "skills/maestro/methods/tdd/test-design.md",
    "skills/maestro/methods/tdd/interface-design.md",
    "skills/maestro/methods/tdd/mocking.md",
    "skills/maestro/methods/tdd/refactoring.md",
    "skills/maestro/methods/tdd/deep-modules.md",
    "skills/maestro/examples/research.md",
];
pub const METHOD_NAMES_V1: [&str; 17] = [
    "DDD",
    "DomainModel",
    "Grilling",
    "PRD",
    "ArchitectureDeepening",
    "Probe",
    "GenerateAndFilter",
    "QABaseline",
    "Audit",
    "ArchitectureReview",
    "AdversarialReview",
    "QAReplay",
    "CloseReview",
    "Verification",
    "TDD",
    "Simplify",
    "ExtensionLaw",
];
pub const JOB_METHOD_POSITIVE_CELLS_V1: usize = 19;
pub const JOB_METHOD_NEGATIVE_CELLS_V1: usize = 100;
pub const REVIEW_AUXILIARY_POSITIVE_CELLS_V1: usize = 4;
pub const REVIEW_AUXILIARY_NEGATIVE_CELLS_V1: usize = 11;
pub const REVIEW_SUBSET_ADMITTED_SHAPES_V1: usize = 13;
pub const REVIEW_SUBSET_REFUSED_SHAPES_V1: usize = 27;
pub const TDD_CHILD_NEEDS_V1: usize = 5;
pub const TDD_CHILD_REFUSED_SHAPES_V1: usize = 35;
pub const RESEARCH_EXAMPLE_POSITIVE_EDGES_V1: usize = 1;
pub const RESEARCH_EXAMPLE_NEGATIVE_EDGES_V1: usize = 6;
pub const SKILL_LEDGER_ROWS_V1: usize = 35;
pub const SKILL_LEDGER_REWRITE_ROWS_V1: usize = 19;
pub const SKILL_LEDGER_REPLACE_ROWS_V1: usize = 9;
pub const SKILL_LEDGER_MIGRATION_ONLY_ROWS_V1: usize = 7;
pub const SKILL_LEDGER_SEMANTIC_DESTINATIONS_V1: usize = 21;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum LegacySkillDispositionV1 {
    Rewrite,
    Replace,
    MigrationOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct LegacySkillLedgerRowV1 {
    pub source_path: &'static str,
    pub disposition: LegacySkillDispositionV1,
    pub active_destination: Option<&'static str>,
}

pub(crate) const LEGACY_SKILL_LEDGER_V1: [LegacySkillLedgerRowV1; 35] = [
    legacy_skill(
        "ask-maestro/SKILL.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/SKILL.md"),
    ),
    legacy_skill(
        "ask-maestro/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
    legacy_skill(
        "maestro-audit/SKILL.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/jobs/review.md+skills/maestro/methods/audit.md"),
    ),
    legacy_skill(
        "maestro-audit/reference/architecture-review.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/architecture-review.md"),
    ),
    legacy_skill(
        "maestro-audit/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
    legacy_skill(
        "maestro-card/SKILL.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/jobs/execute.md"),
    ),
    legacy_skill(
        "maestro-card/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
    legacy_skill(
        "maestro-card/reference/feature.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/jobs/design.md+skills/maestro/jobs/execute.md"),
    ),
    legacy_skill(
        "maestro-card/reference/intake.md",
        LegacySkillDispositionV1::Rewrite,
        Some("recipe:intake-triage"),
    ),
    legacy_skill(
        "maestro-card/reference/loop.md",
        LegacySkillDispositionV1::Replace,
        Some("profiles:bounded-continuation"),
    ),
    legacy_skill(
        "maestro-card/reference/qa-baseline.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/qa-baseline.md"),
    ),
    legacy_skill(
        "maestro-card/reference/qa-slice.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/methods/qa-replay.md"),
    ),
    legacy_skill(
        "maestro-card/reference/simplify.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/simplify.md"),
    ),
    legacy_skill(
        "maestro-card/reference/tdd.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/tdd.md"),
    ),
    legacy_skill(
        "maestro-card/reference/tdd/deep-modules.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/tdd/deep-modules.md"),
    ),
    legacy_skill(
        "maestro-card/reference/tdd/interface-design.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/tdd/interface-design.md"),
    ),
    legacy_skill(
        "maestro-card/reference/tdd/mocking.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/tdd/mocking.md"),
    ),
    legacy_skill(
        "maestro-card/reference/tdd/refactoring.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/tdd/refactoring.md"),
    ),
    legacy_skill(
        "maestro-card/reference/tdd/tests.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/methods/tdd/test-design.md"),
    ),
    legacy_skill(
        "maestro-card/reference/verify.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/verification.md+skills/maestro/methods/adversarial-review.md"),
    ),
    legacy_skill(
        "maestro-card/reference/work.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/jobs/execute.md"),
    ),
    legacy_skill(
        "maestro-design/SKILL.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/jobs/design.md"),
    ),
    legacy_skill(
        "maestro-design/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
    legacy_skill(
        "maestro-design/reference/ddd.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/ddd.md"),
    ),
    legacy_skill(
        "maestro-design/reference/deepening-candidate.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/methods/architecture-deepening.md"),
    ),
    legacy_skill(
        "maestro-design/reference/domain-model.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/domain-model.md"),
    ),
    legacy_skill(
        "maestro-design/reference/grilling.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/grilling.md"),
    ),
    legacy_skill(
        "maestro-design/reference/prd.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/methods/prd.md"),
    ),
    legacy_skill(
        "maestro-research/SKILL.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/jobs/research.md"),
    ),
    legacy_skill(
        "maestro-research/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
    legacy_skill(
        "maestro-research/reference/examples.md",
        LegacySkillDispositionV1::Rewrite,
        Some("skills/maestro/examples/research.md"),
    ),
    legacy_skill(
        "maestro-setup/SKILL.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/jobs/setup.md+recipe:setup"),
    ),
    legacy_skill(
        "maestro-setup/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
    legacy_skill(
        "maestro-witness/SKILL.md",
        LegacySkillDispositionV1::Replace,
        Some("skills/maestro/jobs/review.md+skills/maestro/methods/close-review.md"),
    ),
    legacy_skill(
        "maestro-witness/reference/cli.md",
        LegacySkillDispositionV1::MigrationOnly,
        None,
    ),
];

const fn legacy_skill(
    source_path: &'static str,
    disposition: LegacySkillDispositionV1,
    active_destination: Option<&'static str>,
) -> LegacySkillLedgerRowV1 {
    LegacySkillLedgerRowV1 {
        source_path,
        disposition,
        active_destination,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalInstructionResourceV1 {
    pub logical_path: &'static str,
    pub embedded_source_path: &'static str,
    pub content_sha256: [u8; 32],
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CanonicalAgentResourceInventoryV1 {
    resources: [CanonicalInstructionResourceV1; 31],
    legacy_ledger: [LegacySkillLedgerRowV1; 35],
    resource_closure: [u8; 32],
    legacy_ledger_closure: [u8; 32],
}

impl CanonicalAgentResourceInventoryV1 {
    pub(crate) fn load_embedded() -> Result<Self, CapabilityLiteralError> {
        let resources = std::array::from_fn(|index| CanonicalInstructionResourceV1 {
            logical_path: INSTRUCTION_RESOURCE_PATHS_V1[index],
            embedded_source_path: CANONICAL_INSTRUCTION_EMBEDDED_PATHS_V1[index],
            content_sha256: Sha256::digest(CANONICAL_INSTRUCTION_BYTES_V1[index]).into(),
        });
        let inventory = Self {
            resource_closure: inventory_closure(
                b"maestro.vnext.canonical-agent-instruction-resources.v1",
                resources
                    .iter()
                    .map(|row| (row.logical_path, row.content_sha256)),
            ),
            legacy_ledger_closure: legacy_ledger_closure(&LEGACY_SKILL_LEDGER_V1),
            resources,
            legacy_ledger: LEGACY_SKILL_LEDGER_V1,
        };
        inventory.validate()?;
        Ok(inventory)
    }

    pub(in crate::domain) fn resources(&self) -> &[CanonicalInstructionResourceV1; 31] {
        &self.resources
    }

    pub(in crate::domain) fn legacy_ledger(&self) -> &[LegacySkillLedgerRowV1; 35] {
        &self.legacy_ledger
    }

    pub(crate) const fn resource_closure(&self) -> [u8; 32] {
        self.resource_closure
    }

    pub(crate) const fn legacy_ledger_closure(&self) -> [u8; 32] {
        self.legacy_ledger_closure
    }

    fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if self.resources.map(|row| row.logical_path) != INSTRUCTION_RESOURCE_PATHS_V1
            || self.resources.iter().any(|row| {
                !row.embedded_source_path
                    .starts_with("embedded/vnext/capability/")
                    || row.content_sha256 == [0; 32]
            })
            || self
                .resources
                .iter()
                .map(|row| row.logical_path)
                .collect::<BTreeSet<_>>()
                .len()
                != INSTRUCTION_RESOURCE_PATHS_V1.len()
            || self
                .legacy_ledger
                .iter()
                .map(|row| row.source_path)
                .collect::<BTreeSet<_>>()
                .len()
                != SKILL_LEDGER_ROWS_V1
        {
            return Err(CapabilityLiteralError::InvalidAgentResourceInventory);
        }
        let dispositions = self
            .legacy_ledger
            .iter()
            .fold([0_usize; 3], |mut counts, row| {
                match row.disposition {
                    LegacySkillDispositionV1::Rewrite => counts[0] += 1,
                    LegacySkillDispositionV1::Replace => counts[1] += 1,
                    LegacySkillDispositionV1::MigrationOnly => counts[2] += 1,
                }
                counts
            });
        let semantic_references = self
            .legacy_ledger
            .iter()
            .filter(|row| {
                row.source_path.contains("/reference/")
                    && !row.source_path.ends_with("/reference/cli.md")
                    && row.active_destination.is_some()
            })
            .count();
        if dispositions
            != [
                SKILL_LEDGER_REWRITE_ROWS_V1,
                SKILL_LEDGER_REPLACE_ROWS_V1,
                SKILL_LEDGER_MIGRATION_ONLY_ROWS_V1,
            ]
            || semantic_references != SKILL_LEDGER_SEMANTIC_DESTINATIONS_V1
            || self.legacy_ledger.iter().any(|row| {
                matches!(row.disposition, LegacySkillDispositionV1::MigrationOnly)
                    != row.active_destination.is_none()
            })
            || self.resource_closure == [0; 32]
            || self.legacy_ledger_closure == [0; 32]
        {
            return Err(CapabilityLiteralError::InvalidAgentResourceInventory);
        }
        Ok(())
    }
}

fn inventory_closure<'a>(
    domain: &[u8],
    rows: impl IntoIterator<Item = (&'a str, [u8; 32])>,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update((domain.len() as u64).to_be_bytes());
    digest.update(domain);
    for (path, content_sha256) in rows {
        digest.update((path.len() as u64).to_be_bytes());
        digest.update(path.as_bytes());
        digest.update(content_sha256);
    }
    digest.finalize().into()
}

fn legacy_ledger_closure(rows: &[LegacySkillLedgerRowV1; 35]) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(b"maestro.vnext.legacy-skill-ledger.v1\0");
    for row in rows {
        digest.update((row.source_path.len() as u64).to_be_bytes());
        digest.update(row.source_path.as_bytes());
        digest.update([match row.disposition {
            LegacySkillDispositionV1::Rewrite => 1,
            LegacySkillDispositionV1::Replace => 2,
            LegacySkillDispositionV1::MigrationOnly => 3,
        }]);
        if let Some(destination) = row.active_destination {
            digest.update((destination.len() as u64).to_be_bytes());
            digest.update(destination.as_bytes());
        } else {
            digest.update(0_u64.to_be_bytes());
        }
    }
    digest.finalize().into()
}

const CANONICAL_INSTRUCTION_EMBEDDED_PATHS_V1: [&str; 31] = [
    "embedded/vnext/capability/skills/maestro/SKILL.md",
    "embedded/vnext/capability/skills/maestro/jobs/setup.md",
    "embedded/vnext/capability/skills/maestro/jobs/research.md",
    "embedded/vnext/capability/skills/maestro/jobs/design.md",
    "embedded/vnext/capability/skills/maestro/jobs/review.md",
    "embedded/vnext/capability/skills/maestro/jobs/execute.md",
    "embedded/vnext/capability/skills/maestro/jobs/recover.md",
    "embedded/vnext/capability/skills/maestro/jobs/adapt.md",
    "embedded/vnext/capability/skills/maestro/methods/ddd.md",
    "embedded/vnext/capability/skills/maestro/methods/domain-model.md",
    "embedded/vnext/capability/skills/maestro/methods/grilling.md",
    "embedded/vnext/capability/skills/maestro/methods/prd.md",
    "embedded/vnext/capability/skills/maestro/methods/architecture-deepening.md",
    "embedded/vnext/capability/skills/maestro/methods/probe.md",
    "embedded/vnext/capability/skills/maestro/methods/generate-filter.md",
    "embedded/vnext/capability/skills/maestro/methods/qa-baseline.md",
    "embedded/vnext/capability/skills/maestro/methods/audit.md",
    "embedded/vnext/capability/skills/maestro/methods/architecture-review.md",
    "embedded/vnext/capability/skills/maestro/methods/adversarial-review.md",
    "embedded/vnext/capability/skills/maestro/methods/qa-replay.md",
    "embedded/vnext/capability/skills/maestro/methods/close-review.md",
    "embedded/vnext/capability/skills/maestro/methods/verification.md",
    "embedded/vnext/capability/skills/maestro/methods/tdd.md",
    "embedded/vnext/capability/skills/maestro/methods/simplify.md",
    "embedded/vnext/capability/skills/maestro/methods/extension-law.md",
    "embedded/vnext/capability/skills/maestro/methods/tdd/test-design.md",
    "embedded/vnext/capability/skills/maestro/methods/tdd/interface-design.md",
    "embedded/vnext/capability/skills/maestro/methods/tdd/mocking.md",
    "embedded/vnext/capability/skills/maestro/methods/tdd/refactoring.md",
    "embedded/vnext/capability/skills/maestro/methods/tdd/deep-modules.md",
    "embedded/vnext/capability/skills/maestro/examples/research.md",
];

const CANONICAL_INSTRUCTION_BYTES_V1: [&[u8]; 31] = [
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/SKILL.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/setup.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/research.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/design.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/review.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/execute.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/recover.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/jobs/adapt.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/ddd.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/domain-model.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/grilling.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/prd.md"),
    include_bytes!(
        "../../../embedded/vnext/capability/skills/maestro/methods/architecture-deepening.md"
    ),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/probe.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/generate-filter.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/qa-baseline.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/audit.md"),
    include_bytes!(
        "../../../embedded/vnext/capability/skills/maestro/methods/architecture-review.md"
    ),
    include_bytes!(
        "../../../embedded/vnext/capability/skills/maestro/methods/adversarial-review.md"
    ),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/qa-replay.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/close-review.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/verification.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/tdd.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/simplify.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/extension-law.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/tdd/test-design.md"),
    include_bytes!(
        "../../../embedded/vnext/capability/skills/maestro/methods/tdd/interface-design.md"
    ),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/tdd/mocking.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/tdd/refactoring.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/methods/tdd/deep-modules.md"),
    include_bytes!("../../../embedded/vnext/capability/skills/maestro/examples/research.md"),
];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum InternalJobV1 {
    Setup = 1,
    Research = 2,
    Design = 3,
    Review = 4,
    Execute = 5,
    Recover = 6,
    Adapt = 7,
}

pub type JobV1 = InternalJobV1;

impl InternalJobV1 {
    pub const ALL: [Self; 7] = [
        Self::Setup,
        Self::Research,
        Self::Design,
        Self::Review,
        Self::Execute,
        Self::Recover,
        Self::Adapt,
    ];

    pub const fn name(self) -> &'static str {
        JOB_NAMES_V1[self as usize - 1]
    }

    pub const fn resource_path(self) -> &'static str {
        INSTRUCTION_RESOURCE_PATHS_V1[self as usize]
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum DirectMethodV1 {
    Ddd = 1,
    DomainModel = 2,
    Grilling = 3,
    Prd = 4,
    ArchitectureDeepening = 5,
    Probe = 6,
    GenerateAndFilter = 7,
    QaBaseline = 8,
    Audit = 9,
    ArchitectureReview = 10,
    AdversarialReview = 11,
    QaReplay = 12,
    CloseReview = 13,
    Verification = 14,
    Tdd = 15,
    Simplify = 16,
    ExtensionLaw = 17,
}

pub type MethodV1 = DirectMethodV1;

impl DirectMethodV1 {
    pub const ALL: [Self; 17] = [
        Self::Ddd,
        Self::DomainModel,
        Self::Grilling,
        Self::Prd,
        Self::ArchitectureDeepening,
        Self::Probe,
        Self::GenerateAndFilter,
        Self::QaBaseline,
        Self::Audit,
        Self::ArchitectureReview,
        Self::AdversarialReview,
        Self::QaReplay,
        Self::CloseReview,
        Self::Verification,
        Self::Tdd,
        Self::Simplify,
        Self::ExtensionLaw,
    ];

    pub const fn name(self) -> &'static str {
        METHOD_NAMES_V1[self as usize - 1]
    }

    pub const fn resource_path(self) -> &'static str {
        INSTRUCTION_RESOURCE_PATHS_V1[self as usize + 7]
    }
}

pub const JOB_METHOD_ROWS_V1: [(InternalJobV1, &[DirectMethodV1]); 7] = [
    (InternalJobV1::Setup, &[]),
    (InternalJobV1::Research, &[]),
    (
        InternalJobV1::Design,
        &[
            DirectMethodV1::Ddd,
            DirectMethodV1::DomainModel,
            DirectMethodV1::Grilling,
            DirectMethodV1::Prd,
            DirectMethodV1::ArchitectureDeepening,
            DirectMethodV1::Probe,
            DirectMethodV1::GenerateAndFilter,
            DirectMethodV1::QaBaseline,
        ],
    ),
    (
        InternalJobV1::Review,
        &[
            DirectMethodV1::Audit,
            DirectMethodV1::ArchitectureReview,
            DirectMethodV1::AdversarialReview,
            DirectMethodV1::GenerateAndFilter,
            DirectMethodV1::QaReplay,
            DirectMethodV1::CloseReview,
            DirectMethodV1::Verification,
        ],
    ),
    (
        InternalJobV1::Execute,
        &[DirectMethodV1::Tdd, DirectMethodV1::Simplify],
    ),
    (InternalJobV1::Recover, &[]),
    (
        InternalJobV1::Adapt,
        &[
            DirectMethodV1::ExtensionLaw,
            DirectMethodV1::GenerateAndFilter,
        ],
    ),
];

pub fn job_method_is_admitted(job: InternalJobV1, method: DirectMethodV1) -> bool {
    exact_methods_for_job(job).contains(&method)
}

fn exact_methods_for_job(job: InternalJobV1) -> &'static [DirectMethodV1] {
    JOB_METHOD_ROWS_V1[job as usize - 1].1
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InstructionResourceRefV1 {
    pub logical_path: String,
    pub resource_ref: String,
}

impl InstructionResourceRefV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if !INSTRUCTION_RESOURCE_PATHS_V1.contains(&self.logical_path.as_str())
            || !resource_ref_matches_path(&self.resource_ref, &self.logical_path)
        {
            return Err(CapabilityLiteralError::InvalidInstructionResource);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DirectMethodResourceRefV1 {
    pub method: DirectMethodV1,
    pub instruction_resource: InstructionResourceRefV1,
}

impl DirectMethodResourceRefV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        self.instruction_resource.validate()?;
        if self.instruction_resource.logical_path != self.method.resource_path() {
            return Err(CapabilityLiteralError::InvalidInstructionResource);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobMethodEligibilityRowV1 {
    pub job: InternalJobV1,
    pub direct_method_resource_refs: Vec<DirectMethodResourceRefV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobMethodEligibilityV1 {
    pub schema_version: u64,
    pub capability_job_catalog_ref: String,
    pub direct_method_catalog_ref: String,
    pub rows: Vec<JobMethodEligibilityRowV1>,
}

impl JobMethodEligibilityV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if self.schema_version != 1
            || !is_nonempty_current_ref(&self.capability_job_catalog_ref)
            || !is_nonempty_current_ref(&self.direct_method_catalog_ref)
            || self.rows.len() != InternalJobV1::ALL.len()
        {
            return Err(CapabilityLiteralError::InvalidJobMethodEligibility);
        }
        for (row, expected_job) in self.rows.iter().zip(InternalJobV1::ALL) {
            if row.job != expected_job
                || row
                    .direct_method_resource_refs
                    .iter()
                    .map(|method| method.method)
                    .collect::<Vec<_>>()
                    != exact_methods_for_job(expected_job)
                || row
                    .direct_method_resource_refs
                    .iter()
                    .any(|method| method.validate().is_err())
            {
                return Err(CapabilityLiteralError::InvalidJobMethodEligibility);
            }
        }
        Ok(())
    }

    pub fn resource_for(
        &self,
        job: InternalJobV1,
        method: DirectMethodV1,
    ) -> Option<&DirectMethodResourceRefV1> {
        self.rows
            .get(job as usize - 1)?
            .direct_method_resource_refs
            .iter()
            .find(|resource| resource.method == method)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CapabilityNeedKindV1 {
    UbiquitousLanguageOrContextBoundary = 1,
    StrategicDomainStructure = 2,
    OneUnresolvedHighImpactQuestion = 3,
    ProductRequirementSynthesis = 4,
    DeepenSelectedSolution = 5,
    OneBoundedUncertainty = 6,
    FixedRubricCandidateChoice = 7,
    ContractBoundScenarioSpecs = 8,
    AuditObligations = 9,
    ArchitectureAuditSpecialization = 10,
    FrozenClaimRefutation = 11,
    FrozenScenarioReplay = 12,
    ProofClosureReview = 13,
    ExactClaimFalsification = 14,
    ExecuteRedGreen = 15,
    CurrentGreenSimplification = 16,
    AdaptExtensionBoundary = 17,
    TddBehaviorTestShape = 18,
    TddInterfaceTestability = 19,
    TddExternalBoundarySubstitution = 20,
    TddCurrentGreenRefactor = 21,
    TddModuleDepthLocality = 22,
    ResearchExamples = 23,
}

impl CapabilityNeedKindV1 {
    pub const fn for_method(method: DirectMethodV1) -> Self {
        match method {
            DirectMethodV1::Ddd => Self::UbiquitousLanguageOrContextBoundary,
            DirectMethodV1::DomainModel => Self::StrategicDomainStructure,
            DirectMethodV1::Grilling => Self::OneUnresolvedHighImpactQuestion,
            DirectMethodV1::Prd => Self::ProductRequirementSynthesis,
            DirectMethodV1::ArchitectureDeepening => Self::DeepenSelectedSolution,
            DirectMethodV1::Probe => Self::OneBoundedUncertainty,
            DirectMethodV1::GenerateAndFilter => Self::FixedRubricCandidateChoice,
            DirectMethodV1::QaBaseline => Self::ContractBoundScenarioSpecs,
            DirectMethodV1::Audit => Self::AuditObligations,
            DirectMethodV1::ArchitectureReview => Self::ArchitectureAuditSpecialization,
            DirectMethodV1::AdversarialReview => Self::FrozenClaimRefutation,
            DirectMethodV1::QaReplay => Self::FrozenScenarioReplay,
            DirectMethodV1::CloseReview => Self::ProofClosureReview,
            DirectMethodV1::Verification => Self::ExactClaimFalsification,
            DirectMethodV1::Tdd => Self::ExecuteRedGreen,
            DirectMethodV1::Simplify => Self::CurrentGreenSimplification,
            DirectMethodV1::ExtensionLaw => Self::AdaptExtensionBoundary,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityTypedNeedV1 {
    pub schema_version: u64,
    pub need_kind: CapabilityNeedKindV1,
    pub exact_scope_ref: String,
    pub exact_input_ref: String,
    pub current_canonical_refs: Vec<String>,
    pub invalidation_conditions: Vec<String>,
}

impl CapabilityTypedNeedV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if self.schema_version != 1
            || self.exact_scope_ref.is_empty()
            || self.exact_input_ref.is_empty()
            || self.current_canonical_refs.is_empty()
            || self.invalidation_conditions.is_empty()
            || !all_unique(&self.current_canonical_refs)
            || !all_unique(&self.invalidation_conditions)
        {
            return Err(CapabilityLiteralError::InvalidTypedNeed);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedDirectMethodV1 {
    pub exact_resource_ref: DirectMethodResourceRefV1,
    pub exact_typed_need_ref: CapabilityTypedNeedV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum TddChildV1 {
    TestDesign = 1,
    InterfaceDesign = 2,
    Mocking = 3,
    Refactoring = 4,
    DeepModules = 5,
}

impl TddChildV1 {
    pub const ALL: [Self; 5] = [
        Self::TestDesign,
        Self::InterfaceDesign,
        Self::Mocking,
        Self::Refactoring,
        Self::DeepModules,
    ];

    pub const fn resource_path(self) -> &'static str {
        INSTRUCTION_RESOURCE_PATHS_V1[self as usize + 24]
    }

    pub const fn need_kind(self) -> CapabilityNeedKindV1 {
        match self {
            Self::TestDesign => CapabilityNeedKindV1::TddBehaviorTestShape,
            Self::InterfaceDesign => CapabilityNeedKindV1::TddInterfaceTestability,
            Self::Mocking => CapabilityNeedKindV1::TddExternalBoundarySubstitution,
            Self::Refactoring => CapabilityNeedKindV1::TddCurrentGreenRefactor,
            Self::DeepModules => CapabilityNeedKindV1::TddModuleDepthLocality,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TddChildEligibilityRowV1 {
    pub child: TddChildV1,
    pub child_resource_ref: InstructionResourceRefV1,
    pub required_need_kind: CapabilityNeedKindV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TddChildEligibilityV1 {
    pub schema_version: u64,
    pub exact_tdd_method_resource_ref: DirectMethodResourceRefV1,
    pub rows: Vec<TddChildEligibilityRowV1>,
}

impl TddChildEligibilityV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        self.exact_tdd_method_resource_ref.validate()?;
        if self.schema_version != 1
            || self.exact_tdd_method_resource_ref.method != DirectMethodV1::Tdd
            || self.rows.len() != 5
        {
            return Err(CapabilityLiteralError::InvalidTddChildEligibility);
        }
        for (row, expected) in self.rows.iter().zip(TddChildV1::ALL) {
            row.child_resource_ref.validate()?;
            if row.child != expected
                || row.child_resource_ref.logical_path != expected.resource_path()
                || row.required_need_kind != expected.need_kind()
            {
                return Err(CapabilityLiteralError::InvalidTddChildEligibility);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedTddChildV1 {
    pub exact_resource_ref: InstructionResourceRefV1,
    pub child: TddChildV1,
    pub exact_typed_need_ref: CapabilityTypedNeedV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestedResearchExamplesV1 {
    pub exact_resource_ref: InstructionResourceRefV1,
    pub exact_typed_need_ref: CapabilityTypedNeedV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResearchExampleEligibilityV1 {
    pub schema_version: u64,
    pub research_job: InternalJobV1,
    pub exact_examples_resource_ref: InstructionResourceRefV1,
}

impl ResearchExampleEligibilityV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        self.exact_examples_resource_ref.validate()?;
        if self.schema_version != 1
            || self.research_job != InternalJobV1::Research
            || self.exact_examples_resource_ref.logical_path != INSTRUCTION_RESOURCE_PATHS_V1[30]
        {
            return Err(CapabilityLiteralError::InvalidResearchExampleEligibility);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum ReviewModeV1 {
    Inspect = 1,
    Audit = 2,
    AdversarialReview = 3,
    QaReplay = 4,
    CloseReview = 5,
}

impl ReviewModeV1 {
    pub const ALL: [Self; 5] = [
        Self::Inspect,
        Self::Audit,
        Self::AdversarialReview,
        Self::QaReplay,
        Self::CloseReview,
    ];

    pub const fn primary(self) -> Option<DirectMethodV1> {
        match self {
            Self::Inspect => None,
            Self::Audit => Some(DirectMethodV1::Audit),
            Self::AdversarialReview => Some(DirectMethodV1::AdversarialReview),
            Self::QaReplay => Some(DirectMethodV1::QaReplay),
            Self::CloseReview => Some(DirectMethodV1::CloseReview),
        }
    }

    pub const fn admitted_auxiliaries(self) -> &'static [DirectMethodV1] {
        match self {
            Self::Inspect | Self::AdversarialReview | Self::QaReplay => &[],
            Self::Audit => &[
                DirectMethodV1::ArchitectureReview,
                DirectMethodV1::GenerateAndFilter,
                DirectMethodV1::Verification,
            ],
            Self::CloseReview => &[DirectMethodV1::Verification],
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewModeBasisV1 {
    GateRequirement,
    ExplicitMode,
    InspectDefault,
}

pub type ReviewResolutionBasisV1 = ReviewModeBasisV1;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewModeRequestV1 {
    pub gate_required_modes: Vec<ReviewModeV1>,
    pub explicit_mode: Option<ReviewModeV1>,
    pub exact_input_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewMethodLoadPlanV1 {
    pub primary_method_resource_ref: Option<DirectMethodResourceRefV1>,
    pub auxiliary_method_resource_refs: Vec<DirectMethodResourceRefV1>,
}

impl ReviewMethodLoadPlanV1 {
    pub fn validate_for(&self, mode: ReviewModeV1) -> Result<(), CapabilityLiteralError> {
        if self
            .primary_method_resource_ref
            .as_ref()
            .map(|row| row.method)
            != mode.primary()
            || self
                .auxiliary_method_resource_refs
                .iter()
                .any(|row| row.validate().is_err())
            || !is_strictly_ordered_unique(
                &self
                    .auxiliary_method_resource_refs
                    .iter()
                    .map(|row| row.method)
                    .collect::<Vec<_>>(),
            )
            || self
                .auxiliary_method_resource_refs
                .iter()
                .any(|row| !mode.admitted_auxiliaries().contains(&row.method))
        {
            return Err(CapabilityLiteralError::InvalidReviewLoadPlan);
        }
        if let Some(primary) = &self.primary_method_resource_ref {
            primary.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewModeAmbiguousReasonV1 {
    ConflictingGateRequirements,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewModeBlockedReasonV1 {
    ExplicitModeCannotSatisfyGate,
    StaleInput,
    IncompatibleResourceClosure,
    UnavailableResourceClosure,
    InvalidAuxiliarySubset,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewModeResolutionOutcomeV1 {
    Selected {
        mode: ReviewModeV1,
        basis: ReviewModeBasisV1,
        exact_input_ref: String,
        method_load_plan: ReviewMethodLoadPlanV1,
    },
    Ambiguous(ReviewModeAmbiguousReasonV1),
    Blocked(ReviewModeBlockedReasonV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewModeResolutionV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub exact_selected_job_route_ref: String,
    pub outcome: ReviewModeResolutionOutcomeV1,
}

pub fn resolve_review_mode(
    resolution_basis_ref: String,
    exact_selected_job_route_ref: String,
    request: &ReviewModeRequestV1,
    requested_auxiliaries: &[RequestedDirectMethodV1],
    eligibility: &JobMethodEligibilityV1,
) -> Result<ReviewModeResolutionV1, CapabilityLiteralError> {
    eligibility.validate()?;
    if resolution_basis_ref.is_empty()
        || exact_selected_job_route_ref.is_empty()
        || request.exact_input_ref.is_empty()
    {
        return Err(CapabilityLiteralError::InvalidReviewResolution);
    }
    let gate_modes = request
        .gate_required_modes
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    if gate_modes.len() > 1 {
        return Ok(ReviewModeResolutionV1 {
            schema_version: 1,
            resolution_basis_ref,
            exact_selected_job_route_ref,
            outcome: ReviewModeResolutionOutcomeV1::Ambiguous(
                ReviewModeAmbiguousReasonV1::ConflictingGateRequirements,
            ),
        });
    }
    let (mode, basis) = if let Some(mode) = gate_modes.first() {
        if request
            .explicit_mode
            .is_some_and(|explicit| explicit != *mode)
        {
            return Ok(ReviewModeResolutionV1 {
                schema_version: 1,
                resolution_basis_ref,
                exact_selected_job_route_ref,
                outcome: ReviewModeResolutionOutcomeV1::Blocked(
                    ReviewModeBlockedReasonV1::ExplicitModeCannotSatisfyGate,
                ),
            });
        }
        (*mode, ReviewModeBasisV1::GateRequirement)
    } else if let Some(mode) = request.explicit_mode {
        (mode, ReviewModeBasisV1::ExplicitMode)
    } else {
        (ReviewModeV1::Inspect, ReviewModeBasisV1::InspectDefault)
    };

    let mut auxiliaries = Vec::new();
    for requested in requested_auxiliaries {
        requested.exact_resource_ref.validate()?;
        requested.exact_typed_need_ref.validate()?;
        if requested.exact_typed_need_ref.need_kind
            != CapabilityNeedKindV1::for_method(requested.exact_resource_ref.method)
            || !mode
                .admitted_auxiliaries()
                .contains(&requested.exact_resource_ref.method)
        {
            return Ok(ReviewModeResolutionV1 {
                schema_version: 1,
                resolution_basis_ref,
                exact_selected_job_route_ref,
                outcome: ReviewModeResolutionOutcomeV1::Blocked(
                    ReviewModeBlockedReasonV1::InvalidAuxiliarySubset,
                ),
            });
        }
        auxiliaries.push(requested.exact_resource_ref.clone());
    }
    auxiliaries.sort_by_key(|resource| resource.method);
    if auxiliaries
        .windows(2)
        .any(|pair| pair[0].method == pair[1].method)
    {
        return Err(CapabilityLiteralError::InvalidReviewLoadPlan);
    }
    let primary = mode
        .primary()
        .and_then(|method| eligibility.resource_for(InternalJobV1::Review, method))
        .cloned();
    if mode.primary().is_some() && primary.is_none() {
        return Err(CapabilityLiteralError::InvalidJobMethodEligibility);
    }
    let plan = ReviewMethodLoadPlanV1 {
        primary_method_resource_ref: primary,
        auxiliary_method_resource_refs: auxiliaries,
    };
    plan.validate_for(mode)?;
    Ok(ReviewModeResolutionV1 {
        schema_version: 1,
        resolution_basis_ref,
        exact_selected_job_route_ref,
        outcome: ReviewModeResolutionOutcomeV1::Selected {
            mode,
            basis,
            exact_input_ref: request.exact_input_ref.clone(),
            method_load_plan: plan,
        },
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityMethodIntentV1 {
    pub schema_version: u64,
    pub exact_scope_ref: String,
    pub requested_direct_methods: Vec<RequestedDirectMethodV1>,
    pub requested_tdd_children: Vec<RequestedTddChildV1>,
    pub research_examples: Option<RequestedResearchExamplesV1>,
    pub requested_review_mode: Option<ReviewModeRequestV1>,
}

impl CapabilityMethodIntentV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if self.schema_version != 1 || self.exact_scope_ref.is_empty() {
            return Err(CapabilityLiteralError::InvalidCapabilityMethodIntent);
        }
        let mut direct = BTreeSet::new();
        for request in &self.requested_direct_methods {
            request.exact_resource_ref.validate()?;
            request.exact_typed_need_ref.validate()?;
            if request.exact_typed_need_ref.need_kind
                != CapabilityNeedKindV1::for_method(request.exact_resource_ref.method)
                || !direct.insert(request.exact_resource_ref.method)
            {
                return Err(CapabilityLiteralError::InvalidCapabilityMethodIntent);
            }
        }
        let mut children = BTreeSet::new();
        for request in &self.requested_tdd_children {
            request.exact_resource_ref.validate()?;
            request.exact_typed_need_ref.validate()?;
            if request.exact_resource_ref.logical_path != request.child.resource_path()
                || request.exact_typed_need_ref.need_kind != request.child.need_kind()
                || !children.insert(request.child)
            {
                return Err(CapabilityLiteralError::InvalidCapabilityMethodIntent);
            }
        }
        if !self.requested_tdd_children.is_empty() && !direct.contains(&DirectMethodV1::Tdd) {
            return Err(CapabilityLiteralError::InvalidCapabilityMethodIntent);
        }
        if let Some(examples) = &self.research_examples {
            examples.exact_resource_ref.validate()?;
            examples.exact_typed_need_ref.validate()?;
            if examples.exact_resource_ref.logical_path != INSTRUCTION_RESOURCE_PATHS_V1[30]
                || examples.exact_typed_need_ref.need_kind != CapabilityNeedKindV1::ResearchExamples
            {
                return Err(CapabilityLiteralError::InvalidCapabilityMethodIntent);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityInstructionLoadPlanV1 {
    pub selected_job_resource_ref: InstructionResourceRefV1,
    pub direct_method_resource_refs: Vec<DirectMethodResourceRefV1>,
    pub tdd_child_resource_refs: Vec<InstructionResourceRefV1>,
    pub research_example_resource_ref: Option<InstructionResourceRefV1>,
}

impl CapabilityInstructionLoadPlanV1 {
    pub fn validate_for(&self, job: InternalJobV1) -> Result<(), CapabilityLiteralError> {
        self.selected_job_resource_ref.validate()?;
        if self.selected_job_resource_ref.logical_path != job.resource_path()
            || self.direct_method_resource_refs.len() > 4
            || self.tdd_child_resource_refs.len() > 5
        {
            return Err(CapabilityLiteralError::InvalidCapabilityMethodResolution);
        }
        let mut methods = Vec::new();
        for resource in &self.direct_method_resource_refs {
            resource.validate()?;
            if !job_method_is_admitted(job, resource.method) {
                return Err(CapabilityLiteralError::InvalidCapabilityMethodResolution);
            }
            methods.push(resource.method);
        }
        if !is_strictly_ordered_unique(&methods)
            || (matches!(
                job,
                InternalJobV1::Design | InternalJobV1::Execute | InternalJobV1::Adapt
            ) && methods.len() > 1)
            || (!self.tdd_child_resource_refs.is_empty()
                && (job != InternalJobV1::Execute || !methods.contains(&DirectMethodV1::Tdd)))
            || (self.research_example_resource_ref.is_some() && job != InternalJobV1::Research)
        {
            return Err(CapabilityLiteralError::InvalidCapabilityMethodResolution);
        }
        let mut children = Vec::new();
        for resource in &self.tdd_child_resource_refs {
            resource.validate()?;
            let child = TddChildV1::ALL
                .into_iter()
                .find(|child| resource.logical_path == child.resource_path())
                .ok_or(CapabilityLiteralError::InvalidCapabilityMethodResolution)?;
            children.push(child);
        }
        if !is_strictly_ordered_unique(&children) {
            return Err(CapabilityLiteralError::InvalidCapabilityMethodResolution);
        }
        if let Some(resource) = &self.research_example_resource_ref {
            resource.validate()?;
            if resource.logical_path != INSTRUCTION_RESOURCE_PATHS_V1[30] {
                return Err(CapabilityLiteralError::InvalidCapabilityMethodResolution);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityMethodAmbiguousReasonV1 {
    MultipleDirectMethods,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityMethodBlockedReasonV1 {
    MethodNotEligible,
    TypedNeedMismatch,
    ReviewResolutionBlocked,
    TddChildNotEligible,
    ResearchExamplesNotEligible,
    ResourceClosureUnavailable,
    ContextBudgetExceeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityMethodResolutionOutcomeV1 {
    Selected(CapabilityInstructionLoadPlanV1),
    Ambiguous(CapabilityMethodAmbiguousReasonV1),
    Blocked(CapabilityMethodBlockedReasonV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityMethodResolutionV1 {
    pub schema_version: u64,
    pub resolution_basis_ref: String,
    pub exact_selected_job_route_ref: String,
    pub exact_intent_ref: String,
    pub outcome: CapabilityMethodResolutionOutcomeV1,
}

impl CapabilityMethodResolutionV1 {
    pub fn validate_for(
        &self,
        selected_job: InternalJobV1,
        exact_selected_job_route_ref: &str,
    ) -> Result<(), CapabilityLiteralError> {
        if self.schema_version != 1
            || !is_nonempty_current_ref(&self.resolution_basis_ref)
            || self.exact_selected_job_route_ref != exact_selected_job_route_ref
            || !is_nonempty_current_ref(&self.exact_intent_ref)
        {
            return Err(CapabilityLiteralError::InvalidCapabilityMethodResolution);
        }
        if let CapabilityMethodResolutionOutcomeV1::Selected(plan) = &self.outcome {
            plan.validate_for(selected_job)?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextBudgetMeasurementV1 {
    pub closure_ref: String,
    pub ordered_resource_refs: Vec<String>,
    pub utf8_bytes: u64,
    pub host_observed_units: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContextBudgetProfileV1 {
    pub schema_version: u64,
    pub profile_id: String,
    pub release_ref: String,
    pub host_ref: String,
    pub renderer_or_meter_ref: String,
    pub measurement_procedure_ref: String,
    pub admitted_resource_refs: Vec<String>,
    pub maximum_utf8_bytes: u64,
    pub maximum_host_observed_units: u64,
    pub measurements: Vec<ContextBudgetMeasurementV1>,
}

impl ContextBudgetProfileV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if self.schema_version != 1
            || self.profile_id.is_empty()
            || !is_nonempty_current_ref(&self.release_ref)
            || self.host_ref.is_empty()
            || self.renderer_or_meter_ref.is_empty()
            || self.measurement_procedure_ref.is_empty()
            || self.admitted_resource_refs.is_empty()
            || !all_unique(&self.admitted_resource_refs)
            || self.maximum_utf8_bytes == 0
            || self.maximum_host_observed_units == 0
            || self.measurements.is_empty()
        {
            return Err(CapabilityLiteralError::InvalidContextBudgetProfile);
        }
        let mut closures = BTreeSet::new();
        for measurement in &self.measurements {
            if measurement.closure_ref.is_empty()
                || !closures.insert(&measurement.closure_ref)
                || measurement.ordered_resource_refs.is_empty()
                || !all_unique(&measurement.ordered_resource_refs)
                || measurement.utf8_bytes == 0
                || measurement.host_observed_units == 0
                || measurement.utf8_bytes > self.maximum_utf8_bytes
                || measurement.host_observed_units > self.maximum_host_observed_units
                || measurement
                    .ordered_resource_refs
                    .iter()
                    .any(|resource| !self.admitted_resource_refs.contains(resource))
            {
                return Err(CapabilityLiteralError::InvalidContextBudgetProfile);
            }
        }
        Ok(())
    }

    pub fn admits_plan(
        &self,
        ordered_resource_refs: &[String],
    ) -> Result<(), CapabilityLiteralError> {
        self.validate()?;
        if self
            .measurements
            .iter()
            .any(|measurement| measurement.ordered_resource_refs == ordered_resource_refs)
        {
            Ok(())
        } else {
            Err(CapabilityLiteralError::InvalidContextBudgetProfile)
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SelectedJobRecipeAdmissionOutcomeV1 {
    NoRecipe,
    Admitted(Vec<String>),
    Refused(JobRecipeAdmissionRefusalV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum JobRecipeAdmissionRefusalV1 {
    NonEdge,
    InvalidApplication,
    StaleOrMixedRelease,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectedJobRecipeAdmissionV1 {
    pub resolution_basis_ref: String,
    pub exact_packet_application_ref: String,
    pub exact_selected_job_route_ref: String,
    pub outcome: SelectedJobRecipeAdmissionOutcomeV1,
}

pub const JOB_RECIPE_ROWS_V1: [(RecipeIdV1, &[InternalJobV1]); 10] = [
    (RecipeIdV1::BoundedContinuation, &InternalJobV1::ALL),
    (
        RecipeIdV1::ConflictHandoff,
        &[InternalJobV1::Execute, InternalJobV1::Recover],
    ),
    (RecipeIdV1::DesignRelay, &[InternalJobV1::Design]),
    (RecipeIdV1::Fanout, &[InternalJobV1::Execute]),
    (
        RecipeIdV1::IntakeTriage,
        &[InternalJobV1::Research, InternalJobV1::Design],
    ),
    (
        RecipeIdV1::Learning,
        &[
            InternalJobV1::Review,
            InternalJobV1::Recover,
            InternalJobV1::Adapt,
        ],
    ),
    (RecipeIdV1::Setup, &[InternalJobV1::Setup]),
    (RecipeIdV1::Ship, &[InternalJobV1::Execute]),
    (
        RecipeIdV1::Synthesize,
        &[InternalJobV1::Execute, InternalJobV1::Recover],
    ),
    (
        RecipeIdV1::Wayfinding,
        &[InternalJobV1::Research, InternalJobV1::Design],
    ),
];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobRecipeEligibilityRowV1 {
    pub exact_recipe_resource_ref: String,
    pub eligible_jobs: Vec<InternalJobV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct JobRecipeEligibilityV1 {
    pub schema_version: u64,
    pub orchestration_recipe_catalog_ref: String,
    pub rows: Vec<JobRecipeEligibilityRowV1>,
}

impl JobRecipeEligibilityV1 {
    pub fn validate(&self) -> Result<(), CapabilityLiteralError> {
        if self.schema_version != 1
            || !is_nonempty_current_ref(&self.orchestration_recipe_catalog_ref)
            || self.rows.len() != JOB_RECIPE_ROWS_V1.len()
        {
            return Err(CapabilityLiteralError::InvalidJobRecipeEligibility);
        }
        for (row, (recipe, jobs)) in self.rows.iter().zip(JOB_RECIPE_ROWS_V1) {
            if RecipeIdV1::from_resource_ref(&row.exact_recipe_resource_ref) != Some(recipe)
                || row.eligible_jobs != jobs
                || row.eligible_jobs.is_empty()
                || !is_strictly_ordered_unique(&row.eligible_jobs)
            {
                return Err(CapabilityLiteralError::InvalidJobRecipeEligibility);
            }
        }
        Ok(())
    }

    pub fn admits(&self, job: InternalJobV1, recipe: RecipeIdV1) -> bool {
        self.rows
            .get(recipe as usize - 1)
            .is_some_and(|row| row.eligible_jobs.contains(&job))
    }
}

pub fn admit_selected_job_recipe(
    eligibility: &JobRecipeEligibilityV1,
    job: InternalJobV1,
    application: &RecipeApplicationV1,
    resolution_basis_ref: String,
    exact_packet_application_ref: String,
    exact_selected_job_route_ref: String,
) -> Result<SelectedJobRecipeAdmissionV1, CapabilityLiteralError> {
    eligibility.validate()?;
    if application.validate().is_err()
        || resolution_basis_ref.is_empty()
        || exact_packet_application_ref.is_empty()
        || exact_selected_job_route_ref.is_empty()
    {
        return Err(CapabilityLiteralError::InvalidJobRecipeAdmission);
    }
    let mut resources = Vec::new();
    let mut recipes = Vec::new();
    match &application.primary {
        ExactRecipeSelectionV1::Absent => {}
        ExactRecipeSelectionV1::Primary {
            recipe_resource_ref,
            ..
        } => {
            let recipe = RecipeIdV1::from_resource_ref(recipe_resource_ref)
                .ok_or(CapabilityLiteralError::InvalidJobRecipeAdmission)?;
            resources.push(recipe_resource_ref.clone());
            recipes.push(recipe);
        }
        ExactRecipeSelectionV1::Continuation { .. } => {
            return Err(CapabilityLiteralError::InvalidJobRecipeAdmission);
        }
    }
    match &application.continuation {
        ExactRecipeSelectionV1::Absent => {}
        ExactRecipeSelectionV1::Continuation {
            recipe_resource_ref,
            profile_resource_ref,
            ..
        } => {
            if BoundedContinuationProfileV1::from_resource_ref(profile_resource_ref).is_none() {
                return Err(CapabilityLiteralError::InvalidJobRecipeAdmission);
            }
            let recipe = RecipeIdV1::from_resource_ref(recipe_resource_ref)
                .ok_or(CapabilityLiteralError::InvalidJobRecipeAdmission)?;
            resources.push(recipe_resource_ref.clone());
            recipes.push(recipe);
        }
        ExactRecipeSelectionV1::Primary { .. } => {
            return Err(CapabilityLiteralError::InvalidJobRecipeAdmission);
        }
    }
    let outcome = if resources.is_empty() {
        SelectedJobRecipeAdmissionOutcomeV1::NoRecipe
    } else if recipes
        .into_iter()
        .all(|recipe| eligibility.admits(job, recipe))
    {
        SelectedJobRecipeAdmissionOutcomeV1::Admitted(resources)
    } else {
        SelectedJobRecipeAdmissionOutcomeV1::Refused(JobRecipeAdmissionRefusalV1::NonEdge)
    };
    Ok(SelectedJobRecipeAdmissionV1 {
        resolution_basis_ref,
        exact_packet_application_ref,
        exact_selected_job_route_ref,
        outcome,
    })
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewUnexaminedTargetV1 {
    pub target_ref: String,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewCoverageV1 {
    pub exact_target_universe_ref: String,
    pub examined_refs: Vec<String>,
    pub unexamined: Vec<ReviewUnexaminedTargetV1>,
    pub unknowns: Vec<String>,
}

impl ReviewCoverageV1 {
    pub fn validate_against(
        &self,
        exact_target_universe: &[String],
    ) -> Result<(), CapabilityLiteralError> {
        if self.exact_target_universe_ref.is_empty()
            || exact_target_universe.is_empty()
            || !all_unique(exact_target_universe)
            || !all_unique(&self.examined_refs)
            || !all_unique(&self.unknowns)
            || self
                .unexamined
                .iter()
                .any(|row| row.target_ref.is_empty() || row.reason.is_empty())
        {
            return Err(CapabilityLiteralError::InvalidReviewCoverage);
        }
        let examined = self.examined_refs.iter().cloned().collect::<BTreeSet<_>>();
        let unexamined = self
            .unexamined
            .iter()
            .map(|row| row.target_ref.clone())
            .collect::<BTreeSet<_>>();
        let expected = exact_target_universe
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if examined.len() != self.examined_refs.len()
            || unexamined.len() != self.unexamined.len()
            || !examined.is_disjoint(&unexamined)
            || examined
                .union(&unexamined)
                .cloned()
                .collect::<BTreeSet<_>>()
                != expected
        {
            return Err(CapabilityLiteralError::InvalidReviewCoverage);
        }
        Ok(())
    }

    pub fn supports_no_finding(&self, exact_target_universe: &[String]) -> bool {
        self.validate_against(exact_target_universe).is_ok()
            && self.unexamined.is_empty()
            && self.unknowns.is_empty()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewResultHeaderV1 {
    pub release_ref: String,
    pub review_job_resource_ref: InstructionResourceRefV1,
    pub method_load_plan: ReviewMethodLoadPlanV1,
    pub exact_reviewed_input_refs: Vec<String>,
    pub reviewed_input_hash: [u8; 32],
    pub applicable_contract_root_ref: String,
    pub work_generation_ref: String,
    pub step_revision_ref: Option<String>,
    pub artifact_refs: Vec<String>,
    pub run_refs: Vec<String>,
    pub tree_ref: String,
    pub environment_ref: String,
    pub reviewer_principal_ref: String,
    pub reviewer_session_ref: String,
    pub model_tool_procedure_ref: String,
    pub provenance_lineage_refs: Vec<String>,
    pub as_of_ref: String,
    pub coverage: ReviewCoverageV1,
    pub invalidation_conditions: Vec<String>,
}

impl ReviewResultHeaderV1 {
    pub fn validate_for(
        &self,
        mode: ReviewModeV1,
        exact_target_universe: &[String],
    ) -> Result<(), CapabilityLiteralError> {
        self.review_job_resource_ref.validate()?;
        self.method_load_plan.validate_for(mode)?;
        self.coverage.validate_against(exact_target_universe)?;
        if !is_nonempty_current_ref(&self.release_ref)
            || self.review_job_resource_ref.logical_path != InternalJobV1::Review.resource_path()
            || self.exact_reviewed_input_refs.is_empty()
            || !all_unique(&self.exact_reviewed_input_refs)
            || self.reviewed_input_hash == [0; 32]
            || [
                &self.applicable_contract_root_ref,
                &self.work_generation_ref,
                &self.tree_ref,
                &self.environment_ref,
                &self.reviewer_principal_ref,
                &self.reviewer_session_ref,
                &self.model_tool_procedure_ref,
                &self.as_of_ref,
            ]
            .into_iter()
            .any(|value| value.is_empty())
            || self.provenance_lineage_refs.is_empty()
            || self.invalidation_conditions.is_empty()
        {
            return Err(CapabilityLiteralError::InvalidReviewResult);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InspectResultRowV1 {
    Finding {
        target_ref: String,
        finding_ref: String,
    },
    NoFinding {
        claim_ref: String,
    },
    Unknown {
        target_ref: String,
        reason: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectResultV1 {
    pub bounded_inspected_refs: Vec<String>,
    pub rows: Vec<InspectResultRowV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditObligationDispositionV1 {
    Examined,
    Unexamined,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditObligationResultV1 {
    pub obligation_ref: String,
    pub disposition: AuditObligationDispositionV1,
    pub detail_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditResultV1 {
    pub obligations: Vec<AuditObligationResultV1>,
    pub candidate_finding_refs: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AdversarialClaimDispositionV1 {
    NotRefuted,
    Refuted,
    Indeterminate,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdversarialClaimResultV1 {
    pub claim_ref: String,
    pub disposition: AdversarialClaimDispositionV1,
    pub detail_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AdversarialReviewResultV1 {
    pub claims: Vec<AdversarialClaimResultV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QaReplayScenarioDispositionV1 {
    ObservedAsSpecified,
    ObservedDrift,
    Indeterminate,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QaReplayScenarioResultV1 {
    pub scenario_spec_ref: String,
    pub artifact_ref: String,
    pub environment_ref: String,
    pub disposition: QaReplayScenarioDispositionV1,
    pub detail_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct QaReplayResultV1 {
    pub scenarios: Vec<QaReplayScenarioResultV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CloseReviewRequirementDispositionV1 {
    SupportedInput,
    Defect,
    Indeterminate,
    Error,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseReviewRequirementResultV1 {
    pub requirement_ref: String,
    pub disposition: CloseReviewRequirementDispositionV1,
    pub detail_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseReviewResultV1 {
    pub requirements: Vec<CloseReviewRequirementResultV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewMethodResultV1 {
    Inspect(ReviewResultHeaderV1, InspectResultV1),
    Audit(ReviewResultHeaderV1, AuditResultV1),
    AdversarialReview(ReviewResultHeaderV1, AdversarialReviewResultV1),
    QaReplay(ReviewResultHeaderV1, QaReplayResultV1),
    CloseReview(ReviewResultHeaderV1, CloseReviewResultV1),
}

impl ReviewMethodResultV1 {
    pub fn validate(&self, exact_target_universe: &[String]) -> Result<(), CapabilityLiteralError> {
        let (mode, header) = match self {
            Self::Inspect(header, payload) => {
                if payload.bounded_inspected_refs.is_empty()
                    || payload.rows.iter().any(|row| match row {
                        InspectResultRowV1::Finding {
                            target_ref,
                            finding_ref,
                        } => target_ref.is_empty() || finding_ref.is_empty(),
                        InspectResultRowV1::NoFinding { claim_ref } => {
                            claim_ref.is_empty()
                                || !header.coverage.supports_no_finding(exact_target_universe)
                        }
                        InspectResultRowV1::Unknown { target_ref, reason } => {
                            target_ref.is_empty() || reason.is_empty()
                        }
                    })
                {
                    return Err(CapabilityLiteralError::InvalidReviewResult);
                }
                (ReviewModeV1::Inspect, header)
            }
            Self::Audit(header, payload) => {
                if payload.obligations.is_empty()
                    || payload
                        .obligations
                        .iter()
                        .any(|row| row.obligation_ref.is_empty() || row.detail_ref.is_empty())
                {
                    return Err(CapabilityLiteralError::InvalidReviewResult);
                }
                (ReviewModeV1::Audit, header)
            }
            Self::AdversarialReview(header, payload) => {
                if payload.claims.is_empty()
                    || payload
                        .claims
                        .iter()
                        .any(|row| row.claim_ref.is_empty() || row.detail_ref.is_empty())
                {
                    return Err(CapabilityLiteralError::InvalidReviewResult);
                }
                (ReviewModeV1::AdversarialReview, header)
            }
            Self::QaReplay(header, payload) => {
                if payload.scenarios.is_empty()
                    || payload.scenarios.iter().any(|row| {
                        row.scenario_spec_ref.is_empty()
                            || row.artifact_ref.is_empty()
                            || row.environment_ref.is_empty()
                            || row.detail_ref.is_empty()
                    })
                {
                    return Err(CapabilityLiteralError::InvalidReviewResult);
                }
                (ReviewModeV1::QaReplay, header)
            }
            Self::CloseReview(header, payload) => {
                if payload.requirements.is_empty()
                    || payload
                        .requirements
                        .iter()
                        .any(|row| row.requirement_ref.is_empty() || row.detail_ref.is_empty())
                {
                    return Err(CapabilityLiteralError::InvalidReviewResult);
                }
                (ReviewModeV1::CloseReview, header)
            }
        };
        header.validate_for(mode, exact_target_universe)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewMethodRefusalV1 {
    pub mode: ReviewModeV1,
    pub exact_input_ref: String,
    pub refusal_reason_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewMethodFailureV1 {
    pub mode: ReviewModeV1,
    pub exact_input_ref: String,
    pub failure_reason_ref: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ReviewMethodInvocationV1 {
    Produced(Box<ReviewMethodResultV1>),
    Refused(ReviewMethodRefusalV1),
    Failed(ReviewMethodFailureV1),
}

pub type ReviewInvocationOutcomeV1 = ReviewMethodInvocationV1;

impl ReviewMethodInvocationV1 {
    pub fn validate(&self, exact_target_universe: &[String]) -> Result<(), CapabilityLiteralError> {
        match self {
            Self::Produced(result) => result.validate(exact_target_universe),
            Self::Refused(refusal) => {
                if refusal.exact_input_ref.is_empty() || refusal.refusal_reason_ref.is_empty() {
                    Err(CapabilityLiteralError::InvalidReviewInvocation)
                } else {
                    Ok(())
                }
            }
            Self::Failed(failure) => {
                if failure.exact_input_ref.is_empty() || failure.failure_reason_ref.is_empty() {
                    Err(CapabilityLiteralError::InvalidReviewInvocation)
                } else {
                    Ok(())
                }
            }
        }
    }
}

pub fn exact_review_subset_counts() -> (usize, usize) {
    let auxiliaries = [
        DirectMethodV1::ArchitectureReview,
        DirectMethodV1::GenerateAndFilter,
        DirectMethodV1::Verification,
    ];
    let admitted = ReviewModeV1::ALL
        .into_iter()
        .map(|mode| {
            (0_u8..8)
                .filter(|mask| {
                    auxiliaries.iter().enumerate().all(|(index, method)| {
                        mask & (1 << index) == 0 || mode.admitted_auxiliaries().contains(method)
                    })
                })
                .count()
        })
        .sum::<usize>();
    (admitted, 40 - admitted)
}

fn is_nonempty_current_ref(value: &str) -> bool {
    !value.is_empty() && !value.contains("latest") && !value.contains("fallback")
}

fn resource_ref_matches_path(resource_ref: &str, path: &str) -> bool {
    let prefix = format!("candidate:capability:instruction-resource:{path}:v1@sha256:");
    resource_ref
        .strip_prefix(&prefix)
        .is_some_and(is_nonzero_lower_hex_digest)
}

fn is_nonzero_lower_hex_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
}

fn all_unique(values: &[String]) -> bool {
    values
        .iter()
        .enumerate()
        .all(|(index, value)| !values[..index].contains(value))
}

fn is_strictly_ordered_unique<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

pub fn exact_job_method_degrees() -> BTreeMap<DirectMethodV1, usize> {
    let mut degrees = BTreeMap::new();
    for (_, methods) in JOB_METHOD_ROWS_V1 {
        for method in methods {
            *degrees.entry(*method).or_insert(0) += 1;
        }
    }
    degrees
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum CapabilityLiteralError {
    #[error(
        "InstructionResourceV1 must name one exact content-qualified member of the 31-path tree"
    )]
    InvalidInstructionResource,
    #[error("JobMethodEligibilityV1 must reproduce all 19 admitted and 100 refused cells")]
    InvalidJobMethodEligibility,
    #[error("Capability typed need is incomplete, duplicated, or outside the closed need catalog")]
    InvalidTypedNeed,
    #[error(
        "CapabilityMethodIntentV1 cannot contain a caller load plan, fallback, private map, or invalid request"
    )]
    InvalidCapabilityMethodIntent,
    #[error(
        "CapabilityMethodResolutionV1 must preserve one selected job and its exact eligible closure"
    )]
    InvalidCapabilityMethodResolution,
    #[error("TddChildEligibilityV1 must contain five Execute/TDD-only typed child edges")]
    InvalidTddChildEligibility,
    #[error("ResearchExampleEligibilityV1 must contain only the default-unloaded Research edge")]
    InvalidResearchExampleEligibility,
    #[error("ReviewModeResolutionV1 is incomplete or not facts-first")]
    InvalidReviewResolution,
    #[error("ReviewMethodLoadPlanV1 violates the fixed primary or four/eleven auxiliary relation")]
    InvalidReviewLoadPlan,
    #[error("ReviewCoverageV1 is not a total disjoint disposition of the exact target universe")]
    InvalidReviewCoverage,
    #[error("ReviewMethodResultV1 is not one exact mode payload with complete provenance")]
    InvalidReviewResult,
    #[error(
        "ReviewMethodInvocationV1 must distinguish Produced, semantic Refused, and conclusive Failed"
    )]
    InvalidReviewInvocation,
    #[error(
        "ContextBudgetProfileV1 must bind one host/Release and measure every admitted closure without fallback"
    )]
    InvalidContextBudgetProfile,
    #[error("JobRecipeEligibilityV1 must reproduce ten rows, 22 edges, and 48 non-edges")]
    InvalidJobRecipeEligibility,
    #[error("SelectedJobRecipeAdmissionV1 must be an exact all-or-nothing post-route admission")]
    InvalidJobRecipeAdmission,
    #[error("canonical Agent instruction Resources or the legacy Skill ledger are incomplete")]
    InvalidAgentResourceInventory,
}

#[cfg(test)]
mod successor_cutover_tests {
    use super::*;

    #[test]
    fn canonical_agent_resource_inventory_is_exact_and_separate_from_legacy_ledger() {
        let inventory = CanonicalAgentResourceInventoryV1::load_embedded().unwrap();
        assert_eq!(inventory.resources().len(), 31);
        assert_eq!(inventory.legacy_ledger().len(), 35);
        assert_ne!(
            inventory.resource_closure(),
            inventory.legacy_ledger_closure()
        );
        assert_eq!(
            inventory
                .legacy_ledger()
                .iter()
                .filter(|row| row.disposition == LegacySkillDispositionV1::Rewrite)
                .count(),
            19
        );
        assert_eq!(
            inventory
                .legacy_ledger()
                .iter()
                .filter(|row| row.disposition == LegacySkillDispositionV1::Replace)
                .count(),
            9
        );
        assert_eq!(
            inventory
                .legacy_ledger()
                .iter()
                .filter(|row| row.disposition == LegacySkillDispositionV1::MigrationOnly)
                .count(),
            7
        );
        assert!(inventory.legacy_ledger().iter().all(|row| {
            row.disposition != LegacySkillDispositionV1::MigrationOnly
                || row.active_destination.is_none()
        }));
    }
}
