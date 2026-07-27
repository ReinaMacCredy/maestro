use std::collections::{BTreeMap, BTreeSet, VecDeque};

use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::foundation::core::deterministic_cbor::{self, CborError, CborValue};

use super::super::closed::AuthorityContextKindV1;
use super::super::identity::AuthorityContinuityManifestIdV1;
use super::catalog::{
    ContinuityClassIdV1, ContinuityReferenceError, ContinuityReferenceV1,
    ContinuitySemanticOwnerV1, CoverageObligationIdV1, InstallationAuthorityContinuityClassV1,
    OwnerContributionIdV1, RepositoryAuthorityContinuityClassV1, canonical_owner,
    required_class_ids,
};

const MAX_OBLIGATIONS: usize = 256;
const MAX_CLASSES: usize = 128;
const MAX_CONTRIBUTIONS: usize = 64;
const MAX_REFERENCES_PER_ROW: usize = 128;
const SUPPORTED_PROTOCOL_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ClassDispositionV1 {
    CanonicalRecordClosure = 1,
    DerivedOnly = 2,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityCoverageObligationV1 {
    pub id: CoverageObligationIdV1,
    pub context_kind: AuthorityContextKindV1,
    pub owner: ContinuitySemanticOwnerV1,
    pub source_protocol: ContinuityReferenceV1,
    pub subject_projection: ContinuityReferenceV1,
    pub protocol_version: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CoverageDispositionKindV1 {
    IncludedBy {
        class_ids: Vec<ContinuityClassIdV1>,
    },
    ExplicitlyNonContinuity {
        owner_invariant: ContinuityReferenceV1,
        proof: ContinuityReferenceV1,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityCoverageDispositionV1 {
    pub obligation_id: CoverageObligationIdV1,
    pub owner: ContinuitySemanticOwnerV1,
    pub kind: CoverageDispositionKindV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityOwnerContributionV1 {
    pub id: OwnerContributionIdV1,
    pub context_kind: AuthorityContextKindV1,
    pub owner: ContinuitySemanticOwnerV1,
    pub owner_protocol: ContinuityReferenceV1,
    pub obligation_ids: Vec<CoverageObligationIdV1>,
    pub class_ids: Vec<ContinuityClassIdV1>,
    pub depends_on: Vec<OwnerContributionIdV1>,
    pub protocol_version: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityClassDescriptorV1 {
    pub class_id: ContinuityClassIdV1,
    pub context_kind: AuthorityContextKindV1,
    pub owner: ContinuitySemanticOwnerV1,
    pub owner_contribution_id: OwnerContributionIdV1,
    pub owner_protocol: ContinuityReferenceV1,
    pub subject_projection: ContinuityReferenceV1,
    pub closure_schema: ContinuityReferenceV1,
    pub disposition: ClassDispositionV1,
    pub depends_on: Vec<ContinuityClassIdV1>,
    pub protocol_version: u16,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityTotalityInputV1 {
    pub context_kind: AuthorityContextKindV1,
    pub protocol_version: u16,
    pub canonicalization_version: u16,
    pub obligations: Vec<AuthorityContinuityCoverageObligationV1>,
    pub dispositions: Vec<AuthorityContinuityCoverageDispositionV1>,
    pub owner_contributions: Vec<AuthorityContinuityOwnerContributionV1>,
    pub closed_class_sum: Vec<ContinuityClassIdV1>,
    pub descriptors: Vec<AuthorityContinuityClassDescriptorV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuthorityContinuityManifestV1 {
    id: AuthorityContinuityManifestIdV1,
    context_kind: AuthorityContextKindV1,
    protocol_version: u16,
    canonicalization_version: u16,
    obligations: Vec<AuthorityContinuityCoverageObligationV1>,
    dispositions: Vec<AuthorityContinuityCoverageDispositionV1>,
    owner_contributions: Vec<AuthorityContinuityOwnerContributionV1>,
    class_ids: Vec<ContinuityClassIdV1>,
    descriptors: Vec<AuthorityContinuityClassDescriptorV1>,
}

impl AuthorityContinuityManifestV1 {
    pub const SCHEMA_DOMAIN: &'static str =
        "maestro.vnext.authority-continuity-totality-manifest.v1";

    pub fn repository() -> Result<Self, AuthorityContinuityError> {
        Self::prove(repository_authority_continuity_totality_input()?)
    }

    pub fn installation() -> Result<Self, AuthorityContinuityError> {
        Self::prove(installation_authority_continuity_totality_input()?)
    }

    pub fn prove(
        mut input: AuthorityContinuityTotalityInputV1,
    ) -> Result<Self, AuthorityContinuityError> {
        validate_totality(&input)?;
        normalize_totality(&mut input);
        let mut manifest = Self {
            id: AuthorityContinuityManifestIdV1::from_digest([0; 32]),
            context_kind: input.context_kind,
            protocol_version: input.protocol_version,
            canonicalization_version: input.canonicalization_version,
            obligations: input.obligations,
            dispositions: input.dispositions,
            owner_contributions: input.owner_contributions,
            class_ids: input.closed_class_sum,
            descriptors: input.descriptors,
        };
        manifest.id =
            AuthorityContinuityManifestIdV1::from_digest(hash(&manifest.schema_value()?)?);
        Ok(manifest)
    }

    pub const fn id(&self) -> AuthorityContinuityManifestIdV1 {
        self.id
    }

    pub const fn context_kind(&self) -> AuthorityContextKindV1 {
        self.context_kind
    }

    pub fn class_count(&self) -> usize {
        self.class_ids.len()
    }

    pub fn obligation_count(&self) -> usize {
        self.obligations.len()
    }

    pub fn class_ids(&self) -> &[ContinuityClassIdV1] {
        &self.class_ids
    }

    pub fn descriptors(&self) -> &[AuthorityContinuityClassDescriptorV1] {
        &self.descriptors
    }

    pub fn descriptor(
        &self,
        class_id: ContinuityClassIdV1,
    ) -> Option<&AuthorityContinuityClassDescriptorV1> {
        self.descriptors
            .binary_search_by_key(&class_id, |descriptor| descriptor.class_id)
            .ok()
            .map(|index| &self.descriptors[index])
    }

    pub fn schema_value(&self) -> Result<CborValue, CborError> {
        Ok(CborValue::Array(vec![
            CborValue::text(Self::SCHEMA_DOMAIN)?,
            CborValue::Unsigned(self.context_kind as u64),
            CborValue::Unsigned(u64::from(self.protocol_version)),
            CborValue::Unsigned(u64::from(self.canonicalization_version)),
            CborValue::Array(self.obligations.iter().map(obligation_value).collect()),
            CborValue::Array(self.dispositions.iter().map(disposition_value).collect()),
            CborValue::Array(
                self.owner_contributions
                    .iter()
                    .map(contribution_value)
                    .collect(),
            ),
            CborValue::Array(
                self.class_ids
                    .iter()
                    .map(|class_id| class_id.schema_value())
                    .collect(),
            ),
            CborValue::Array(self.descriptors.iter().map(descriptor_value).collect()),
        ]))
    }

    pub fn canonical_bytes(&self) -> Result<Vec<u8>, CborError> {
        deterministic_cbor::encode(&self.schema_value()?)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum AuthorityContinuityError {
    #[error(transparent)]
    CanonicalCbor(#[from] CborError),
    #[error(transparent)]
    Reference(#[from] ContinuityReferenceError),
    #[error("continuity protocol or canonicalization version is unsupported")]
    UnsupportedVersion,
    #[error("continuity totality input is empty or exceeds a finite bound")]
    BoundViolation,
    #[error("continuity material has the wrong Authority context")]
    WrongContext,
    #[error("continuity material contains a duplicate obligation")]
    DuplicateObligation,
    #[error(
        "submitted owner-source obligations or dispositions differ from the frozen independent census"
    )]
    FrozenOwnerCensusMismatch,
    #[error("continuity obligations and exactly-one dispositions differ")]
    ObligationDispositionMismatch,
    #[error("IncludedBy must name a nonempty finite exact class set without duplicates")]
    InvalidIncludedClassSet,
    #[error("continuity material contains a duplicate class")]
    DuplicateClass,
    #[error("the closed class sum differs from the frozen context class universe")]
    FrozenClassSumMismatch,
    #[error("IncludedBy references differ from the closed typed class sum")]
    IncludedReferenceMismatch,
    #[error("continuity material contains a duplicate owner contribution")]
    DuplicateOwnerContribution,
    #[error("an obligation or class does not have exactly one owner contribution")]
    ContributionCoverageMismatch,
    #[error("continuity material names an orphan contribution or dependency")]
    OrphanReference,
    #[error("continuity owner or context does not match the source-record owner")]
    WrongOwner,
    #[error("the concrete descriptor set differs from the closed typed class sum")]
    DescriptorSetMismatch,
    #[error("continuity descriptor or owner-contribution references form a cycle")]
    CyclicReference,
}

pub fn repository_authority_continuity_totality_input()
-> Result<AuthorityContinuityTotalityInputV1, AuthorityContinuityError> {
    builtin_totality_input(AuthorityContextKindV1::RepositoryAuthorityContext)
}

pub fn installation_authority_continuity_totality_input()
-> Result<AuthorityContinuityTotalityInputV1, AuthorityContinuityError> {
    builtin_totality_input(AuthorityContextKindV1::InstallationAuthorityContext)
}

fn validate_totality(
    input: &AuthorityContinuityTotalityInputV1,
) -> Result<(), AuthorityContinuityError> {
    if input.protocol_version != SUPPORTED_PROTOCOL_VERSION
        || input.canonicalization_version != SUPPORTED_PROTOCOL_VERSION
        || input.obligations.is_empty()
        || input.obligations.len() > MAX_OBLIGATIONS
        || input.dispositions.len() > MAX_OBLIGATIONS
        || input.owner_contributions.is_empty()
        || input.owner_contributions.len() > MAX_CONTRIBUTIONS
        || input.closed_class_sum.is_empty()
        || input.closed_class_sum.len() > MAX_CLASSES
        || input.descriptors.len() > MAX_CLASSES
    {
        return if input.protocol_version != SUPPORTED_PROTOCOL_VERSION
            || input.canonicalization_version != SUPPORTED_PROTOCOL_VERSION
        {
            Err(AuthorityContinuityError::UnsupportedVersion)
        } else {
            Err(AuthorityContinuityError::BoundViolation)
        };
    }

    let obligations = unique_map(
        &input.obligations,
        |obligation| obligation.id,
        AuthorityContinuityError::DuplicateObligation,
    )?;
    if input.obligations.iter().any(|obligation| {
        obligation.context_kind != input.context_kind
            || obligation.protocol_version != SUPPORTED_PROTOCOL_VERSION
    }) {
        return Err(AuthorityContinuityError::WrongContext);
    }
    let (expected_obligations, expected_dispositions) =
        materialize_frozen_owner_census(input.context_kind)?;
    if !exact_rows_by_key(&input.obligations, &expected_obligations, |obligation| {
        obligation.id
    }) {
        return Err(AuthorityContinuityError::FrozenOwnerCensusMismatch);
    }

    let dispositions = unique_map(
        &input.dispositions,
        |disposition| disposition.obligation_id,
        AuthorityContinuityError::ObligationDispositionMismatch,
    )?;
    if obligations.keys().copied().collect::<BTreeSet<_>>()
        != dispositions.keys().copied().collect::<BTreeSet<_>>()
    {
        return Err(AuthorityContinuityError::ObligationDispositionMismatch);
    }
    let class_sum = unique_set(
        &input.closed_class_sum,
        AuthorityContinuityError::DuplicateClass,
    )?;
    if class_sum
        .iter()
        .any(|class_id| class_id.context_kind() != input.context_kind)
    {
        return Err(AuthorityContinuityError::WrongContext);
    }
    if class_sum
        != required_class_ids(input.context_kind)
            .into_iter()
            .collect::<BTreeSet<_>>()
    {
        return Err(AuthorityContinuityError::FrozenClassSumMismatch);
    }

    let mut included_references = BTreeSet::new();
    for disposition in &input.dispositions {
        let obligation = obligations
            .get(&disposition.obligation_id)
            .expect("invariant: disposition equality was established");
        if obligation.owner != disposition.owner {
            return Err(AuthorityContinuityError::WrongOwner);
        }
        if let CoverageDispositionKindV1::IncludedBy { class_ids } = &disposition.kind {
            if class_ids.is_empty()
                || class_ids.len() > MAX_REFERENCES_PER_ROW
                || unique_set(class_ids, AuthorityContinuityError::InvalidIncludedClassSet)?.len()
                    != class_ids.len()
            {
                return Err(AuthorityContinuityError::InvalidIncludedClassSet);
            }
            for class_id in class_ids {
                if class_id.context_kind() != input.context_kind {
                    return Err(AuthorityContinuityError::WrongContext);
                }
                if canonical_owner(*class_id) != obligation.owner {
                    return Err(AuthorityContinuityError::WrongOwner);
                }
                included_references.insert(*class_id);
            }
        }
    }
    if included_references != class_sum {
        return Err(AuthorityContinuityError::IncludedReferenceMismatch);
    }
    if !exact_rows_by_key(&input.dispositions, &expected_dispositions, |disposition| {
        disposition.obligation_id
    }) {
        return Err(AuthorityContinuityError::FrozenOwnerCensusMismatch);
    }

    let contributions = unique_map(
        &input.owner_contributions,
        |contribution| contribution.id,
        AuthorityContinuityError::DuplicateOwnerContribution,
    )?;
    let contribution_ids = contributions.keys().copied().collect::<BTreeSet<_>>();
    let mut contributed_obligations = BTreeSet::new();
    let mut contributed_classes = BTreeSet::new();
    for contribution in &input.owner_contributions {
        if contribution.context_kind != input.context_kind
            || contribution.protocol_version != SUPPORTED_PROTOCOL_VERSION
            || contribution.obligation_ids.len() > MAX_REFERENCES_PER_ROW
            || contribution.class_ids.len() > MAX_REFERENCES_PER_ROW
            || contribution.depends_on.len() > MAX_REFERENCES_PER_ROW
        {
            return Err(AuthorityContinuityError::WrongContext);
        }
        if unique_set(
            &contribution.obligation_ids,
            AuthorityContinuityError::ContributionCoverageMismatch,
        )?
        .len()
            != contribution.obligation_ids.len()
            || unique_set(
                &contribution.class_ids,
                AuthorityContinuityError::ContributionCoverageMismatch,
            )?
            .len()
                != contribution.class_ids.len()
        {
            return Err(AuthorityContinuityError::ContributionCoverageMismatch);
        }
        for obligation_id in &contribution.obligation_ids {
            let obligation = obligations
                .get(obligation_id)
                .ok_or(AuthorityContinuityError::OrphanReference)?;
            if obligation.owner != contribution.owner
                || !contributed_obligations.insert(*obligation_id)
            {
                return Err(AuthorityContinuityError::WrongOwner);
            }
        }
        for class_id in &contribution.class_ids {
            if canonical_owner(*class_id) != contribution.owner
                || !contributed_classes.insert(*class_id)
            {
                return Err(AuthorityContinuityError::WrongOwner);
            }
        }
    }
    if contributed_obligations != obligations.keys().copied().collect()
        || contributed_classes != class_sum
    {
        return Err(AuthorityContinuityError::ContributionCoverageMismatch);
    }
    validate_dag(
        &contribution_ids,
        input
            .owner_contributions
            .iter()
            .map(|contribution| (contribution.id, contribution.depends_on.as_slice())),
    )?;

    let descriptors = unique_map(
        &input.descriptors,
        |descriptor| descriptor.class_id,
        AuthorityContinuityError::DuplicateClass,
    )?;
    if descriptors.keys().copied().collect::<BTreeSet<_>>() != class_sum {
        return Err(AuthorityContinuityError::DescriptorSetMismatch);
    }
    for descriptor in &input.descriptors {
        if descriptor.context_kind != input.context_kind
            || descriptor.class_id.context_kind() != input.context_kind
            || descriptor.protocol_version != SUPPORTED_PROTOCOL_VERSION
            || descriptor.depends_on.len() > MAX_REFERENCES_PER_ROW
        {
            return Err(AuthorityContinuityError::WrongContext);
        }
        let contribution = contributions
            .get(&descriptor.owner_contribution_id)
            .ok_or(AuthorityContinuityError::OrphanReference)?;
        if descriptor.owner != canonical_owner(descriptor.class_id)
            || descriptor.owner != contribution.owner
            || descriptor.owner_protocol != contribution.owner_protocol
            || !contribution.class_ids.contains(&descriptor.class_id)
        {
            return Err(AuthorityContinuityError::WrongOwner);
        }
    }
    validate_dag(
        &class_sum,
        input
            .descriptors
            .iter()
            .map(|descriptor| (descriptor.class_id, descriptor.depends_on.as_slice())),
    )?;
    Ok(())
}

fn normalize_totality(input: &mut AuthorityContinuityTotalityInputV1) {
    input.obligations.sort_by_key(|row| row.id);
    input.dispositions.sort_by_key(|row| row.obligation_id);
    input.owner_contributions.sort_by_key(|row| row.id);
    input.closed_class_sum.sort_unstable();
    input.descriptors.sort_by_key(|row| row.class_id);
    for disposition in &mut input.dispositions {
        if let CoverageDispositionKindV1::IncludedBy { class_ids } = &mut disposition.kind {
            class_ids.sort_unstable();
        }
    }
    for contribution in &mut input.owner_contributions {
        contribution.obligation_ids.sort_unstable();
        contribution.class_ids.sort_unstable();
        contribution.depends_on.sort_unstable();
    }
    for descriptor in &mut input.descriptors {
        descriptor.depends_on.sort_unstable();
    }
}

fn unique_map<K: Copy + Ord, V>(
    values: &[V],
    key: impl Fn(&V) -> K,
    error: AuthorityContinuityError,
) -> Result<BTreeMap<K, &V>, AuthorityContinuityError> {
    let mut map = BTreeMap::new();
    for value in values {
        if map.insert(key(value), value).is_some() {
            return Err(error);
        }
    }
    Ok(map)
}

fn unique_set<T: Copy + Ord>(
    values: &[T],
    error: AuthorityContinuityError,
) -> Result<BTreeSet<T>, AuthorityContinuityError> {
    let set = values.iter().copied().collect::<BTreeSet<_>>();
    if set.len() == values.len() {
        Ok(set)
    } else {
        Err(error)
    }
}

fn exact_rows_by_key<K: Copy + Ord, V: Eq>(
    actual: &[V],
    expected: &[V],
    key: impl Fn(&V) -> K,
) -> bool {
    if actual.len() != expected.len() {
        return false;
    }
    let actual = actual
        .iter()
        .map(|row| (key(row), row))
        .collect::<BTreeMap<_, _>>();
    let expected = expected
        .iter()
        .map(|row| (key(row), row))
        .collect::<BTreeMap<_, _>>();
    actual.len() == expected.len() && actual == expected
}

fn validate_dag<'a, N: Copy + Ord + 'a>(
    nodes: &BTreeSet<N>,
    dependencies: impl Iterator<Item = (N, &'a [N])>,
) -> Result<(), AuthorityContinuityError> {
    let mut dependents = BTreeMap::<N, Vec<N>>::new();
    let mut indegree = nodes
        .iter()
        .map(|node| (*node, 0_usize))
        .collect::<BTreeMap<_, _>>();
    for (node, requirements) in dependencies {
        let unique = requirements.iter().copied().collect::<BTreeSet<_>>();
        if unique.len() != requirements.len() {
            return Err(AuthorityContinuityError::CyclicReference);
        }
        for requirement in unique {
            if !nodes.contains(&requirement) {
                return Err(AuthorityContinuityError::OrphanReference);
            }
            *indegree
                .get_mut(&node)
                .expect("invariant: dependency owner belongs to node set") += 1;
            dependents.entry(requirement).or_default().push(node);
        }
    }
    let mut queue = indegree
        .iter()
        .filter_map(|(node, degree)| (*degree == 0).then_some(*node))
        .collect::<VecDeque<_>>();
    let mut visited = 0;
    while let Some(node) = queue.pop_front() {
        visited += 1;
        for dependent in dependents.get(&node).into_iter().flatten() {
            let degree = indegree
                .get_mut(dependent)
                .expect("invariant: dependent belongs to node set");
            *degree -= 1;
            if *degree == 0 {
                queue.push_back(*dependent);
            }
        }
    }
    if visited == nodes.len() {
        Ok(())
    } else {
        Err(AuthorityContinuityError::CyclicReference)
    }
}

#[derive(Clone, Copy)]
struct FrozenOwnerSourceRowV1 {
    obligation_id: u16,
    owner: ContinuitySemanticOwnerV1,
    included_class: Option<ContinuityClassIdV1>,
}

fn frozen_owner_source_rows(context_kind: AuthorityContextKindV1) -> Vec<FrozenOwnerSourceRowV1> {
    use ContinuitySemanticOwnerV1 as Owner;
    use InstallationAuthorityContinuityClassV1 as I;
    use RepositoryAuthorityContinuityClassV1 as R;

    macro_rules! repository_row {
        ($id:literal, $owner:ident, $class:ident) => {
            FrozenOwnerSourceRowV1 {
                obligation_id: $id,
                owner: Owner::$owner,
                included_class: Some(ContinuityClassIdV1::Repository(R::$class)),
            }
        };
    }
    macro_rules! installation_row {
        ($id:literal, $owner:ident, $class:ident) => {
            FrozenOwnerSourceRowV1 {
                obligation_id: $id,
                owner: Owner::$owner,
                included_class: Some(ContinuityClassIdV1::Installation(I::$class)),
            }
        };
    }

    match context_kind {
        AuthorityContextKindV1::RepositoryAuthorityContext => vec![
            repository_row!(1, Authority, RepositoryOrdinaryMutationCapacityState),
            repository_row!(2, Authority, RepositoryAuthorityAdministrationCapacityState),
            repository_row!(3, Authority, RepositoryEvidenceAcquisitionCapacityState),
            repository_row!(4, Authority, RepositoryPlanningPublicationCapacityState),
            repository_row!(5, Authority, RepositoryExternalEffectCapacityState),
            repository_row!(6, Authority, RepositoryPersistenceMaintenanceCapacityState),
            repository_row!(7, Authority, RepositoryStoreGenerationCurrentness),
            repository_row!(8, Authority, RepositoryGovernanceHead),
            repository_row!(9, Authority, RepositoryAuthorityEpochState),
            repository_row!(10, Authority, RepositoryTrustRootState),
            repository_row!(11, Authority, RepositoryPrincipalBindingState),
            repository_row!(12, Authority, RepositorySessionState),
            repository_row!(13, Authority, RepositoryGrantState),
            repository_row!(14, Authority, RepositoryDelegationState),
            repository_row!(15, Authority, RepositoryMandateState),
            repository_row!(16, Authority, RepositoryRevocationState),
            repository_row!(17, Authority, RepositoryAuthorizationReceiptState),
            repository_row!(18, Authority, RepositoryConsumptionCellState),
            repository_row!(19, Authority, RepositoryContinuityState),
            repository_row!(20, Authority, RepositoryTrustedTimeState),
            repository_row!(21, Authority, RepositoryRecoveryCommitmentState),
            repository_row!(22, Authority, RepositoryRecoveryAdmissionState),
            repository_row!(23, Execution, RepositoryStepExecutionState),
            repository_row!(24, Execution, RepositoryEffectIntentState),
            repository_row!(25, Evidence, RepositoryEvidenceState),
            repository_row!(26, Evidence, RepositoryGateSnapshot),
            repository_row!(27, Planning, RepositoryPlanningState),
            repository_row!(28, Coordination, RepositoryCoordinationState),
            repository_row!(29, Design, RepositoryDesignDecisionState),
            repository_row!(30, Contract, RepositoryContractState),
            repository_row!(31, Work, RepositoryWorkState),
            repository_row!(32, Persistence, RepositoryPersistenceRetentionState),
            repository_row!(33, Memory, RepositoryMemoryState),
            repository_row!(34, Intake, RepositoryIntakeState),
            repository_row!(35, Research, RepositoryResearchState),
            FrozenOwnerSourceRowV1 {
                obligation_id: 1_000,
                owner: Owner::Authority,
                included_class: None,
            },
        ],
        AuthorityContextKindV1::InstallationAuthorityContext => vec![
            installation_row!(
                1,
                Authority,
                InstallationAuthorityAdministrationCapacityState
            ),
            installation_row!(2, Authority, InstallationDistributionMutationCapacityState),
            installation_row!(
                3,
                Authority,
                InstallationGovernedReviewPublicationCapacityState
            ),
            installation_row!(4, Authority, InstallationExternalEffectCapacityState),
            installation_row!(5, Authority, InstallationWriterAdministrationCapacityState),
            installation_row!(
                6,
                Authority,
                InstallationPersistenceMaintenanceCapacityState
            ),
            installation_row!(7, Authority, InstallationLocatorCurrentness),
            installation_row!(8, Authority, InstallationStoreGenerationCurrentness),
            installation_row!(9, Authority, InstallationGovernanceHead),
            installation_row!(10, Authority, InstallationAuthorityEpochState),
            installation_row!(11, Authority, InstallationTrustRootState),
            installation_row!(12, Authority, InstallationPrincipalBindingState),
            installation_row!(13, Authority, InstallationGrantState),
            installation_row!(14, Authority, InstallationMandateState),
            installation_row!(15, Authority, InstallationRevocationState),
            installation_row!(16, Authority, InstallationAuthorizationReceiptState),
            installation_row!(17, Authority, InstallationConsumptionCellState),
            installation_row!(18, Authority, InstallationContinuityState),
            installation_row!(19, Authority, InstallationRecoveryCommitmentState),
            installation_row!(20, Authority, InstallationRecoveryAdmissionState),
            installation_row!(21, Installation, InstallationWriterCohortState),
            installation_row!(22, Installation, InstallationClientCompatibilityState),
            installation_row!(23, Distribution, InstallationDistributionTargetState),
            installation_row!(24, Distribution, InstallationDistributionTransactionState),
            installation_row!(25, Installation, InstallationBinarySlotState),
            installation_row!(26, Distribution, InstallationResourceManifestState),
            installation_row!(27, Distribution, InstallationGovernedReviewPublicationState),
            installation_row!(28, Execution, InstallationEffectIntentState),
            installation_row!(29, Evidence, InstallationEvidenceState),
            installation_row!(30, Persistence, InstallationPersistenceRetentionState),
            FrozenOwnerSourceRowV1 {
                obligation_id: 1_001,
                owner: Owner::Authority,
                included_class: None,
            },
        ],
    }
}

fn materialize_frozen_owner_census(
    context_kind: AuthorityContextKindV1,
) -> Result<
    (
        Vec<AuthorityContinuityCoverageObligationV1>,
        Vec<AuthorityContinuityCoverageDispositionV1>,
    ),
    AuthorityContinuityError,
> {
    let context_label = match context_kind {
        AuthorityContextKindV1::RepositoryAuthorityContext => "repository",
        AuthorityContextKindV1::InstallationAuthorityContext => "installation",
    };
    let mut obligations = Vec::new();
    let mut dispositions = Vec::new();
    for row in frozen_owner_source_rows(context_kind) {
        let id = CoverageObligationIdV1::new(row.obligation_id)
            .expect("invariant: frozen owner-source IDs are nonzero");
        let (source_protocol, subject_projection, kind) = match row.included_class {
            Some(class_id) => (
                ContinuityReferenceV1::derive(&format!(
                    "{context_label}-owner-{}-source-{}",
                    row.owner as u8,
                    class_id.tag()
                ))?,
                ContinuityReferenceV1::derive(&format!(
                    "{context_label}-subject-projection-{}",
                    class_id.tag()
                ))?,
                CoverageDispositionKindV1::IncludedBy {
                    class_ids: vec![class_id],
                },
            ),
            None => (
                ContinuityReferenceV1::derive(&format!("{context_label}-continuity-meta-types"))?,
                ContinuityReferenceV1::derive(&format!(
                    "{context_label}-continuity-meta-type-projection"
                ))?,
                CoverageDispositionKindV1::ExplicitlyNonContinuity {
                    owner_invariant: ContinuityReferenceV1::derive(&format!(
                        "{context_label}-meta-types-outside-class-universe"
                    ))?,
                    proof: ContinuityReferenceV1::derive(&format!(
                        "{context_label}-meta-type-exclusion-proof"
                    ))?,
                },
            ),
        };
        obligations.push(AuthorityContinuityCoverageObligationV1 {
            id,
            context_kind,
            owner: row.owner,
            source_protocol,
            subject_projection,
            protocol_version: SUPPORTED_PROTOCOL_VERSION,
        });
        dispositions.push(AuthorityContinuityCoverageDispositionV1 {
            obligation_id: id,
            owner: row.owner,
            kind,
        });
    }
    Ok((obligations, dispositions))
}

fn builtin_totality_input(
    context_kind: AuthorityContextKindV1,
) -> Result<AuthorityContinuityTotalityInputV1, AuthorityContinuityError> {
    let class_ids = required_class_ids(context_kind);
    let context_label = match context_kind {
        AuthorityContextKindV1::RepositoryAuthorityContext => "repository",
        AuthorityContextKindV1::InstallationAuthorityContext => "installation",
    };
    let (obligations, dispositions) = materialize_frozen_owner_census(context_kind)?;
    let mut descriptors = Vec::with_capacity(class_ids.len());
    let mut by_owner = BTreeMap::<
        ContinuitySemanticOwnerV1,
        (Vec<CoverageObligationIdV1>, Vec<ContinuityClassIdV1>),
    >::new();

    for obligation in &obligations {
        by_owner
            .entry(obligation.owner)
            .or_default()
            .0
            .push(obligation.id);
    }
    for class_id in &class_ids {
        by_owner
            .entry(canonical_owner(*class_id))
            .or_default()
            .1
            .push(*class_id);
    }

    let mut contribution_ids = BTreeMap::new();
    for owner in by_owner.keys() {
        contribution_ids.insert(
            *owner,
            OwnerContributionIdV1::new(u16::from(*owner as u8))
                .expect("invariant: owner tags are nonzero"),
        );
    }
    let owner_contributions = by_owner
        .iter()
        .map(|(owner, (obligation_ids, owner_class_ids))| {
            Ok(AuthorityContinuityOwnerContributionV1 {
                id: contribution_ids[owner],
                context_kind,
                owner: *owner,
                owner_protocol: ContinuityReferenceV1::derive(&format!(
                    "{context_label}-owner-protocol-{}",
                    *owner as u8
                ))?,
                obligation_ids: obligation_ids.clone(),
                class_ids: owner_class_ids.clone(),
                depends_on: Vec::new(),
                protocol_version: SUPPORTED_PROTOCOL_VERSION,
            })
        })
        .collect::<Result<Vec<_>, AuthorityContinuityError>>()?;

    for class_id in &class_ids {
        let owner = canonical_owner(*class_id);
        descriptors.push(AuthorityContinuityClassDescriptorV1 {
            class_id: *class_id,
            context_kind,
            owner,
            owner_contribution_id: contribution_ids[&owner],
            owner_protocol: ContinuityReferenceV1::derive(&format!(
                "{context_label}-owner-protocol-{}",
                owner as u8
            ))?,
            subject_projection: ContinuityReferenceV1::derive(&format!(
                "{context_label}-subject-projection-{}",
                class_id.tag()
            ))?,
            closure_schema: ContinuityReferenceV1::derive(&format!(
                "{context_label}-closure-schema-{}",
                class_id.tag()
            ))?,
            disposition: ClassDispositionV1::CanonicalRecordClosure,
            depends_on: Vec::new(),
            protocol_version: SUPPORTED_PROTOCOL_VERSION,
        });
    }

    Ok(AuthorityContinuityTotalityInputV1 {
        context_kind,
        protocol_version: SUPPORTED_PROTOCOL_VERSION,
        canonicalization_version: SUPPORTED_PROTOCOL_VERSION,
        obligations,
        dispositions,
        owner_contributions,
        closed_class_sum: class_ids,
        descriptors,
    })
}

fn obligation_value(obligation: &AuthorityContinuityCoverageObligationV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(u64::from(obligation.id.get())),
        CborValue::Unsigned(obligation.context_kind as u64),
        CborValue::Unsigned(obligation.owner as u64),
        CborValue::Bytes(obligation.source_protocol.as_bytes().to_vec()),
        CborValue::Bytes(obligation.subject_projection.as_bytes().to_vec()),
        CborValue::Unsigned(u64::from(obligation.protocol_version)),
    ])
}

fn disposition_value(disposition: &AuthorityContinuityCoverageDispositionV1) -> CborValue {
    let kind = match &disposition.kind {
        CoverageDispositionKindV1::IncludedBy { class_ids } => CborValue::Array(vec![
            CborValue::Unsigned(1),
            CborValue::Array(
                class_ids
                    .iter()
                    .map(|class_id| class_id.schema_value())
                    .collect(),
            ),
        ]),
        CoverageDispositionKindV1::ExplicitlyNonContinuity {
            owner_invariant,
            proof,
        } => CborValue::Array(vec![
            CborValue::Unsigned(2),
            CborValue::Bytes(owner_invariant.as_bytes().to_vec()),
            CborValue::Bytes(proof.as_bytes().to_vec()),
        ]),
    };
    CborValue::Array(vec![
        CborValue::Unsigned(u64::from(disposition.obligation_id.get())),
        CborValue::Unsigned(disposition.owner as u64),
        kind,
    ])
}

fn contribution_value(contribution: &AuthorityContinuityOwnerContributionV1) -> CborValue {
    CborValue::Array(vec![
        CborValue::Unsigned(u64::from(contribution.id.get())),
        CborValue::Unsigned(contribution.context_kind as u64),
        CborValue::Unsigned(contribution.owner as u64),
        CborValue::Bytes(contribution.owner_protocol.as_bytes().to_vec()),
        CborValue::Array(
            contribution
                .obligation_ids
                .iter()
                .map(|id| CborValue::Unsigned(u64::from(id.get())))
                .collect(),
        ),
        CborValue::Array(
            contribution
                .class_ids
                .iter()
                .map(|class_id| class_id.schema_value())
                .collect(),
        ),
        CborValue::Array(
            contribution
                .depends_on
                .iter()
                .map(|id| CborValue::Unsigned(u64::from(id.get())))
                .collect(),
        ),
        CborValue::Unsigned(u64::from(contribution.protocol_version)),
    ])
}

fn descriptor_value(descriptor: &AuthorityContinuityClassDescriptorV1) -> CborValue {
    CborValue::Array(vec![
        descriptor.class_id.schema_value(),
        CborValue::Unsigned(descriptor.context_kind as u64),
        CborValue::Unsigned(descriptor.owner as u64),
        CborValue::Unsigned(u64::from(descriptor.owner_contribution_id.get())),
        CborValue::Bytes(descriptor.owner_protocol.as_bytes().to_vec()),
        CborValue::Bytes(descriptor.subject_projection.as_bytes().to_vec()),
        CborValue::Bytes(descriptor.closure_schema.as_bytes().to_vec()),
        CborValue::Unsigned(descriptor.disposition as u64),
        CborValue::Array(
            descriptor
                .depends_on
                .iter()
                .map(|class_id| class_id.schema_value())
                .collect(),
        ),
        CborValue::Unsigned(u64::from(descriptor.protocol_version)),
    ])
}

fn hash(value: &CborValue) -> Result<[u8; 32], CborError> {
    Ok(Sha256::digest(deterministic_cbor::encode(value)?).into())
}
